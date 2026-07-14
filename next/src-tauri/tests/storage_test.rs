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

#[tokio::test]
async fn get_file_index_by_filename_returns_none_when_ambiguous() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Show A Season 1").await.unwrap();
    storage.insert_minimal_anime(2, "Show A Season 2").await.unwrap();

    // Two different shows, each with an episode file that happens to share
    // the exact same basename — a real-world case with generic numbering.
    storage
        .upsert_file_index("D:/Anime/Show A S1/01.mkv", Some(1), 1, 100, 1_782_769_000)
        .await
        .unwrap();
    storage
        .upsert_file_index("D:/Anime/Show A S2/01.mkv", Some(2), 1, 100, 1_782_769_001)
        .await
        .unwrap();

    let result = storage.get_file_index_by_filename("01.mkv").await.unwrap();

    assert!(
        result.is_none(),
        "an ambiguous basename match (two different anime_ids) must not silently pick a winner, got {:?}",
        result
    );
}

#[tokio::test]
async fn get_file_index_by_filename_still_resolves_unambiguous_match() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();
    storage
        .upsert_file_index("D:/Anime/Cowboy Bebop - 01.mkv", Some(1), 1, 100, 1_782_769_000)
        .await
        .unwrap();

    let result = storage
        .get_file_index_by_filename("Cowboy Bebop - 01.mkv")
        .await
        .unwrap();

    assert_eq!(result.map(|r| r.anime_id), Some(Some(1)));
}
