use anivault_core::engine::matcher::{recognize_file, confirm_identification};
use anivault_core::engine::runtime::{fresh_test_state, EngineState};

async fn test_state() -> EngineState {
    fresh_test_state().await
}

#[tokio::test]
async fn recognize_known_file_skips_matching() {
    let state = test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Cowboy Bebop")
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            "D:/Anime/Cowboy Bebop - 01.mkv",
            1,
            1,
            100,
            1_782_769_008,
        )
        .await
        .unwrap();

    let result =
        recognize_file("D:/Anime/Cowboy Bebop - 01.mkv", None, &state.storage)
            .await
            .unwrap();
    assert!(result.known_file);
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].anime_id, 1);
    assert_eq!(result.candidates[0].confidence, 100);
}

#[tokio::test]
async fn recognize_new_file_parses_and_searches() {
    let state = test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Mushishi")
        .await
        .unwrap();

    // Use full path — strip_path in matcher handles directory stripping
    let result =
        recognize_file("D:/Anime/Mushishi - 07.mkv", None, &state.storage)
            .await
            .unwrap();
    assert!(!result.known_file);
    assert_eq!(
        result.parsed.as_ref().unwrap().episode_number,
        7
    );
    assert!(
        result.candidates.iter().any(|c| c.anime_id == 1),
        "Should find candidate with anime_id 1, got candidates: {:?}",
        result.candidates
    );
}

#[tokio::test]
async fn confirm_identification_upserts_file_index() {
    let state = test_state().await;
    state
        .storage
        .insert_minimal_anime(42, "Test Anime")
        .await
        .unwrap();

    confirm_identification(&state, "D:/Anime/Test - 03.mkv", 42, 3)
        .await
        .unwrap();

    let idx = state
        .storage
        .get_file_index("D:/Anime/Test - 03.mkv")
        .await
        .unwrap()
        .expect("file index should exist");
    assert_eq!(idx.anime_id, Some(42));
    assert_eq!(idx.episode, Some(3));
    assert_eq!(idx.confidence, 100);

    let events = state.events.drain();
    assert!(events.iter().any(|e| matches!(
        e,
        anivault_core::engine::events::EngineEvent::AnimeIdentified(ev)
            if ev.anime_id == 42 && ev.episode == 3
    )));
}
