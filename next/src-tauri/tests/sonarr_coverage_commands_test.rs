use anivault_core::commands::{add_to_sonarr_inner, get_sonarr_coverage_inner, lookup_sonarr_candidates_inner};
use anivault_core::engine::runtime::{fresh_test_state, EngineState};

/// Sonarr "connected" to a port nothing listens on.
async fn state_with_dead_sonarr() -> EngineState {
    let state = fresh_test_state().await;
    let key = anivault_core::engine::secrets::protect_secret("k").unwrap();
    state.storage.set_setting("sonarr.url", "\"http://127.0.0.1:1/\"", 1).await.unwrap();
    state.storage.set_setting("sonarr.api_key", &key, 1).await.unwrap();
    state
}

#[tokio::test]
async fn unreachable_sonarr_reports_unreachable_not_everything_missing() {
    let state = state_with_dead_sonarr().await;
    state.storage.upsert_anime_full(1, r#"{"romaji":"X"}"#, 12, None, None, None, None, 1).await.unwrap();
    state.storage.upsert_list_entry_full(1, "watching", 0, None, "", 1, 1).await.unwrap();

    let resp = get_sonarr_coverage_inner(&state).await.unwrap();
    assert!(!resp.reachable);
    assert!(resp.error.is_some());
    assert!(resp.rows.is_empty());
}

#[tokio::test]
async fn coverage_without_sonarr_connected_is_an_error() {
    let state = fresh_test_state().await;
    let err = get_sonarr_coverage_inner(&state).await.unwrap_err();
    assert!(err.to_string().contains("not connected"), "{err}");
}

#[tokio::test]
async fn lookup_refuses_shows_that_are_not_watching_or_planning() {
    let state = state_with_dead_sonarr().await;
    state.storage.upsert_anime_full(1, r#"{"romaji":"X"}"#, 12, None, None, None, None, 1).await.unwrap();
    state.storage.upsert_list_entry_full(1, "completed", 0, None, "", 1, 1).await.unwrap();
    let err = lookup_sonarr_candidates_inner(&state, 1).await.unwrap_err();
    assert!(err.to_string().contains("not Watching or Planning"), "{err}");
}

#[tokio::test]
async fn add_without_sonarr_connected_is_an_error_and_links_nothing() {
    let state = fresh_test_state().await;
    assert!(add_to_sonarr_inner(&state, 1, 100).await.is_err());
    assert!(state.storage.coverage_links().await.unwrap().is_empty());
}

#[tokio::test]
async fn an_unreadable_sonarr_key_is_an_error_not_not_connected() {
    let state = fresh_test_state().await;
    state.storage.set_setting("sonarr.url", "\"http://127.0.0.1:1/\"", 1).await.unwrap();
    state.storage.set_setting("sonarr.api_key", "not-encrypted-on-this-account", 1).await.unwrap();

    let err = anivault_core::commands::list_sonarr_tags_inner(&state).await.unwrap_err();
    assert!(err.to_string().contains("saved login couldn't be read"), "{err}");

    // The drop prompt says why the Sonarr options are missing.
    let previews = anivault_core::commands::get_status_change_preview_inner(&state, vec![1]).await.unwrap();
    assert!(previews[0].sonarr_error.as_deref().unwrap_or("").contains("saved login couldn't be read"));
}

#[tokio::test]
async fn a_calendar_with_a_failed_source_raises_a_problem() {
    let state = state_with_dead_sonarr().await;
    anivault_core::commands::get_calendar_inner(&state).await.unwrap();

    let problems = state.problems.current();
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].area, "calendar");
    assert!(problems[0].detail.starts_with("Sonarr: "));
}

#[tokio::test]
async fn tvdb_lookup_names_input_that_is_not_an_id_or_link() {
    let state = state_with_dead_sonarr().await;
    let err = anivault_core::commands::lookup_sonarr_by_tvdb_inner(&state, "Frieren").await.unwrap_err();
    assert!(err.to_string().contains("isn't a TVDB ID or thetvdb.com link"), "{err}");
}

#[tokio::test]
async fn tvdb_lookup_without_sonarr_connected_is_an_error() {
    let state = fresh_test_state().await;
    let err = anivault_core::commands::lookup_sonarr_by_tvdb_inner(&state, "480598").await.unwrap_err();
    assert!(err.to_string().contains("not connected"), "{err}");
}
