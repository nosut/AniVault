use std::collections::HashMap;
use std::time::Duration;

use crate::engine::anilist::auth::load_token;
use crate::engine::anilist::client::AniListClient;
use crate::engine::events::EngineEvent;
use crate::engine::runtime::EngineState;

/// Returns the backoff delay in seconds for a given retry count.
///
/// Pattern: 1, 2, 4, capped at 4 for subsequent retries.
pub fn backoff_delay(retry_count: i32) -> u64 {
    match retry_count {
        0 => 1,
        1 => 2,
        _ => 4,
    }
}

/// Drain pending sync rows for the "anilist" service.
///
/// 1. Loads the stored access token — exits early if none.
/// 2. Fetches up to 50 pending sync rows.
/// 3. Deduplicates by `anime_id`, keeping only the latest episode.
/// 4. Pushes progress via `AniListClient::push_progress`.
/// 5. On success: deletes the sync row.
/// 6. On failure: increments retry, schedules backoff. At retry >= 3,
///    publishes `EngineEvent::SyncFailed` and leaves the row blocked.
pub async fn drain_queue(state: &EngineState) -> anyhow::Result<()> {
    let token = match load_token(&state.storage).await? {
        Some(t) => t,
        None => return Ok(()), // not connected, nothing to drain
    };
    let client = AniListClient::new(token);

    let rows = state.storage.fetch_pending_sync_rows("anilist", 50).await?;
    if rows.is_empty() {
        return Ok(());
    }

    // Dedup: rows are ordered by created_at ASC, so the last row per anime_id is
    // the newest — push its status + progress (captures both category and episode).
    // One API call per anime; every queued row for that anime is settled by it.
    let mut latest: HashMap<i64, (Option<String>, i32)> = HashMap::new();
    let mut anime_rows: HashMap<i64, Vec<(i64, i32)>> = HashMap::new(); // anime_id -> (row id, retry_count)
    for row in &rows {
        let payload: serde_json::Value =
            serde_json::from_str(&row.payload_json).unwrap_or_default();
        let episode = payload["episode"].as_i64().unwrap_or(0) as i32;
        let status = payload["status"].as_str().map(|s| s.to_string());
        latest.insert(row.anime_id, (status, episode));
        anime_rows
            .entry(row.anime_id)
            .or_default()
            .push((row.id, row.retry_count));
    }

    let mut any_success = false;
    for (anime_id, (status, episode)) in &latest {
        match client
            .push_list_entry(*anime_id, status.as_deref(), *episode)
            .await
        {
            Ok(()) => {
                any_success = true;
                for (row_id, _) in &anime_rows[anime_id] {
                    state.storage.delete_sync_row(*row_id).await?;
                }
            }
            Err(err) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                let mut newly_blocked = false;
                for (row_id, retry_count) in &anime_rows[anime_id] {
                    let new_count = retry_count + 1;
                    if new_count >= 3 {
                        newly_blocked = true;
                    }
                    let next_retry = now + backoff_delay(*retry_count) as i64;
                    state
                        .storage
                        .update_sync_retry(*row_id, new_count, next_retry)
                        .await?;
                }
                if newly_blocked {
                    state.events.publish(EngineEvent::SyncFailed {
                        service: "anilist".to_string(),
                        anime_id: *anime_id,
                        message: format!("max retries: {err}"),
                    });
                }
            }
        }
    }

    // Record when a push last went through, for the Sync card.
    if any_success {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let _ = state
            .storage
            .set_setting("anilist.last_sync_at", &now.to_string(), now)
            .await;
    }

    Ok(())
}

/// Queue an AniList update for an anime's current list state (status + progress).
/// Best-effort and only when connected — reads the live `list_entry` so the push
/// always reflects the latest local state. Call after any list-entry change.
pub async fn enqueue_anilist_sync(state: &EngineState, anime_id: i64) {
    // Only queue when connected to AniList.
    if !matches!(load_token(&state.storage).await, Ok(Some(_))) {
        return;
    }
    let entry = match state.storage.get_list_entry(anime_id).await {
        Ok(Some(e)) => e,
        _ => return,
    };
    let payload = serde_json::json!({
        "episode": entry.watched_episodes,
        "status": entry.status,
    })
    .to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let _ = state
        .storage
        .queue_sync(anime_id, "anilist", "update", &payload, now)
        .await;
}

/// Backfill missing episode counts / airing status from AniList (best-effort,
/// only when connected). Called on the sync worker's first pass and periodically.
async fn run_meta_backfill(state: &EngineState) {
    let token = match load_token(&state.storage).await {
        Ok(Some(t)) => t,
        _ => return, // not connected
    };
    let client = AniListClient::new(token);
    match crate::engine::anilist::import::backfill_anime_meta(&state.storage, &client, 100).await {
        Ok(n) if n > 0 => tracing::info!("Backfilled episode metadata for {n} anime"),
        Ok(_) => {}
        Err(e) => tracing::warn!("episode metadata backfill failed: {e}"),
    }
    // Sequel seasons AniList hasn't given an English title yet inherit one from
    // their prequel, so the library doesn't mix "Sousou no Frieren 3rd Season"
    // in among titles that did get translated.
    let data_dir = state.database_path.parent();
    match crate::engine::anilist::import::backfill_derived_titles(
        &state.storage,
        &client,
        data_dir,
        100,
    )
    .await
    {
        Ok(n) if n > 0 => tracing::info!("Derived English titles for {n} anime"),
        Ok(_) => {}
        Err(e) => tracing::warn!("derived-title backfill failed: {e}"),
    }
}

/// Spawn a background task that polls the sync queue every 30 seconds and, on the
/// first pass then roughly every 10 minutes, backfills unknown episode counts.
pub fn spawn_sync_worker(state: &EngineState) -> tauri::async_runtime::JoinHandle<()> {
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        tracing::debug!("Sync worker started for service: anilist");
        let mut cycle: u64 = 0;
        loop {
            let _ = drain_queue(&state).await;
            // First pass covers "on startup"; every 20th pass (~10 min) refreshes.
            if cycle % 20 == 0 {
                run_meta_backfill(&state).await;
            }
            cycle += 1;
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    })
}
