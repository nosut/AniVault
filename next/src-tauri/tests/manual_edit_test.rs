use anivault_core::engine::storage::Storage;

#[tokio::test]
async fn manual_edit_sets_exact_episode() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Spy x Family").await.unwrap();
    storage.update_watched_episodes(1, 12).await.unwrap();

    // Manually set to 10 (lower than 12)
    storage.set_watched_episodes(1, 10).await.unwrap();

    let (_, _, eps) = storage.anime_by_id(1).await.unwrap().unwrap();
    assert_eq!(eps, 10, "manual edit should set exact episode, not max");

    let count = storage.watch_history_count(1, 10).await.unwrap();
    assert_eq!(count, 1, "manual edit should create watch history entry");
}

#[tokio::test]
async fn manual_edit_can_increase_episode() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Frieren").await.unwrap();
    storage.update_watched_episodes(1, 3).await.unwrap();

    storage.set_watched_episodes(1, 14).await.unwrap();

    let (_, _, eps) = storage.anime_by_id(1).await.unwrap().unwrap();
    assert_eq!(eps, 14);

    let count = storage.watch_history_count(1, 14).await.unwrap();
    assert_eq!(count, 1);
}
