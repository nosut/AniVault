use anivault_core::commands::{
    get_tracking_status_inner, list_recent_history_inner, mark_episode_watched_inner,
    start_tracking_inner, stop_tracking_inner,
};
use anivault_core::engine::runtime::fresh_test_state;

#[tokio::test]
async fn tracking_starts_and_stops() {
    let state = fresh_test_state().await;

    let status = start_tracking_inner(&state).await.unwrap();
    assert!(status.active);
    assert!(status.watching.is_none());

    let status = stop_tracking_inner(&state).await.unwrap();
    assert!(!status.active);
}

#[tokio::test]
async fn tracking_status_returns_running_state() {
    let state = fresh_test_state().await;

    let status = get_tracking_status_inner(&state).await.unwrap();
    assert!(!status.active);

    start_tracking_inner(&state).await.unwrap();
    let status = get_tracking_status_inner(&state).await.unwrap();
    assert!(status.active);
}

#[tokio::test]
async fn mark_episode_watched_creates_history_and_updates_progress() {
    let state = fresh_test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Test")
        .await
        .unwrap();

    mark_episode_watched_inner(1, 5, &state).await.unwrap();

    let history = list_recent_history_inner(10, &state).await.unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].anime_id, 1);
    assert_eq!(history[0].episode, 5);

    let entry = state
        .storage
        .get_list_entry(1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(entry.watched_episodes, 5);
}
