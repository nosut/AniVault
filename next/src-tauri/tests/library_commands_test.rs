use taiga_next::commands::{
    fetch_anime_detail_inner, get_library_stats_inner, search_library_inner, update_list_entry_inner,
};
use taiga_next::engine::runtime::fresh_test_state;

async fn test_state() -> taiga_next::engine::runtime::EngineState {
    fresh_test_state().await
}

#[tokio::test]
async fn search_library_returns_matches() {
    let state = test_state().await;
    state
        .storage
        .upsert_anime(1, r#"{"romaji":"Test Anime"}"#, 12, None, 1000)
        .await
        .unwrap();
    state
        .storage
        .upsert_list_entry_full(1, "watching", 5, None, "", 2000, 2000)
        .await
        .unwrap();

    let results = search_library_inner(&state, "Test".to_string(), Some("watching".to_string()), 10, 0)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].anime_id, 1);
    assert_eq!(results[0].title, "Test Anime");
    assert_eq!(results[0].status, "watching");
}

#[tokio::test]
async fn get_library_stats_returns_counts() {
    let state = test_state().await;
    state
        .storage
        .upsert_anime(1, r#"{"romaji":"Anime 1"}"#, 12, None, 1000)
        .await
        .unwrap();
    state
        .storage
        .upsert_list_entry_full(1, "watching", 5, None, "", 2000, 2000)
        .await
        .unwrap();

    let stats = get_library_stats_inner(&state).await.unwrap();
    assert!(stats.total >= 1);
    assert!(stats.watching >= 1);
}

#[tokio::test]
async fn fetch_anime_detail_returns_row() {
    let state = test_state().await;
    state
        .storage
        .upsert_anime(1, r#"{"romaji":"Detail Test"}"#, 24, None, 1000)
        .await
        .unwrap();
    state
        .storage
        .upsert_list_entry_full(1, "completed", 24, Some(90), "Perfect", 3000, 4000)
        .await
        .unwrap();

    let detail = fetch_anime_detail_inner(&state, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(detail.anime_id, 1);
    assert_eq!(detail.titles_json, r#"{"romaji":"Detail Test"}"#);
    assert_eq!(detail.list_status, Some("completed".to_string()));
    assert_eq!(detail.watched_episodes, Some(24));
    assert_eq!(detail.score, Some(90));
}

#[tokio::test]
async fn update_list_entry_works() {
    let state = test_state().await;
    state
        .storage
        .upsert_anime(1, r#"{"romaji":"Update Test"}"#, 12, None, 1000)
        .await
        .unwrap();
    state
        .storage
        .upsert_list_entry_full(1, "watching", 5, None, "", 2000, 2000)
        .await
        .unwrap();

    update_list_entry_inner(
        &state,
        1,
        Some("completed".to_string()),
        None,
        None,
    )
    .await
    .unwrap();

    let entry = state.storage.get_list_entry_full(1).await.unwrap().unwrap();
    assert_eq!(entry.status, "completed");
    assert_eq!(entry.watched_episodes, 5);
}
