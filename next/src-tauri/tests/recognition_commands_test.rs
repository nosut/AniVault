use taiga_next::commands::{
    confirm_identification_inner, identify_file_inner, list_known_files_inner,
};
use taiga_next::engine::runtime::fresh_test_state;

async fn test_state() -> taiga_next::engine::runtime::EngineState {
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

    let result = identify_file_inner("Cowboy Bebop - 01.mkv", None, &state)
        .await
        .unwrap();
    assert!(!result.known_file);
    assert!(result.candidates.iter().any(|c| c.anime_id == 1));

    confirm_identification_inner("Cowboy Bebop - 01.mkv", 1, 1, &state)
        .await
        .unwrap();

    // Re-identify — should be known now
    let result2 = identify_file_inner("Cowboy Bebop - 01.mkv", None, &state)
        .await
        .unwrap();
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
    confirm_identification_inner("Test Series - 05.mkv", 99, 5, &state)
        .await
        .unwrap();

    // Now it shows up
    let files = list_known_files_inner(10, &state).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].file_path, "Test Series - 05.mkv");
    assert_eq!(files[0].anime_id, Some(99));
    assert_eq!(files[0].episode, Some(5));
}
