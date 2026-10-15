use anivault_core::engine::events::EngineEvent;
use anivault_core::engine::runtime::{fresh_test_state, EngineState};

async fn show(state: &EngineState, id: i64, status: &str, watched: i32) {
    state
        .storage
        .upsert_anime_full(id, r#"{"romaji":"Show"}"#, 12, None, None, None, None, 1)
        .await
        .unwrap();
    state
        .storage
        .upsert_list_entry_full(id, status, watched, None, "", 1, 1)
        .await
        .unwrap();
}

fn status_changes(state: &EngineState) -> Vec<(i64, Option<String>, String)> {
    state
        .events
        .drain()
        .into_iter()
        .filter_map(|e| match e {
            EngineEvent::StatusChanged { anime_id, from, to, .. } => Some((anime_id, from, to)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn explicit_status_change_is_announced_with_the_previous_status() {
    let state = fresh_test_state().await;
    show(&state, 1, "watching", 3).await;
    anivault_core::commands::update_list_entry_inner(1, Some("dropped".into()), None, None, &state)
        .await
        .unwrap();
    assert_eq!(status_changes(&state), vec![(1, Some("watching".into()), "dropped".into())]);
}

#[tokio::test]
async fn unchanged_status_and_progress_only_edits_are_not_announced() {
    let state = fresh_test_state().await;
    show(&state, 1, "dropped", 3).await;
    anivault_core::commands::update_list_entry_inner(1, Some("dropped".into()), None, None, &state)
        .await
        .unwrap();
    anivault_core::commands::update_list_entry_inner(1, None, Some(5), None, &state)
        .await
        .unwrap();
    assert!(status_changes(&state).is_empty());
}

#[tokio::test]
async fn auto_complete_from_progress_is_announced() {
    let state = fresh_test_state().await;
    show(&state, 1, "watching", 11).await;
    anivault_core::commands::update_list_entry_inner(1, None, Some(12), None, &state)
        .await
        .unwrap();
    assert_eq!(status_changes(&state), vec![(1, Some("watching".into()), "completed".into())]);
}

#[tokio::test]
async fn playback_resuming_a_dropped_show_is_announced() {
    let state = fresh_test_state().await;
    show(&state, 1, "dropped", 3).await;
    assert!(anivault_core::engine::session::record_progress(&state, 1, 4, None, None, "test").await.unwrap());
    assert_eq!(status_changes(&state), vec![(1, Some("dropped".into()), "watching".into())]);
}

#[tokio::test]
async fn a_failed_progress_write_is_an_error_with_no_toast_or_event() {
    let state = fresh_test_state().await;
    show(&state, 1, "completed", 12).await;
    anivault_core::engine::storage::Tests::exec(&state.storage, "ALTER TABLE list_entry RENAME TO list_entry_gone").await;

    let result = anivault_core::engine::session::record_progress(&state, 1, 13, None, None, "test").await;

    assert!(result.is_err());
    assert!(state.events.drain().iter().all(|e| !matches!(e, EngineEvent::ProgressAdvanced { .. })));
    let problems = state.problems.current();
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].summary, "Your watch progress couldn't be saved.");
}

#[tokio::test]
async fn a_failed_history_write_still_saves_progress_and_says_so() {
    let state = fresh_test_state().await;
    show(&state, 1, "watching", 3).await;
    anivault_core::engine::storage::Tests::exec(&state.storage, "DROP TABLE watch_history").await;

    assert!(anivault_core::engine::session::record_progress(&state, 1, 4, None, None, "test").await.unwrap());

    assert_eq!(state.storage.get_list_entry(1).await.unwrap().unwrap().watched_episodes, 4);
    let problems = state.problems.current();
    assert_eq!(problems.len(), 1);
    assert!(problems[0].summary.contains("couldn't be added to your watch history"));
}

#[tokio::test]
async fn a_failed_confirm_is_left_to_the_screen_that_asked() {
    let state = fresh_test_state().await;
    show(&state, 1, "watching", 1).await;
    anivault_core::engine::storage::Tests::exec(&state.storage, "ALTER TABLE list_entry RENAME TO list_entry_gone").await;

    let result = anivault_core::engine::session::record_progress(&state, 1, 2, None, None, "confirmed").await;

    assert!(result.is_err());
    assert!(state.problems.current().is_empty());
}
