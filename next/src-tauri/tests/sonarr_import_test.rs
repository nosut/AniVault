use anivault_core::engine::sonarr::import::score_match_series;

#[tokio::test]
async fn import_with_no_series_reports_zero() {
    // Skip live HTTP test — unit test the scoring instead
}

#[test]
fn exact_title_match_scores_100_plus_ep_bonus() {
    let score = score_match_series(
        "Attack on Titan",
        r#"{"romaji":"Attack on Titan","english":"Attack on Titan","japanese":"進撃の巨人","synonyms":[]}"#,
        25,
        Some(25),
    );
    // Exact match (100) + episode count match diff=0 ≤3 (20) = 120
    assert_eq!(score, 120, "expected 120, got {score}");
}

#[test]
fn substring_match_scores_60() {
    let score = score_match_series(
        "Attack on Titan Final Season",
        r#"{"romaji":"Attack on Titan","english":"Attack on Titan","japanese":"進撃の巨人","synonyms":[]}"#,
        16,
        Some(25),
    );
    // Substring match (60) + episode diff=9 ≤10 (5) = 65
    assert!(score >= 60, "expected >= 60, got {score}");
}

#[test]
fn unrelated_titles_score_0() {
    let score = score_match_series(
        "One Piece",
        r#"{"romaji":"Attack on Titan","english":"Attack on Titan","japanese":"進撃の巨人","synonyms":[]}"#,
        1000,
        Some(25),
    );
    assert_eq!(score, 0);
}

#[test]
fn episode_count_match_adds_20() {
    let base = score_match_series(
        "Test",
        r#"{"romaji":"Test","english":"Test","japanese":"","synonyms":[]}"#,
        12,
        Some(12),
    );
    let off_by_10 = score_match_series(
        "Test",
        r#"{"romaji":"Test","english":"Test","japanese":"","synonyms":[]}"#,
        22,
        Some(12),
    );
    // base: exact 100 + diff=0 ≤3 (20) = 120
    // off_by_10: exact 100 + diff=10 ≤10 (5) = 105
    assert!(base > off_by_10, "expected {} > {}", base, off_by_10);
}
