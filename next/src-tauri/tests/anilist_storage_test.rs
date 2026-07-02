use anivault_core::engine::storage::Storage;

async fn new_storage() -> Storage {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage
}

#[tokio::test]
async fn upsert_anime_inserts_row() {
    let storage = new_storage().await;
    storage
        .upsert_anime(1, r#"{"romaji":"Cowboy Bebop"}"#, 26, Some("https://img.jpg"), 1000)
        .await
        .unwrap();

    let row = storage.fetch_anime(1).await.unwrap().unwrap();
    assert_eq!(row.id, 1);
}

#[tokio::test]
async fn list_entry_full_roundtrip() {
    let storage = new_storage().await;
    storage
        .upsert_anime(1, r#"{"romaji":"Cowboy Bebop"}"#, 26, None, 1000)
        .await
        .unwrap();

    storage
        .upsert_list_entry_full(1, "Completed", 26, Some(85), "Great show", 2000, 3000)
        .await
        .unwrap();

    let entry = storage.get_list_entry_full(1).await.unwrap().unwrap();
    assert_eq!(entry.anime_id, 1);
    assert_eq!(entry.status, "Completed");
    assert_eq!(entry.watched_episodes, 26);
    assert_eq!(entry.score, Some(85));
    assert_eq!(entry.notes, Some("Great show".to_string()));
    assert_eq!(entry.local_updated, 2000);
    assert_eq!(entry.remote_updated, Some(3000));
}

#[tokio::test]
async fn tracker_mapping_upsert_idempotent() {
    let storage = new_storage().await;
    storage
        .upsert_anime(1, r#"{"romaji":"Test"}"#, 12, None, 1000)
        .await
        .unwrap();

    storage
        .upsert_tracker_mapping(1, "anilist", "99")
        .await
        .unwrap();
    // Second insert with same PK should not error (INSERT OR IGNORE).
    storage
        .upsert_tracker_mapping(1, "anilist", "99")
        .await
        .unwrap();
}

#[tokio::test]
async fn sync_queue_lifecycle() {
    let storage = new_storage().await;
    storage
        .upsert_anime(1, r#"{"romaji":"Test"}"#, 12, None, 1000)
        .await
        .unwrap();

    // Queue a sync row.
    storage
        .queue_sync(1, "anilist", "progress_update", r#"{"episode":5}"#, 1000)
        .await
        .unwrap();

    // Fetch pending should return 1 row.
    let rows = storage
        .fetch_pending_sync_rows("anilist", 10)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].anime_id, 1);
    assert_eq!(rows[0].retry_count, 0);
    assert!(rows[0].next_retry_at.is_none());

    // Delete and verify empty.
    storage.delete_sync_row(rows[0].id).await.unwrap();
    let rows = storage
        .fetch_pending_sync_rows("anilist", 10)
        .await
        .unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn sync_status_counts_work() {
    let storage = new_storage().await;
    storage
        .upsert_anime(1, r#"{"romaji":"Test"}"#, 12, None, 1000)
        .await
        .unwrap();

    // Queue one sync row — pending = 1, failed = 0, blocked = 0.
    storage
        .queue_sync(1, "anilist", "progress_update", r#"{"episode":5}"#, 1000)
        .await
        .unwrap();

    let (pending, failed, blocked) = storage.sync_status_counts("anilist").await.unwrap();
    assert_eq!(pending, 1);
    assert_eq!(failed, 0);
    assert_eq!(blocked, 0);
}
