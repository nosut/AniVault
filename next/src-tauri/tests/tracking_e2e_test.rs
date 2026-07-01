use anivault_core::engine::events::MediaDetected;
use anivault_core::engine::orchestrator::handle_media_detected;
use anivault_core::engine::recognition::matcher::build_fts_index;
use anivault_core::engine::storage::Storage;

#[tokio::test]
async fn media_detected_advances_local_progress_once() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.ensure_fts_index().await.unwrap();
    storage.insert_minimal_anime(1, "Spy x Family").await.unwrap();
    build_fts_index(&storage).await.unwrap();

    let detected = MediaDetected {
        player_name: "mpv.exe".to_string(),
        file_path: Some("D:/Anime/[SubsPlease] Spy x Family - 17.mkv".to_string()),
        window_title: None,
        detected_at_unix: 1_782_769_008,
    };

    let matched = handle_media_detected(&storage, detected.clone()).await.unwrap();
    handle_media_detected(&storage, detected).await.unwrap();

    assert_eq!(matched.unwrap().anime_id, 1);
    assert_eq!(storage.watch_history_count(1, 17).await.unwrap(), 1);
    assert_eq!(storage.anime_by_id(1).await.unwrap().unwrap().2, 17);
}

#[tokio::test]
async fn media_detected_without_episode_does_not_update_progress() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.ensure_fts_index().await.unwrap();
    storage.insert_minimal_anime(1, "Mushishi").await.unwrap();
    build_fts_index(&storage).await.unwrap();

    let detected = MediaDetected {
        player_name: "vlc.exe".to_string(),
        file_path: Some("D:/Anime/Mushishi.mkv".to_string()),
        window_title: None,
        detected_at_unix: 1_782_769_009,
    };

    let matched = handle_media_detected(&storage, detected).await.unwrap();

    assert!(matched.is_none());
    assert_eq!(storage.watch_history_count(1, 1).await.unwrap(), 0);
    assert_eq!(storage.anime_by_id(1).await.unwrap().unwrap().2, 0);
}
