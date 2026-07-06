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
    let mut latest: HashMap<i64, (Option<String>, i32)> = HashMap::new();
    for row in &rows {
        let payload: serde_json::Value =
            serde_json::from_str(&row.payload_json).unwrap_or_default();
        let episode = payload["episode"].as_i64().unwrap_or(0) as i32;
        let status = payload["status"].as_str().map(|s| s.to_string());
        latest.insert(row.anime_id, (status, episode));
    }

    for row in &rows {
        let (status, episode) = latest
            .get(&row.anime_id)
            .cloned()
            .unwrap_or((None, 0));

        match client
            .push_list_entry(row.anime_id, status.as_deref(), episode)
            .await
        {
            Ok(()) => {
                state.storage.delete_sync_row(row.id).await?;
            }
            Err(err) => {
                let new_count = row.retry_count + 1;
                let delay = backoff_delay(row.retry_count) as i64;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                let next_retry = now + delay;

                if new_count >= 3 {
                    state.events.publish(EngineEvent::SyncFailed {
                        service: "anilist".to_string(),
                        anime_id: row.anime_id,
                        message: format!("max retries: {err}"),
                    });
                }

                state
                    .storage
                    .update_sync_retry(row.id, new_count, next_retry)
                    .await?;
            }
        }
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
