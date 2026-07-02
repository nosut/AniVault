//! Sync failure handling tests.
//!
//! Focuses on retry backoff calculation, retry-count persistence,
//! blocked-item exclusion, and SyncFailed event delivery.

use anivault_core::engine::events::EngineEvent;
use anivault_core::engine::storage::Storage;
use anivault_core::engine::sync_worker::backoff_delay;

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

// ---------------------------------------------------------------------------
// Test 1 – backoff_delay calculation
// ---------------------------------------------------------------------------

#[test]
fn backoff_delay_increases_with_retry_count() {
    // Pattern: 0→1s, 1→2s, 2+ capped at 4s.
    let d0 = backoff_delay(0);
    let d1 = backoff_delay(1);
    let d2 = backoff_delay(2);
    let d3 = backoff_delay(3);
    let d4 = backoff_delay(10);

    assert_eq!(d0, 1, "retry 0 → 1s");
    assert_eq!(d1, 2, "retry 1 → 2s");
    assert_eq!(d2, 4, "retry 2 → 4s");
    assert_eq!(d3, 4, "retry 3 → 4s (capped)");
    assert_eq!(d4, 4, "retry 10 → 4s (capped)");

    // Monotonic: each step is >= previous.
    assert!(d0 < d1, "delay must increase from 0→1");
    assert!(d1 < d2, "delay must increase from 1→2");
}

#[test]
fn backoff_delay_caps_at_maximum() {
    for count in 0..100 {
        let d = backoff_delay(count);
        assert!(d <= 4, "backoff_delay({}) = {} exceeds cap 4", count, d);
        assert!(d >= 1, "backoff_delay({}) = {} below minimum 1", count, d);
    }
}

// ---------------------------------------------------------------------------
// Test 2 – retry count gets incremented and persisted
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sync_queue_retry_count_increments() {
    let storage = new_storage().await;

    storage
        .upsert_anime(42, r#"{"romaji":"Test 42"}"#, 12, None, 1000)
        .await
        .unwrap();

    // Insert a fresh sync row.
    let row_id = storage
        .queue_sync(42, "anilist", "progress_update", r#"{"episode":5}"#, 1000)
        .await
        .unwrap();

    // Fresh row: retry_count = 0, next_retry_at = NULL → pending.
    let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
    let row = rows.iter().find(|r| r.id == row_id).unwrap();
    assert_eq!(row.retry_count, 0);
    assert!(row.next_retry_at.is_none());

    // Simulate first failure: set retry_count = 1, future next_retry.
    storage
        .update_sync_retry(row_id, 1, i64::MAX)
        .await
        .unwrap();
    let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
    assert!(
        !rows.iter().any(|r| r.id == row_id),
        "future next_retry_at excludes row from pending"
    );

    // Simulate second failure: retry_count = 2, past next_retry_at.
    storage
        .update_sync_retry(row_id, 2, past_ts())
        .await
        .unwrap();
    let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
    let row = rows.iter().find(|r| r.id == row_id).unwrap();
    assert_eq!(row.retry_count, 2, "retry_count persisted as 2");
    assert!(row.next_retry_at.is_some());
}

// ---------------------------------------------------------------------------
// Test 3 – blocked items (retry_count >= 3) excluded from pending
// ---------------------------------------------------------------------------

#[tokio::test]
async fn blocked_items_excluded_from_pending() {
    let storage = new_storage().await;

    storage
        .upsert_anime(99, r#"{"romaji":"Test 99"}"#, 12, None, 1000)
        .await
        .unwrap();

    // Queue a row and drive it to retry_count = 3 with future next_retry_at.
    let row_id = storage
        .queue_sync(99, "anilist", "progress_update", r#"{"episode":1}"#, 1000)
        .await
        .unwrap();

    storage
        .update_sync_retry(row_id, 3, i64::MAX)
        .await
        .unwrap();

    // fetch_pending_sync_rows should NOT return it (future next_retry_at).
    let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
    assert!(
        !rows.iter().any(|r| r.id == row_id),
        "blocked item (retry_count=3, future next_retry) excluded from pending"
    );

    // sync_status_counts should count it as blocked.
    let (_pending, _failed, blocked) = storage.sync_status_counts("anilist").await.unwrap();
    assert_eq!(blocked, 1, "sync_status_counts should report 1 blocked");
}

// ---------------------------------------------------------------------------
// Test 4 – items remain pending until retry_count >= 3 with future next_retry
// ---------------------------------------------------------------------------

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

    // State 0 – pending (retry_count=0, next_retry_at=NULL)
    {
        let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
        assert!(rows.iter().any(|r| r.id == row_id));
        let (p, f, b) = storage.sync_status_counts("anilist").await.unwrap();
        assert_eq!(p, 1, "state 0 → pending=1");
        assert_eq!(f, 0, "state 0 → failed=0");
        assert_eq!(b, 0, "state 0 → blocked=0");
    }

    // State 1 – failed (retry_count=1, past next_retry_at)
    storage
        .update_sync_retry(row_id, 1, past_ts())
        .await
        .unwrap();
    {
        let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
        assert!(rows.iter().any(|r| r.id == row_id));
        let (p, f, b) = storage.sync_status_counts("anilist").await.unwrap();
        assert_eq!(p, 0, "state 1 → pending=0");
        assert_eq!(f, 1, "state 1 → failed=1");
        assert_eq!(b, 0, "state 1 → blocked=0");
    }

    // State 2 – failed (retry_count=2, past next_retry_at)
    storage
        .update_sync_retry(row_id, 2, past_ts())
        .await
        .unwrap();
    {
        let (p, f, b) = storage.sync_status_counts("anilist").await.unwrap();
        assert_eq!(p, 0, "state 2 → pending=0");
        assert_eq!(f, 1, "state 2 → failed=1");
        assert_eq!(b, 0, "state 2 → blocked=0");
    }

    // State 3 – blocked (retry_count=3, future next_retry_at)
    storage
        .update_sync_retry(row_id, 3, i64::MAX)
        .await
        .unwrap();
    {
        let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
        assert!(!rows.iter().any(|r| r.id == row_id));
        let (p, f, b) = storage.sync_status_counts("anilist").await.unwrap();
        assert_eq!(p, 0, "state 3 → pending=0");
        assert_eq!(f, 0, "state 3 → failed=0");
        assert_eq!(b, 1, "state 3 → blocked=1");
    }
}

// ---------------------------------------------------------------------------
// Test 5 – SyncFailed event structure
// ---------------------------------------------------------------------------

#[test]
fn sync_failed_event_contains_required_fields() {
    let event = EngineEvent::SyncFailed {
        service: "anilist".to_string(),
        anime_id: 42,
        message: "test error".to_string(),
    };

    // Verify the variant pattern.
    match &event {
        EngineEvent::SyncFailed {
            service,
            anime_id,
            message,
        } => {
            assert_eq!(service, "anilist");
            assert_eq!(*anime_id, 42);
            assert_eq!(message, "test error");
        }
        other => panic!("expected SyncFailed, got {other:?}"),
    }

    // Verify Clone + Debug + PartialEq.
    let cloned = event.clone();
    assert_eq!(event, cloned);

    // Verify Serialize/Deserialize roundtrip.
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: EngineEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, deserialized);
}

// ---------------------------------------------------------------------------
// Test 6 – sync queue handles multiple rows with mixed retry states
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multiple_rows_mixed_retry_states() {
    let storage = new_storage().await;

    // Insert anime rows first (FK constraint).
    for id in [1i64, 2, 3] {
        storage
            .upsert_anime(id, &format!(r#"{{"romaji":"Test {id}"}}"#), 12, None, 1000)
            .await
            .unwrap();
    }

    // Row A: fresh (retry_count=0)
    let id_a = storage
        .queue_sync(1, "anilist", "progress_update", r#"{"episode":1}"#, 1000)
        .await
        .unwrap();

    // Row B: failed once (retry_count=1, past next_retry_at)
    let id_b = storage
        .queue_sync(2, "anilist", "progress_update", r#"{"episode":2}"#, 1000)
        .await
        .unwrap();
    storage
        .update_sync_retry(id_b, 1, past_ts())
        .await
        .unwrap();

    // Row C: blocked (retry_count=3, future next_retry_at)
    let id_c = storage
        .queue_sync(3, "anilist", "progress_update", r#"{"episode":3}"#, 1000)
        .await
        .unwrap();
    storage
        .update_sync_retry(id_c, 3, i64::MAX)
        .await
        .unwrap();

    // fetch_pending_sync_rows returns A (fresh) and B (past next_retry) but NOT C.
    let rows = storage.fetch_pending_sync_rows("anilist", 10).await.unwrap();
    let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    assert!(ids.contains(&id_a), "fresh row should be pending");
    assert!(ids.contains(&id_b), "failed row with past next_retry should be pending");
    assert!(!ids.contains(&id_c), "blocked row should NOT be pending");

    // Status counts: pending=A, failed=B, blocked=C.
    let (pending, failed, blocked) = storage.sync_status_counts("anilist").await.unwrap();
    assert_eq!(pending, 1);
    assert_eq!(failed, 1);
    assert_eq!(blocked, 1);
}
