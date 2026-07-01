use taiga_next::engine::storage::Storage;

#[tokio::test]
async fn search_anime_by_title_exact() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();
    storage.insert_minimal_anime(2, "Cowboy Bebop: The Movie").await.unwrap();

    let results = storage.search_anime_by_title("Cowboy Bebop", 10).await.unwrap();
    assert!(results.len() >= 1);
    assert_eq!(results[0].id, 1);
}

#[tokio::test]
async fn search_anime_by_partial_title() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Attack on Titan").await.unwrap();
    storage.insert_minimal_anime(2, "Fullmetal Alchemist").await.unwrap();

    let results = storage.search_anime_by_title("Alchemist", 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 2);
}
