use anivault_core::engine::matcher::{confirm_identification, recognize_file};
use anivault_core::engine::runtime::{fresh_test_state, EngineState};
use anivault_core::engine::storage::MappingSource;

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
            MappingSource::Manual,
            1_782_769_008,
        )
        .await
        .unwrap();

    let result = recognize_file("D:/Anime/Cowboy Bebop - 01.mkv", None, &state.storage)
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
    let result = recognize_file("D:/Anime/Mushishi - 07.mkv", None, &state.storage)
        .await
        .unwrap();
    assert!(!result.known_file);
    assert_eq!(result.parsed.as_ref().unwrap().episode_number, 7);
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
    assert_eq!(idx.mapping_source, MappingSource::Manual);

    let events = state.events.drain();
    assert!(events.iter().any(|e| matches!(
        e,
        anivault_core::engine::events::EngineEvent::AnimeIdentified(ev)
            if ev.anime_id == 42 && ev.episode == 3
    )));
}

/// The Now Playing Confirm button hands over whatever the scanner called a file
/// path, which for mpv/VLC is the window title. Indexing that would create a row
/// keyed by a string that is not a file — it can never be looked up by path, and
/// Up Next would offer it as something to play.
#[tokio::test]
async fn confirm_identification_does_not_index_a_window_title() {
    let state = test_state().await;
    state
        .storage
        .insert_minimal_anime(42, "Test Anime")
        .await
        .unwrap();

    let window_title = "Test Anime S01E03 An Episode Title - mpv";
    confirm_identification(&state, window_title, 42, 3)
        .await
        .unwrap();

    assert!(
        state
            .storage
            .get_file_index(window_title)
            .await
            .unwrap()
            .is_none(),
        "a window title must never become a file index key"
    );

    let events = state.events.drain();
    assert!(
        events.iter().any(|e| matches!(
            e,
            anivault_core::engine::events::EngineEvent::AnimeIdentified(ev)
                if ev.anime_id == 42 && ev.episode == 3
        )),
        "the identification itself is still worth publishing"
    );
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
        result
            .candidates
            .iter()
            .any(|c| c.anime_id == 1 && c.confidence >= 80),
        "expected a high-confidence synonym match, got {:?}",
        result.candidates
    );
}

/// mpv reports the mkv's *embedded* title, which carries no `.mkv` extension —
/// so neither the path lookup nor the filename fallback can fire, and the title
/// search sees only the base show name. That name is an exact (100) match on the
/// season-1 entry and only an 80 containment match on "… 2nd Season", so a
/// season-2 episode used to bind the session to season 1 — and Up Next then
/// offered S01E06 after S02E05.
#[tokio::test]
async fn embedded_title_with_a_season_marker_prefers_the_mapped_season_entry() {
    let state = test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Kusuriya no Hitorigoto")
        .await
        .unwrap();
    state
        .storage
        .insert_minimal_anime(2, "Kusuriya no Hitorigoto 2nd Season")
        .await
        .unwrap();
    for (path, anime_id) in [
        (
            "Y:/Anime/The Apothecary Diaries/Season 1/The Apothecary Diaries - S01E05 - Covert Operations.mkv",
            1,
        ),
        (
            "Y:/Anime/The Apothecary Diaries/Season 2/The Apothecary Diaries - S02E05 - The Moon Fairy.mkv",
            2,
        ),
    ] {
        state
            .storage
            .upsert_file_index(
                path,
                Some(anime_id),
                5,
                100,
                MappingSource::Manual,
                1_782_769_008,
            )
            .await
            .unwrap();
    }

    let title = "Kusuriya no Hitorigoto - S02E05 - The Moon Fairy - mpv";
    let result = recognize_file(title, Some(title), &state.storage)
        .await
        .unwrap();

    assert_eq!(result.parsed.as_ref().unwrap().season_number, Some(2));
    assert_eq!(
        result.candidates.first().map(|c| c.anime_id),
        Some(2),
        "season 2 should win, got: {:?}",
        result.candidates
    );
}

#[tokio::test]
async fn embedded_title_for_season_one_still_prefers_the_base_entry() {
    let state = test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Kusuriya no Hitorigoto")
        .await
        .unwrap();
    state
        .storage
        .insert_minimal_anime(2, "Kusuriya no Hitorigoto 2nd Season")
        .await
        .unwrap();
    for (path, anime_id) in [
        (
            "Y:/Anime/The Apothecary Diaries/Season 1/The Apothecary Diaries - S01E06 - The Garden Party.mkv",
            1,
        ),
        (
            "Y:/Anime/The Apothecary Diaries/Season 2/The Apothecary Diaries - S02E06 - The Crystal Pavilion.mkv",
            2,
        ),
    ] {
        state
            .storage
            .upsert_file_index(
                path,
                Some(anime_id),
                6,
                100,
                MappingSource::Manual,
                1_782_769_008,
            )
            .await
            .unwrap();
    }

    let title = "Kusuriya no Hitorigoto - S01E06 - The Garden Party - mpv";
    let result = recognize_file(title, Some(title), &state.storage)
        .await
        .unwrap();

    assert_eq!(
        result.candidates.first().map(|c| c.anime_id),
        Some(1),
        "season 1 should win, got: {:?}",
        result.candidates
    );
}

/// Nothing is mapped, so the season has to come from the candidates' own titles.
#[tokio::test]
async fn season_marker_falls_back_to_the_season_named_in_the_title() {
    let state = test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Kusuriya no Hitorigoto")
        .await
        .unwrap();
    state
        .storage
        .insert_minimal_anime(2, "Kusuriya no Hitorigoto 2nd Season")
        .await
        .unwrap();
    state
        .storage
        .insert_minimal_anime(3, "Kusuriya no Hitorigoto 3rd Season")
        .await
        .unwrap();

    let title = "Kusuriya no Hitorigoto - S02E05 - The Moon Fairy - mpv";
    let result = recognize_file(title, Some(title), &state.storage)
        .await
        .unwrap();

    assert_eq!(
        result.candidates.first().map(|c| c.anime_id),
        Some(2),
        "the 2nd Season entry should win, got: {:?}",
        result.candidates
    );
}

/// A filename with no season marker must rank exactly as it did before.
#[tokio::test]
async fn without_a_season_marker_ranking_is_unchanged() {
    let state = test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Mushishi")
        .await
        .unwrap();
    state
        .storage
        .insert_minimal_anime(2, "Mushishi 2nd Season")
        .await
        .unwrap();

    let result = recognize_file("D:/Anime/Mushishi - 07.mkv", None, &state.storage)
        .await
        .unwrap();

    assert_eq!(result.parsed.as_ref().unwrap().season_number, None);
    assert_eq!(
        result.candidates.first().map(|c| c.anime_id),
        Some(1),
        "got: {:?}",
        result.candidates
    );
}

/// A weak word-overlap match that happens to be some *other* show's second
/// season must not be floated over the real match — it would sink the real one
/// below the auto-confirm threshold and stop tracking altogether.
#[tokio::test]
async fn a_weak_match_is_not_promoted_by_the_season_in_its_title() {
    let state = test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Sword Art Online")
        .await
        .unwrap();
    state
        .storage
        .insert_minimal_anime(2, "Sword Art Alternative 2nd Season")
        .await
        .unwrap();

    let result = recognize_file("D:/Anime/Sword Art Online S02E04.mkv", None, &state.storage)
        .await
        .unwrap();

    assert_eq!(
        result.candidates.first().map(|c| c.anime_id),
        Some(1),
        "got: {:?}",
        result.candidates
    );
}
