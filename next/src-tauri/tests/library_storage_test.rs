use taiga_next::engine::storage::Tests;

#[tokio::test]
async fn search_library_finds_by_title() {
    let storage = Tests::new_in_memory().await;
    storage
        .upsert_anime(1, r#"{"romaji":"Cowboy Bebop"}"#, 26, None, 1000)
        .await
        .unwrap();

    let results = storage.search_library("bebop", None, 10, 0).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].anime_id, 1);
    assert_eq!(results[0].title, "Cowboy Bebop");
    assert_eq!(results[0].status, "unlisted");
}

#[tokio::test]
async fn library_stats_counts_statuses() {
    let storage = Tests::new_in_memory().await;
    storage
        .upsert_anime(1, r#"{"romaji":"Anime 1"}"#, 12, None, 1000)
        .await
        .unwrap();
    storage
        .upsert_anime(2, r#"{"romaji":"Anime 2"}"#, 24, None, 1000)
        .await
        .unwrap();
    storage
        .upsert_anime(3, r#"{"romaji":"Anime 3"}"#, 26, None, 1000)
        .await
        .unwrap();

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
        .upsert_list_entry_full(1, "watching", 5, Some(80), "Good show", 2000, 3000)
        .await
        .unwrap();
    storage
        .upsert_tracker_mapping(1, "anilist", "99")
        .await
        .unwrap();

    let detail = storage.anime_detail(1).await.unwrap().unwrap();
    assert_eq!(detail.anime_id, 1);
    assert_eq!(detail.titles_json, r#"{"romaji":"Cowboy Bebop"}"#);
    assert_eq!(detail.episode_count, Some(26));
    assert_eq!(detail.image_url, Some("https://img.jpg".to_string()));
    assert_eq!(detail.list_status, Some("watching".to_string()));
    assert_eq!(detail.watched_episodes, Some(5));
    assert_eq!(detail.score, Some(80));
    assert_eq!(detail.notes, Some("Good show".to_string()));
    assert_eq!(detail.tracker_id, Some("99".to_string()));
}

#[tokio::test]
async fn update_list_entry_partial_changes_only_status() {
    let storage = Tests::new_in_memory().await;
    storage
        .upsert_anime(1, r#"{"romaji":"Test"}"#, 12, None, 1000)
        .await
        .unwrap();
    storage
        .upsert_list_entry_full(1, "watching", 5, None, "", 1000, 1000)
        .await
        .unwrap();

    storage
        .update_list_entry_partial(1, Some("completed"), None, None)
        .await
        .unwrap();

    let entry = storage.get_list_entry_full(1).await.unwrap().unwrap();
    assert_eq!(entry.status, "completed");
    assert_eq!(entry.watched_episodes, 5);
}
