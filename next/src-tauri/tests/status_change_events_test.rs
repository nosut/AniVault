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
    assert!(anivault_core::engine::session::record_progress(&state, 1, 4, None, None, "test").await);
    assert_eq!(status_changes(&state), vec![(1, Some("dropped".into()), "watching".into())]);
}
