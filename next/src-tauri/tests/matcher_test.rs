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
            Some(1),
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

#[tokio::test]
async fn recognize_file_ranks_candidates_by_score_including_synonyms() {
    let state = test_state().await;
    let titles = serde_json::json!({
        "romaji": "Koe no Katachi",
        "english": "A Silent Voice",
        "japanese": null,
        "synonyms": ["Silent Voice"]
    })
    .to_string();
    state
        .storage
        .upsert_anime(1, &titles, 1, None, 0)
        .await
        .unwrap();

    // Matches only via the synonym, not romaji/english — this exercises the
    // synonym branch that a naive port of the old inline scoring could drop.
    let result = recognize_file("D:/Anime/Silent Voice - 01.mkv", None, &state.storage)
        .await
        .unwrap();

    assert!(
        result.candidates.iter().any(|c| c.anime_id == 1 && c.confidence >= 80),
        "expected a high-confidence synonym match, got {:?}",
        result.candidates
    );
}
