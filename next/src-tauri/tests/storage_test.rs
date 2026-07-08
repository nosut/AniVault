use anivault_core::engine::storage::Storage;

#[tokio::test]
async fn storage_migrates_and_uses_journal_mode_supported_by_memory_sqlite() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let journal_mode = storage.journal_mode().await.unwrap();
    assert_eq!(journal_mode.to_lowercase(), "memory");
}

#[tokio::test]
async fn storage_appends_history_and_queues_sync() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();

    let history_id = storage
        .append_watch_history(1, 7, Some("D:/Anime/Cowboy Bebop 07.mkv"), Some("mpv"), "manual", 1_782_769_008)
        .await
        .unwrap();
    assert!(history_id > 0);

    let sync_id = storage
        .queue_sync(1, "anilist", "update_progress", r#"{"episode":7}"#, 1_782_769_008)
        .await
        .unwrap();
    assert!(sync_id > 0);

    let pending = storage.pending_sync_count("anilist").await.unwrap();
    assert_eq!(pending, 1);
}
