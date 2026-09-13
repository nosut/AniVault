//! Sync failure handling tests.
//!
//! Focuses on retry-count persistence, blocked-item exclusion, failure
//! classification in the drain loop, and SyncFailed event delivery.

use anivault_core::engine::anilist::auth;
use anivault_core::engine::anilist::client::AniListHttpError;
use anivault_core::engine::events::EngineEvent;
use anivault_core::engine::runtime::{fresh_test_state, EngineState};
use anivault_core::engine::storage::{Storage, SYNC_BLOCKED_RETRY_COUNT};
use anivault_core::engine::sync_worker::{
    backoff_delay, handle_push_failure, resolve_sync_push, PushFailure, SyncPush,
};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

async fn new_storage() -> Storage {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage
}

/// Return a Unix timestamp safely in the past.
fn past_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
        - 3600 // one hour ago
}

fn http(status: u16, body: &str) -> anyhow::Error {
    anyhow::Error::new(AniListHttpError {
        status,
        body: body.to_string(),
    })
}

// ---------------------------------------------------------------------------
// retry count gets incremented and persisted
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sync_queue_retry_count_increments() {
    let storage = new_storage().await;

    storage
        .upsert_anime(42, r#"{"romaji":"Test 42"}"#, 12, None, 1000)
        .await
        .unwrap();

    let row_id = storage
        .queue_sync(42, "anilist", "progress_update", r#"{"episode":5}"#, 1000)
        .await
        .unwrap();

    // Fresh row: retry_count = 0, next_retry_at = NULL → pending.
    let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
    let row = rows.iter().find(|r| r.id == row_id).unwrap();
    assert_eq!(row.retry_count, 0);
    assert!(row.next_retry_at.is_none());

    storage.update_sync_retry(row_id, 1, i64::MAX).await.unwrap();
    let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
    assert!(
        !rows.iter().any(|r| r.id == row_id),
        "future next_retry_at excludes row from pending"
    );

    storage.update_sync_retry(row_id, 2, past_ts()).await.unwrap();
    let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
    let row = rows.iter().find(|r| r.id == row_id).unwrap();
    assert_eq!(row.retry_count, 2, "retry_count persisted as 2");
    assert!(row.next_retry_at.is_some());
}

// ---------------------------------------------------------------------------
// transient failures never block, however many retries pile up
// ---------------------------------------------------------------------------

#[tokio::test]
async fn many_transient_retries_stay_pending_once_due() {
    let storage = new_storage().await;
    storage
        .upsert_anime(5, r#"{"romaji":"Test 5"}"#, 12, None, 1000)
        .await
        .unwrap();
    let row_id = storage
        .queue_sync(5, "anilist", "update", r#"{"episode":1}"#, 1000)
        .await
        .unwrap();
    storage.update_sync_retry(row_id, 40, past_ts()).await.unwrap();

    let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
    assert!(rows.iter().any(|r| r.id == row_id));
    let (_p, failed, blocked) = storage.sync_status_counts("anilist").await.unwrap();
    assert_eq!(failed, 1);
    assert_eq!(blocked, 0);
}

// ---------------------------------------------------------------------------
// blocked items excluded from pending
// ---------------------------------------------------------------------------

#[tokio::test]
async fn blocked_items_excluded_from_pending() {
    let storage = new_storage().await;

    storage
        .upsert_anime(99, r#"{"romaji":"Test 99"}"#, 12, None, 1000)
        .await
        .unwrap();

    let row_id = storage
        .queue_sync(99, "anilist", "progress_update", r#"{"episode":1}"#, 1000)
        .await
        .unwrap();

    storage
        .update_sync_retry(row_id, SYNC_BLOCKED_RETRY_COUNT, past_ts())
        .await
        .unwrap();

    let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
    assert!(
        !rows.iter().any(|r| r.id == row_id),
        "blocked item excluded from pending even when its retry time has passed"
    );

    let (_pending, _failed, blocked) = storage.sync_status_counts("anilist").await.unwrap();
    assert_eq!(blocked, 1, "sync_status_counts should report 1 blocked");
}

#[tokio::test]
async fn sync_queue_transitions_through_retry_states() {
    let storage = new_storage().await;

    storage
        .upsert_anime(7, r#"{"romaji":"Test 7"}"#, 12, None, 1000)
        .await
        .unwrap();

    let row_id = storage
        .queue_sync(7, "anilist", "progress_update", r#"{"episode":3}"#, 1000)
        .await
        .unwrap();

    let counts = storage.sync_status_counts("anilist").await.unwrap();
    assert_eq!(counts, (1, 0, 0), "fresh → pending");

    // Waiting out its backoff still counts as failed rather than vanishing.
    storage.update_sync_retry(row_id, 1, i64::MAX).await.unwrap();
    let counts = storage.sync_status_counts("anilist").await.unwrap();
    assert_eq!(counts, (0, 1, 0), "backing off → failed");

    storage.update_sync_retry(row_id, 2, past_ts()).await.unwrap();
    let counts = storage.sync_status_counts("anilist").await.unwrap();
    assert_eq!(counts, (0, 1, 0), "due again → failed");

    storage
        .update_sync_retry(row_id, SYNC_BLOCKED_RETRY_COUNT, i64::MAX)
        .await
        .unwrap();
    let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
    assert!(!rows.iter().any(|r| r.id == row_id));
    let counts = storage.sync_status_counts("anilist").await.unwrap();
    assert_eq!(counts, (0, 0, 1), "terminal → blocked");
}

// ---------------------------------------------------------------------------
// queue maintenance
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delete_sync_rows_for_anime_clears_old_blocked_rows_too() {
    let storage = new_storage().await;
    for id in [1i64, 2] {
        storage
            .upsert_anime(id, &format!(r#"{{"romaji":"T{id}"}}"#), 12, None, 1000)
            .await
            .unwrap();
    }
    let old = storage
        .queue_sync(1, "anilist", "update", "{}", 900)
        .await
        .unwrap();
    storage
        .update_sync_retry(old, SYNC_BLOCKED_RETRY_COUNT, i64::MAX)
        .await
        .unwrap();
    storage
        .queue_sync(1, "anilist", "update", "{}", 1000)
        .await
        .unwrap();
    storage
        .queue_sync(2, "anilist", "update", "{}", 1000)
        .await
        .unwrap();

    storage.delete_sync_rows_for_anime(1, "anilist").await.unwrap();

    assert_eq!(storage.pending_sync_count("anilist").await.unwrap(), 1);
    let (p, _f, b) = storage.sync_status_counts("anilist").await.unwrap();
    assert_eq!((p, b), (1, 0));
}

#[tokio::test]
async fn reset_blocked_sync_rows_makes_them_pending_again() {
    let storage = new_storage().await;
    storage
        .upsert_anime(3, r#"{"romaji":"T3"}"#, 12, None, 1000)
        .await
        .unwrap();
    let row = storage
        .queue_sync(3, "anilist", "update", "{}", 1000)
        .await
        .unwrap();
    storage
        .update_sync_retry(row, SYNC_BLOCKED_RETRY_COUNT, i64::MAX)
        .await
        .unwrap();

    let reset = storage.reset_blocked_sync_rows("anilist").await.unwrap();
    assert_eq!(reset, 1);

    let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
    let r = rows.iter().find(|r| r.id == row).expect("row pending again");
    assert_eq!(r.retry_count, 0);
    assert!(r.next_retry_at.is_none());
}

#[tokio::test]
async fn retry_blocked_sync_command_resets_rows() {
    let state = fresh_test_state().await;
    state
        .storage
        .upsert_anime(3, r#"{"romaji":"T3"}"#, 12, None, 1000)
        .await
        .unwrap();
    let row = state
        .storage
        .queue_sync(3, "anilist", "update", "{}", 1000)
        .await
        .unwrap();
    state
        .storage
        .update_sync_retry(row, SYNC_BLOCKED_RETRY_COUNT, i64::MAX)
        .await
        .unwrap();

    let n = anivault_core::commands::retry_blocked_sync_inner(&state)
        .await
        .unwrap();
    assert_eq!(n, 1);
    let status = anivault_core::commands::get_sync_status_inner(&state)
        .await
        .unwrap();
    assert_eq!((status.pending, status.blocked), (1, 0));
}

// ---------------------------------------------------------------------------
// failure handling in the drain loop
// ---------------------------------------------------------------------------

async fn state_with_queued_row() -> (EngineState, i64) {
    let state = fresh_test_state().await;
    state
        .storage
        .upsert_anime(11, r#"{"romaji":"T11"}"#, 12, None, 1000)
        .await
        .unwrap();
    let row = state
        .storage
        .queue_sync(11, "anilist", "update", r#"{"episode":2}"#, 1000)
        .await
        .unwrap();
    (state, row)
}

#[tokio::test]
async fn terminal_failure_blocks_rows_and_publishes_sync_failed() {
    let (state, row) = state_with_queued_row().await;

    let outcome = handle_push_failure(&state, 11, &[(row, 0)], &http(404, "gone"), 10_000)
        .await
        .unwrap();

    assert_eq!(outcome, PushFailure::Blocked);
    let (_p, _f, blocked) = state.storage.sync_status_counts("anilist").await.unwrap();
    assert_eq!(blocked, 1);
    assert!(state
        .events
        .drain()
        .iter()
        .any(|e| matches!(e, EngineEvent::SyncFailed { anime_id: 11, .. })));
}

#[tokio::test]
async fn transient_failure_schedules_backoff_without_blocking() {
    let (state, row) = state_with_queued_row().await;
    let now = 10_000;

    let outcome = handle_push_failure(&state, 11, &[(row, 7)], &http(503, ""), now)
        .await
        .unwrap();

    assert_eq!(outcome, PushFailure::Retrying);
    let (_p, failed, blocked) = state.storage.sync_status_counts("anilist").await.unwrap();
    assert_eq!((failed, blocked), (1, 0));
    let next = state
        .storage
        .sync_row_next_retry_at(row)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next, now + backoff_delay(7) as i64);
    assert!(
        state.events.drain().is_empty(),
        "no SyncFailed for a transient error"
    );
}

#[tokio::test]
async fn token_failure_leaves_rows_alone_and_marks_token_invalid() {
    let (state, row) = state_with_queued_row().await;
    auth::store_token(&state.storage, "expired").await.unwrap();

    let outcome = handle_push_failure(&state, 11, &[(row, 0)], &http(401, ""), 10_000)
        .await
        .unwrap();

    assert_eq!(outcome, PushFailure::TokenInvalid);
    let counts = state.storage.sync_status_counts("anilist").await.unwrap();
    assert_eq!(counts, (1, 0, 0), "row untouched");
    assert!(auth::is_token_invalid(&state.storage).await.unwrap());
    assert_eq!(auth::load_token(&state.storage).await.unwrap(), None);
}

// ---------------------------------------------------------------------------
// what gets pushed: live list state, not the enqueue-time payload
// ---------------------------------------------------------------------------

#[tokio::test]
async fn resolve_sync_push_reads_live_list_entry() {
    let storage = new_storage().await;
    storage
        .upsert_anime(21, r#"{"romaji":"T21"}"#, 12, None, 1000)
        .await
        .unwrap();
    // Enqueued at episode 3 …
    storage
        .upsert_list_entry_progress(21, "watching", 3, 1000)
        .await
        .unwrap();
    let stale = r#"{"episode":3,"status":"watching"}"#;
    // … then more was watched (e.g. while disconnected) and scored.
    storage
        .upsert_list_entry_full(21, "watching", 7, Some(85), "", 2000, 0)
        .await
        .unwrap();

    let push = resolve_sync_push(&storage, 21, stale).await.unwrap();
    assert_eq!(
        push,
        SyncPush {
            status: Some("watching".to_string()),
            progress: 7,
            score: Some(85),
        }
    );
}

#[tokio::test]
async fn resolve_sync_push_falls_back_to_payload_without_list_entry() {
    let storage = new_storage().await;
    let push = resolve_sync_push(
        &storage,
        404,
        r#"{"episode":4,"status":"completed","score":70}"#,
    )
    .await
    .unwrap();
    assert_eq!(
        push,
        SyncPush {
            status: Some("completed".to_string()),
            progress: 4,
            score: Some(70),
        }
    );
}

// ---------------------------------------------------------------------------
// SyncFailed event structure
// ---------------------------------------------------------------------------

#[test]
fn sync_failed_event_contains_required_fields() {
    let event = EngineEvent::SyncFailed {
        service: "anilist".to_string(),
        anime_id: 42,
        message: "test error".to_string(),
    };
    let cloned = event.clone();
    assert_eq!(event, cloned);
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: EngineEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, deserialized);
}

// ---------------------------------------------------------------------------
// mixed retry states
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multiple_rows_mixed_retry_states() {
    let storage = new_storage().await;

    for id in [1i64, 2, 3] {
        storage
            .upsert_anime(id, &format!(r#"{{"romaji":"Test {id}"}}"#), 12, None, 1000)
            .await
            .unwrap();
    }

    let id_a = storage
        .queue_sync(1, "anilist", "progress_update", r#"{"episode":1}"#, 1000)
        .await
        .unwrap();
    let id_b = storage
        .queue_sync(2, "anilist", "progress_update", r#"{"episode":2}"#, 1000)
        .await
        .unwrap();
    storage.update_sync_retry(id_b, 1, past_ts()).await.unwrap();
    let id_c = storage
        .queue_sync(3, "anilist", "progress_update", r#"{"episode":3}"#, 1000)
        .await
        .unwrap();
    storage
        .update_sync_retry(id_c, SYNC_BLOCKED_RETRY_COUNT, i64::MAX)
        .await
        .unwrap();

    let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
    let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    assert!(ids.contains(&id_a));
    assert!(ids.contains(&id_b));
    assert!(!ids.contains(&id_c));

    let counts = storage.sync_status_counts("anilist").await.unwrap();
    assert_eq!(counts, (1, 1, 1));
}
