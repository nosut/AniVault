use anivault_core::engine::storage::Storage;

#[tokio::test]
async fn library_returns_all_anime_with_progress() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    storage.insert_minimal_anime(1, "Spy x Family").await.unwrap();
    storage.insert_minimal_anime(2, "Frieren").await.unwrap();
    storage.insert_minimal_anime(3, "Dandadan").await.unwrap();

    storage.update_watched_episodes(1, 12).await.unwrap();
    storage.update_watched_episodes(2, 7).await.unwrap();

    let library = storage.get_library_anime().await.unwrap();
    assert_eq!(library.len(), 3);

    let spy = library.iter().find(|a| a.id == 1).unwrap();
    assert_eq!(spy.title, "Spy x Family");
    assert_eq!(spy.watched_episodes, 12);
    assert_eq!(spy.status, "watching");

    let frieren = library.iter().find(|a| a.id == 2).unwrap();
    assert_eq!(frieren.watched_episodes, 7);

    let dandadan = library.iter().find(|a| a.id == 3).unwrap();
    assert_eq!(dandadan.watched_episodes, 0);
    assert_eq!(dandadan.status, "plan_to_watch");
}
