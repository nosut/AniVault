use anivault_core::engine::anilist::auth;
use anivault_core::engine::runtime::EngineState;

async fn test_state() -> EngineState {
    anivault_core::engine::runtime::fresh_test_state().await
}

#[tokio::test]
async fn sync_status_returns_zeros_when_empty() {
    let state = test_state().await;
    let status = anivault_core::commands::get_sync_status_inner(&state)
        .await
        .unwrap();
    assert_eq!(status.pending, 0);
    assert_eq!(status.failed, 0);
    assert_eq!(status.blocked, 0);
}

#[tokio::test]
async fn disconnect_clears_token() {
    let state = test_state().await;
    auth::store_token(&state.storage, "x").await.unwrap();
    anivault_core::commands::disconnect_anilist_inner(&state)
        .await
        .unwrap();
    let token = auth::load_token(&state.storage).await.unwrap();
    assert_eq!(token, None);
}

#[tokio::test]
async fn disconnect_clears_the_sync_queue() {
    let state = test_state().await;
    auth::store_token(&state.storage, "x").await.unwrap();
    state
        .storage
        .upsert_anime(5, r#"{"romaji":"T5"}"#, 12, None, 1000)
        .await
        .unwrap();
    state
        .storage
        .queue_sync(5, "anilist", "update", "{}", 1000)
        .await
        .unwrap();

    anivault_core::commands::disconnect_anilist_inner(&state)
        .await
        .unwrap();

    assert_eq!(state.storage.pending_sync_count("anilist").await.unwrap(), 0);
}

#[tokio::test]
async fn enqueued_payload_includes_the_score() {
    let state = test_state().await;
    auth::store_token(&state.storage, "x").await.unwrap();
    state
        .storage
        .upsert_anime(6, r#"{"romaji":"T6"}"#, 12, None, 1000)
        .await
        .unwrap();
    state
        .storage
        .upsert_list_entry_full(6, "completed", 12, Some(90), "", 1000, 0)
        .await
        .unwrap();

    anivault_core::engine::sync_worker::enqueue_anilist_sync(&state, 6).await;

    let rows = state
        .storage
        .fetch_pending_sync_rows("anilist", 10)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&rows[0].payload_json).unwrap();
    assert_eq!(payload["score"], 90);
    assert_eq!(payload["episode"], 12);
    assert_eq!(payload["status"], "completed");
}

#[tokio::test]
async fn connection_status_reports_an_expired_token() {
    let state = test_state().await;
    auth::store_token(&state.storage, "x").await.unwrap();
    auth::mark_token_invalid(&state.storage).await.unwrap();
    assert!(!anivault_core::commands::get_anilist_connection_status_inner(&state)
        .await
        .unwrap());
    assert!(anivault_core::commands::get_anilist_token_invalid_inner(&state)
        .await
        .unwrap());
}
