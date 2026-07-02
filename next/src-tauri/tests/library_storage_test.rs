use anivault_core::engine::storage::Tests;

#[tokio::test]
async fn search_library_finds_by_title() {
    let storage = Tests::new_in_memory().await;

    storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();
    storage
        .upsert_list_entry_full(1, "watching", 5, None, "", 1000, 1000)
        .await
        .unwrap();

    let results = storage
        .search_library("bebop", None, 10, 0)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].title.contains("Cowboy"));
    assert_eq!(results[0].status, "watching");
    assert_eq!(results[0].watched_episodes, 5);
}

#[tokio::test]
async fn library_stats_counts_statuses() {
    let storage = Tests::new_in_memory().await;

    for id in 1..=3 {
        storage
            .insert_minimal_anime(id, &format!("Anime {}", id))
            .await
            .unwrap();
    }

    storage
        .upsert_list_entry_full(1, "watching", 5, None, "", 1000, 1000)
        .await
        .unwrap();
    storage
        .upsert_list_entry_full(2, "watching", 10, None, "", 1000, 1000)
        .await
        .unwrap();
    storage
        .upsert_list_entry_full(3, "completed", 26, None, "", 1000, 1000)
        .await
        .unwrap();

    let stats = storage.library_stats().await.unwrap();

    assert_eq!(stats.total, 3);
    assert_eq!(stats.watching, 2);
    assert_eq!(stats.completed, 1);
    assert_eq!(stats.on_hold, 0);
    assert_eq!(stats.dropped, 0);
    assert_eq!(stats.plan_to_watch, 0);
}

#[tokio::test]
async fn anime_detail_returns_full_row() {
    let storage = Tests::new_in_memory().await;

    storage
        .upsert_anime(
            1,
            r#"{"romaji":"Cowboy Bebop"}"#,
            26,
            Some("https://img.jpg"),
            1000,
        )
        .await
        .unwrap();

    storage
        .upsert_list_entry_full(1, "watching", 5, Some(85), "Great", 2000, 3000)
        .await
        .unwrap();

    storage
        .upsert_tracker_mapping(1, "anilist", "42")
        .await
        .unwrap();

    let detail = storage.anime_detail(1).await.unwrap();

    assert_eq!(detail.anime_id, 1);
    assert_eq!(detail.list_status, Some("watching".to_string()));
    assert_eq!(detail.tracker_id, Some("42".to_string()));
    assert_eq!(detail.episode_count, Some(26));
    assert_eq!(detail.score, Some(85));
    assert_eq!(detail.watched_episodes, Some(5));
}

#[tokio::test]
async fn update_list_entry_partial_changes_only_status() {
    let storage = Tests::new_in_memory().await;

    storage.insert_minimal_anime(1, "Test").await.unwrap();

    // Insert initial list entry with watching + 5 episodes via the partial method (UPSERT)
    storage
        .update_list_entry_partial(1, Some("watching"), Some(5), None)
        .await
        .unwrap();

    // Change only status to completed, keep watched_episodes and score unchanged
    storage
        .update_list_entry_partial(1, Some("completed"), None, None)
        .await
        .unwrap();

    // Fetch via existing public method
    let entry = storage.get_list_entry(1).await.unwrap().unwrap();
    assert_eq!(entry.status, "completed");
    assert_eq!(entry.watched_episodes, 5);
}
