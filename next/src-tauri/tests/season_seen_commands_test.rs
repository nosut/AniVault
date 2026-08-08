use anivault_core::commands::diff_season_inner;
use anivault_core::engine::runtime::{fresh_test_state, EngineState};

async fn state() -> EngineState {
    fresh_test_state().await
}

fn sorted(mut ids: Vec<i64>) -> Vec<i64> {
    ids.sort_unstable();
    ids
}

#[tokio::test]
async fn the_first_visit_flags_nothing_and_records_the_baseline() {
    // Every id is trivially "not yet recorded" on a first visit. Reporting them
    // would light up the entire grid, which is the opposite of the point.
    let state = state().await;
    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2, 3], true)
        .await
        .unwrap();

    assert!(diff.first_visit);
    assert!(diff.new_ids.is_empty(), "nothing is new on a first visit");
    assert_eq!(
        sorted(state.storage.season_seen_ids("FALL", 2026).await.unwrap()),
        vec![1, 2, 3],
        "the baseline is still recorded"
    );
}

#[tokio::test]
async fn a_later_visit_reports_only_the_additions() {
    let state = state().await;
    diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2, 3], true)
        .await
        .unwrap();

    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2, 3, 4, 5], true)
        .await
        .unwrap();

    assert!(!diff.first_visit);
    assert_eq!(sorted(diff.new_ids), vec![4, 5]);
    assert_eq!(
        sorted(state.storage.season_seen_ids("FALL", 2026).await.unwrap()),
        vec![1, 2, 3, 4, 5]
    );
}

#[tokio::test]
async fn seeing_the_same_listing_again_reports_nothing() {
    let state = state().await;
    diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2], true).await.unwrap();
    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2], true)
        .await
        .unwrap();
    assert!(!diff.first_visit);
    assert!(diff.new_ids.is_empty());
}

#[tokio::test]
async fn a_filtered_view_reports_without_recording() {
    // A genre-filtered listing must never become the baseline: it contains only
    // that genre, so recording it would mark the rest of the season new.
    let state = state().await;
    diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2], true).await.unwrap();

    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![2, 9], false)
        .await
        .unwrap();

    assert_eq!(diff.new_ids, vec![9]);
    assert_eq!(
        sorted(state.storage.season_seen_ids("FALL", 2026).await.unwrap()),
        vec![1, 2],
        "the filtered view wrote nothing"
    );
}

#[tokio::test]
async fn a_delisted_show_does_not_resurrect_as_new() {
    let state = state().await;
    diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2], true).await.unwrap();
    // AniList drops 2 …
    diff_season_inner(&state, "FALL".into(), 2026, vec![1], true).await.unwrap();
    // … then lists it again.
    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2], true)
        .await
        .unwrap();
    assert!(diff.new_ids.is_empty(), "it was already known; removals are not tracked");
}

#[tokio::test]
async fn an_empty_listing_leaves_the_season_unbaselined() {
    let state = state().await;
    let diff = diff_season_inner(&state, "FALL".into(), 2030, vec![], true)
        .await
        .unwrap();
    assert!(diff.first_visit);
    assert!(diff.new_ids.is_empty());

    // Nothing was recorded, so the next real listing is still a first visit.
    let diff = diff_season_inner(&state, "FALL".into(), 2030, vec![4], true)
        .await
        .unwrap();
    assert!(diff.first_visit);
    assert!(diff.new_ids.is_empty());
}

#[tokio::test]
async fn a_filtered_first_visit_leaves_the_season_unbaselined() {
    // The two rules interact: a first visit flags nothing, and a filtered view
    // records nothing, so the season still has no baseline afterwards. The next
    // unfiltered visit is therefore still the first visit. `first_visit` tracks
    // "has a baseline", not "has ever been opened".
    let state = state().await;
    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![1], false)
        .await
        .unwrap();
    assert!(diff.first_visit);
    assert!(state.storage.season_seen_ids("FALL", 2026).await.unwrap().is_empty());

    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2, 3], true)
        .await
        .unwrap();
    assert!(diff.first_visit, "the unfiltered visit baselines the full listing");
    assert!(diff.new_ids.is_empty());
    assert_eq!(
        sorted(state.storage.season_seen_ids("FALL", 2026).await.unwrap()),
        vec![1, 2, 3]
    );
}

#[tokio::test]
async fn the_future_page_keys_independently_of_real_seasons() {
    let state = state().await;
    diff_season_inner(&state, "__FUTURE__".into(), 0, vec![1], true).await.unwrap();
    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![1], true)
        .await
        .unwrap();
    assert!(diff.first_visit, "a real season is untouched by the future page");
}
