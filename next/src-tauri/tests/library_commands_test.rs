use taiga_next::engine::runtime::fresh_test_state;

#[tokio::test]
async fn search_library_returns_matches() {
    let state = fresh_test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Cowboy Bebop")
        .await
        .unwrap();
    state
        .storage
        .upsert_list_entry_full(1, "watching", 5, None, "", 0, 0)
        .await
        .unwrap();
    let result =
        taiga_next::commands::search_library_inner("bebop".into(), None, 10, 0, &state)
            .await
            .unwrap();
    assert_eq!(result.len(), 1);
}

#[tokio::test]
async fn get_library_stats_returns_counts() {
    let state = fresh_test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "A")
        .await
        .unwrap();
    state
        .storage
        .upsert_list_entry_full(1, "watching", 3, None, "", 0, 0)
        .await
        .unwrap();
    let stats = taiga_next::commands::get_library_stats_inner(&state)
        .await
        .unwrap();
    assert!(stats.watching >= 1);
}

#[tokio::test]
async fn fetch_anime_detail_returns_data() {
    let state = fresh_test_state().await;
    state
        .storage
        .upsert_anime(1, r#"{"romaji":"Test"}"#, 12, None, 0)
        .await
        .unwrap();
    state
        .storage
        .upsert_list_entry_full(1, "completed", 12, Some(8), "", 0, 0)
        .await
        .unwrap();
    let detail = taiga_next::commands::fetch_anime_detail_inner(1, &state)
        .await
        .unwrap();
    assert_eq!(detail.list_status, Some("completed".to_string()));
    assert_eq!(detail.score, Some(8));
}

#[tokio::test]
async fn update_list_entry_edits_progress() {
    let state = fresh_test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Test")
        .await
        .unwrap();
    state
        .storage
        .upsert_list_entry_full(1, "watching", 3, None, "", 0, 0)
        .await
        .unwrap();
    taiga_next::commands::update_list_entry_inner(1, None, Some(7), None, &state)
        .await
        .unwrap();
    let entry = state.storage.get_list_entry(1).await.unwrap().unwrap();
    assert_eq!(entry.watched_episodes, 7);
}
