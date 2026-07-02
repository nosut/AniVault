use taiga_next::engine::anilist::auth;
use taiga_next::engine::runtime::EngineState;

async fn test_state() -> EngineState {
    taiga_next::engine::runtime::fresh_test_state().await
}

#[tokio::test]
async fn sync_status_returns_zeros_when_empty() {
    let state = test_state().await;
    let status = taiga_next::commands::get_sync_status_inner(&state)
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
    taiga_next::commands::disconnect_anilist_inner(&state)
        .await
        .unwrap();
    let token = auth::load_token(&state.storage).await.unwrap();
    assert_eq!(token, None);
}
