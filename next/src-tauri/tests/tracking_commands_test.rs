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

#[tokio::test]
async fn a_stale_loop_finishing_does_not_clear_a_newer_loop() {
    use anivault_core::engine::tracker::finish_tracking_loop;
    let state = fresh_test_state().await;

    start_tracking_inner(&state).await.unwrap();
    let first = state.tracking.lock().unwrap().generation;
    stop_tracking_inner(&state).await.unwrap();
    start_tracking_inner(&state).await.unwrap();
    let second = state.tracking.lock().unwrap().generation;
    assert_ne!(first, second, "each start gets its own generation");

    // The first loop only notices its cancel after its sleep, then cleans up.
    finish_tracking_loop(&state, first);

    let status = get_tracking_status_inner(&state).await.unwrap();
    assert!(status.active, "the newer loop is still running");

    finish_tracking_loop(&state, second);
    assert!(!get_tracking_status_inner(&state).await.unwrap().active);
}

#[tokio::test]
async fn a_panicking_loop_still_resets_tracking_state() {
    use anivault_core::engine::tracker::TrackingLoopGuard;
    let state = fresh_test_state().await;
    start_tracking_inner(&state).await.unwrap();
    let generation = state.tracking.lock().unwrap().generation;

    let s = state.clone();
    let joined = tokio::spawn(async move {
        let _guard = TrackingLoopGuard::new(s, generation);
        panic!("boom inside the tracking loop");
    })
    .await;
    assert!(joined.is_err(), "the task panicked");

    assert!(!get_tracking_status_inner(&state).await.unwrap().active);
}

#[tokio::test]
async fn set_tracking_enabled_persists_and_starts_or_stops_the_loop() {
    use anivault_core::commands::set_tracking_enabled_inner;
    let state = fresh_test_state().await;

    let status = set_tracking_enabled_inner(false, &state).await.unwrap();
    assert!(!status.active);
    assert_eq!(
        state.storage.get_setting("tracking.enabled").await.unwrap().as_deref(),
        Some("false")
    );

    let status = set_tracking_enabled_inner(true, &state).await.unwrap();
    assert!(status.active);
    assert!(get_tracking_status_inner(&state).await.unwrap().active);
    assert_eq!(
        state.storage.get_setting("tracking.enabled").await.unwrap().as_deref(),
        Some("true")
    );

    let status = set_tracking_enabled_inner(false, &state).await.unwrap();
    assert!(!status.active);
}
