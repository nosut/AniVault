use anivault_core::engine::runtime::fresh_test_state;

#[tokio::test]
async fn session_state_starts_unpaused() {
    let state = fresh_test_state().await;
    let paused = anivault_core::commands::get_session_state_inner(&state)
        .await
        .unwrap()
        .paused;
    assert!(!paused);
}

#[tokio::test]
async fn toggle_pause_flips_state() {
    let state = fresh_test_state().await;
    let after = anivault_core::commands::toggle_pause_tracking_inner(&state)
        .await
        .unwrap();
    assert!(after.paused);
    let after2 = anivault_core::commands::toggle_pause_tracking_inner(&state)
        .await
        .unwrap();
    assert!(!after2.paused);
}

#[tokio::test]
async fn launch_on_startup_setting_roundtrip() {
    let state = fresh_test_state().await;
    anivault_core::commands::set_launch_on_startup_inner(true, &state)
        .await
        .unwrap();
    let enabled = anivault_core::commands::get_launch_on_startup_inner(&state)
        .await
        .unwrap();
    assert!(enabled);
    anivault_core::commands::set_launch_on_startup_inner(false, &state)
        .await
        .unwrap();
    let disabled = anivault_core::commands::get_launch_on_startup_inner(&state)
        .await
        .unwrap();
    assert!(!disabled);
}
