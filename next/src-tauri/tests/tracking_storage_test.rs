use taiga_next::engine::storage::Storage;

#[tokio::test]
async fn fetch_anime_returns_none_for_missing_id() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    assert!(storage.fetch_anime(999).await.unwrap().is_none());
}

#[tokio::test]
async fn fetch_anime_returns_row_for_existing_anime() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();

    let row = storage.fetch_anime(1).await.unwrap().unwrap();
    assert_eq!(row.id, 1);
    assert!(row.titles_json.contains("Cowboy Bebop"));
}

#[tokio::test]
async fn upsert_list_entry_progress_increments_watched_count() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Test").await.unwrap();

    storage
        .upsert_list_entry_progress(1, "Watching", 3, 1_782_769_008)
        .await
        .unwrap();

    let entry = storage.get_list_entry(1).await.unwrap().unwrap();
    assert_eq!(entry.watched_episodes, 3);
    assert_eq!(entry.status, "Watching");

    storage
        .upsert_list_entry_progress(1, "Watching", 4, 1_782_769_009)
        .await
        .unwrap();

    let entry = storage.get_list_entry(1).await.unwrap().unwrap();
    assert_eq!(entry.watched_episodes, 4);
}

#[tokio::test]
async fn list_recent_watch_history_returns_empty_when_no_history() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let rows = storage.list_recent_watch_history(10).await.unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn list_recent_watch_history_returns_most_recent_first() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();

    storage
        .append_watch_history(1, 1, None, Some("mpv"), 1_782_769_000)
        .await
        .unwrap();
    storage
        .append_watch_history(1, 2, None, Some("mpv"), 1_782_769_100)
        .await
        .unwrap();

    let rows = storage.list_recent_watch_history(5).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].episode, 2); // most recent first
    assert_eq!(rows[1].episode, 1);
}
