use anivault_core::engine::anilist::import::merge_entry;
use anivault_core::engine::storage::Tests;

#[tokio::test]
async fn merge_anilist_wins_when_newer() {
    let storage = Tests::new_in_memory().await;

    storage
        .upsert_anime(1, r#"{"romaji":"Cowboy Bebop"}"#, 26, None, 500)
        .await
        .unwrap();

    storage
        .upsert_list_entry_full(1, "watching", 5, Some(80), "ok", 500, 0)
        .await
        .unwrap();

    let merged = merge_entry(&storage, 1, "watching", Some(10), Some(90), "great", 2000)
        .await
        .unwrap();
    assert!(merged);

    let entry = storage.get_list_entry_full(1).await.unwrap().unwrap();
    assert_eq!(entry.watched_episodes, 10);
}

#[tokio::test]
async fn local_wins_when_newer() {
    let storage = Tests::new_in_memory().await;

    storage
        .upsert_anime(1, r#"{"romaji":"Cowboy Bebop"}"#, 26, None, 3000)
        .await
        .unwrap();

    storage
        .upsert_list_entry_full(1, "watching", 7, Some(80), "ok", 3000, 0)
        .await
        .unwrap();

    let merged = merge_entry(&storage, 1, "watching", Some(5), Some(85), "new notes", 2000)
        .await
        .unwrap();
    assert!(!merged);

    let entry = storage.get_list_entry_full(1).await.unwrap().unwrap();
    assert_eq!(entry.watched_episodes, 7);
}
