use std::collections::HashMap;
use std::time::Duration;

use crate::engine::anilist::auth::{load_token, mark_token_invalid, token_stored};
use crate::engine::anilist::client::{AniListClient, AniListHttpError};
use crate::engine::events::EngineEvent;
use crate::engine::runtime::EngineState;
use crate::engine::storage::{Storage, SYNC_BLOCKED_RETRY_COUNT};

/// Returns the backoff delay in seconds for a given retry count.
///
/// Pattern: 1 min, 5 min, 15 min, 1 h, then capped at 6 h. Transient failures
/// (network, 5xx, 429) retry forever at this pace instead of giving up.
pub fn backoff_delay(retry_count: i32) -> u64 {
    match retry_count {
        i32::MIN..=0 => 60,
        1 => 5 * 60,
        2 => 15 * 60,
        3 => 60 * 60,
        _ => 6 * 60 * 60,
    }
}

fn http_error(err: &anyhow::Error) -> Option<&AniListHttpError> {
    err.downcast_ref::<AniListHttpError>()
}

/// AniList rejected the access token (expired after a year, or revoked).
/// AniList answers with 401, or with 400 and an "Invalid token" GraphQL error.
pub fn is_token_error(err: &anyhow::Error) -> bool {
    match http_error(err) {
        Some(e) => e.status == 401 || e.body.to_ascii_lowercase().contains("invalid token"),
        None => false,
    }
}

/// A failure that retrying will not fix: AniList rejected the request itself
/// (4xx other than rate limiting). Token errors are excluded — the rows are
/// fine and go through once the user reconnects.
pub fn is_terminal_sync_error(err: &anyhow::Error) -> bool {
    match http_error(err) {
        Some(e) => (400..500).contains(&e.status) && e.status != 429 && !is_token_error(err),
        None => false,
    }
}

/// What a sync push sends for one anime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPush {
    pub status: Option<String>,
    pub progress: i32,
    pub score: Option<i32>,
}

/// Resolve what to push for an anime: the live `list_entry` when there is one,
/// so a row queued long ago (e.g. before a disconnect) can't push older
/// progress over newer. Falls back to the queued payload otherwise.
pub async fn resolve_sync_push(
    storage: &Storage,
    anime_id: i64,
    payload_json: &str,
) -> anyhow::Result<SyncPush> {
    if let Some(entry) = storage.get_list_entry_full(anime_id).await? {
        return Ok(SyncPush {
            status: Some(entry.status),
            progress: entry.watched_episodes,
            score: entry.score,
        });
    }
    let payload: serde_json::Value = serde_json::from_str(payload_json).unwrap_or_default();
    Ok(SyncPush {
        status: payload["status"].as_str().map(|s| s.to_string()),
        progress: payload["episode"].as_i64().unwrap_or(0) as i32,
        score: payload["score"].as_i64().map(|v| v as i32),
    })
}

/// How a failed push for one anime was handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushFailure {
    /// The token was rejected; rows are untouched and draining should stop.
    TokenInvalid,
    /// AniList rejected the request; rows are blocked until retried by hand.
    Blocked,
    /// Transient; rows wait out their backoff.
    Retrying,
}

/// Record a failed push for `rows` (row id, retry count) of one anime.
pub async fn handle_push_failure(
    state: &EngineState,
    anime_id: i64,
    rows: &[(i64, i32)],
    err: &anyhow::Error,
    now: i64,
) -> anyhow::Result<PushFailure> {
    if is_token_error(err) {
        mark_token_invalid(&state.storage).await?;
        tracing::warn!("AniList rejected the access token; sync paused until reconnect");
        return Ok(PushFailure::TokenInvalid);
    }
    if is_terminal_sync_error(err) {
        for (row_id, _) in rows {
            state
                .storage
                .update_sync_retry(*row_id, SYNC_BLOCKED_RETRY_COUNT, now)
                .await?;
        }
        state.events.publish(EngineEvent::SyncFailed {
            service: "anilist".to_string(),
            anime_id,
            message: err.to_string(),
        });
        return Ok(PushFailure::Blocked);
    }
    for (row_id, retry_count) in rows {
        let new_count = (retry_count + 1).min(SYNC_BLOCKED_RETRY_COUNT - 1);
        let next_retry = now + backoff_delay(*retry_count) as i64;
        state
            .storage
            .update_sync_retry(*row_id, new_count, next_retry)
            .await?;
    }
    Ok(PushFailure::Retrying)
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Drain pending sync rows for the "anilist" service.
///
/// 1. Loads the stored access token — exits early if none (or if rejected).
/// 2. Fetches up to 50 pending sync rows and groups them by `anime_id`.
/// 3. Pushes the live list state for each anime (see `resolve_sync_push`).
/// 4. On success: deletes every queued row for that anime.
/// 5. On failure: see `handle_push_failure`. A rejected token stops the pass.
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

    // Rows are ordered by created_at ASC, so the last payload per anime is the
    // newest (only used when the anime has no list entry any more).
    let mut latest_payload: HashMap<i64, &str> = HashMap::new();
    let mut anime_rows: HashMap<i64, Vec<(i64, i32)>> = HashMap::new(); // anime_id -> (row id, retry_count)
    let mut order: Vec<i64> = Vec::new();
    for row in &rows {
        if !anime_rows.contains_key(&row.anime_id) {
            order.push(row.anime_id);
        }
        latest_payload.insert(row.anime_id, row.payload_json.as_str());
        anime_rows
            .entry(row.anime_id)
            .or_default()
            .push((row.id, row.retry_count));
    }

    let mut any_success = false;
    for anime_id in order {
        let push = resolve_sync_push(&state.storage, anime_id, latest_payload[&anime_id]).await?;
        match client
            .push_list_entry(anime_id, push.status.as_deref(), push.progress, push.score)
            .await
        {
            Ok(()) => {
                any_success = true;
                state
                    .storage
                    .delete_sync_rows_for_anime(anime_id, "anilist")
                    .await?;
            }
            Err(err) => {
                let outcome =
                    handle_push_failure(state, anime_id, &anime_rows[&anime_id], &err, unix_now())
                        .await?;
                if outcome == PushFailure::TokenInvalid {
                    break;
                }
            }
        }
    }

    // Record when a push last went through, for the Sync card.
    if any_success {
        let now = unix_now();
        let _ = state
            .storage
            .set_setting("anilist.last_sync_at", &now.to_string(), now)
            .await;
    }

    Ok(())
}

/// Queue an AniList update for an anime's current list state. Best-effort and
/// only when a token is stored (an expired one included, so the change reaches
/// AniList after reconnecting). The drain pushes the live `list_entry`; the
/// payload is a fallback. Call after any list-entry change.
pub async fn enqueue_anilist_sync(state: &EngineState, anime_id: i64) {
    if !matches!(token_stored(&state.storage).await, Ok(true)) {
        return;
    }
    let entry = match state.storage.get_list_entry_full(anime_id).await {
        Ok(Some(e)) => e,
        _ => return,
    };
    let payload = serde_json::json!({
        "episode": entry.watched_episodes,
        "status": entry.status,
        "score": entry.score,
    })
    .to_string();
    let _ = state
        .storage
        .queue_sync(anime_id, "anilist", "update", &payload, unix_now())
        .await;
}

// Backfill missing episode counts / airing status from AniList (best-effort,
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
