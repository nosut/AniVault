use anivault_core::commands::{
    confirm_identification_inner, identify_file_inner, list_known_files_inner,
};
use anivault_core::engine::runtime::fresh_test_state;

async fn test_state() -> anivault_core::engine::runtime::EngineState {
    fresh_test_state().await
}

#[tokio::test]
async fn identify_empty_file_returns_no_candidates() {
    let state = test_state().await;
    let result = identify_file_inner("unknown_file.mp4", None, &state)
        .await
        .unwrap();
    assert!(!result.known_file);
    assert!(result.candidates.is_empty());
}

#[tokio::test]
async fn identify_and_confirm_remembers_mapping() {
    let state = test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Cowboy Bebop")
        .await
        .unwrap();

    let file_path = "D:/Anime/Cowboy Bebop - 01.mkv";

    let result = identify_file_inner(file_path, None, &state)
        .await
        .unwrap();
    assert!(!result.known_file);
    assert!(result.candidates.iter().any(|c| c.anime_id == 1));

    confirm_identification_inner(file_path, 1, 1, &state).await.unwrap();

    // Re-identify — should be known now
    let result2 = identify_file_inner(file_path, None, &state).await.unwrap();
    assert!(result2.known_file);
}

#[tokio::test]
async fn list_known_files_after_confirmation() {
    let state = test_state().await;
    state
        .storage
        .insert_minimal_anime(99, "Test Series")
        .await
        .unwrap();

    // No files yet
    let files = list_known_files_inner(10, &state).await.unwrap();
    assert!(files.is_empty());

    // Confirm a file
    let file_path = "D:/Anime/Test Series - 05.mkv";
    confirm_identification_inner(file_path, 99, 5, &state)
        .await
        .unwrap();

    // Now it shows up
    let files = list_known_files_inner(10, &state).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file_path, file_path);
    assert_eq!(files[0].anime_id, Some(99));
    assert_eq!(files[0].episode, Some(5));
}

#[tokio::test]
async fn confirming_a_window_title_records_progress_and_history() {
    let state = anivault_core::engine::runtime::fresh_test_state().await;
    state
        .storage
        .insert_minimal_anime(77, "Frieren")
        .await
        .unwrap();

    // mpv/VLC report a window title, not a path.
    anivault_core::commands::confirm_identification_inner(
        "[SubsPlease] Frieren - 04 (1080p).mkv - mpv",
        77,
        4,
        &state,
    )
    .await
    .unwrap();

    let entry = state.storage.get_list_entry(77).await.unwrap().unwrap();
    assert_eq!(entry.watched_episodes, 4);
    assert_eq!(state.storage.watch_history_count(77, 4).await.unwrap(), 1);
    assert!(state.events.drain().iter().any(|e| matches!(
        e,
        anivault_core::engine::events::EngineEvent::ProgressAdvanced { anime_id: 77, new_episode: 4, .. }
    )));

    // Confirming the same episode again does not duplicate the history row.
    anivault_core::commands::confirm_identification_inner(
        "[SubsPlease] Frieren - 04 (1080p).mkv - mpv",
        77,
        4,
        &state,
    )
    .await
    .unwrap();
    assert_eq!(state.storage.watch_history_count(77, 4).await.unwrap(), 1);
}
