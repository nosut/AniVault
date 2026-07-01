use anivault_core::engine::storage::Storage;
use anivault_core::engine::sync::{queue_sync_results, pending_sync_batch, complete_sync_item, reschedule_sync_item, backoff_delay};

#[tokio::test]
async fn queue_and_dequeue_sync_items() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Spy x Family").await.unwrap();

    let now = 1_700_000_000;
    queue_sync_results(&storage, 1, 17, 1, now).await.unwrap();

    let pending = pending_sync_batch(&storage, "anilist", 10, now).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].anime_id, 1);
    assert_eq!(pending[0].operation, "update_progress");
}

#[tokio::test]
async fn complete_sync_removes_item() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();

    let now = 1_700_000_000;
    queue_sync_results(&storage, 1, 3, 1, now).await.unwrap();

    let pending = pending_sync_batch(&storage, "anilist", 10, now).await.unwrap();
    complete_sync_item(&storage, pending[0].queue_id).await.unwrap();

    let after = pending_sync_batch(&storage, "anilist", 10, now).await.unwrap();
    assert!(after.is_empty());
}

#[tokio::test]
async fn reschedule_backoff_delays_retry() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Dandadan").await.unwrap();

    let now = 1_700_000_000;
    queue_sync_results(&storage, 1, 1, 1, now).await.unwrap();

    let pending = pending_sync_batch(&storage, "anilist", 10, now).await.unwrap();
    reschedule_sync_item(&storage, pending[0].queue_id, now).await.unwrap();

    let after = pending_sync_batch(&storage, "anilist", 10, now).await.unwrap();
    assert!(after.is_empty(), "should not be retryable immediately");

    let later = pending_sync_batch(&storage, "anilist", 10, now + 60).await.unwrap();
    assert_eq!(later.len(), 1, "should be retryable after backoff");
}

#[test]
fn backoff_increments_with_retries() {
    let d0 = backoff_delay(0);
    let d1 = backoff_delay(1);
    let d3 = backoff_delay(3);

    assert_eq!(d0, 30);
    assert_eq!(d1, 60);
    assert!(d3 > d1, "delay should increase with retries, got {d3} > {d1}");
    assert!(d3 <= 21600, "max backoff is 6 hours, got {d3}");
}
