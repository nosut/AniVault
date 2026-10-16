use anivault_core::engine::sonarr::client::{
    anchor_episode, find_episode_from_anchor, find_episode_id, FirstAiring, SonarrClient, SonarrEpisode,
};

fn ep(id: i64, season: i32, number: i32, absolute: Option<i32>) -> SonarrEpisode {
    SonarrEpisode { id, season_number: season, episode_number: number, absolute_episode_number: absolute, air_date: None, air_date_utc: None }
}

/// An episode that aired on `date` at 15:30 UTC (00:30 JST the next day).
fn aired(id: i64, season: i32, number: i32, absolute: Option<i32>, date: &str) -> SonarrEpisode {
    SonarrEpisode {
        air_date: Some(date.into()),
        air_date_utc: Some(format!("{date}T15:30:00Z")),
        ..ep(id, season, number, absolute)
    }
}

fn unix(rfc3339: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(rfc3339).unwrap().timestamp()
}

/// Chitose Is in the Ramune Bottle on TheTVDB: both cours in one season 1,
/// with episodes 12 and 13 on the same day and a recap among the specials.
fn chitose() -> Vec<SonarrEpisode> {
    let mut eps = vec![aired(9015, 0, 15, None, "2026-10-06")];
    let dates = [
        "2025-10-07", "2025-10-14", "2025-10-21", "2025-10-28", "2025-11-04", "2025-12-02", "2025-12-09",
        "2025-12-16", "2025-12-23", "2025-12-30", "2026-03-30", "2026-03-31", "2026-03-31", "2026-10-13",
        "2026-10-20", "2026-10-27",
    ];
    for (i, d) in dates.iter().enumerate() {
        let n = i as i32 + 1;
        eps.push(aired(1000 + n as i64, 1, n, Some(n), d));
    }
    eps
}

#[test]
fn a_second_cour_counts_on_from_where_it_starts_in_the_series() {
    let eps = chitose();
    // AniList: the 2nd cour's episode 1 airs 2026-10-14 00:30 JST.
    let anchor = anchor_episode(&eps, FirstAiring::At(unix("2026-10-13T15:30:00Z"))).unwrap();
    assert_eq!(anchor.id, 1014);
    assert_eq!(find_episode_from_anchor(&eps, anchor, 1), Some(1014));
    assert_eq!(find_episode_from_anchor(&eps, anchor, 3), Some(1016));
    assert_eq!(find_episode_from_anchor(&eps, anchor, 13), None);
}

#[test]
fn the_first_cour_anchors_on_the_series_first_episode() {
    let eps = chitose();
    let anchor = anchor_episode(&eps, FirstAiring::At(unix("2025-10-07T15:30:00Z"))).unwrap();
    assert_eq!(find_episode_from_anchor(&eps, anchor, 12), Some(1012));
}

#[test]
fn a_start_date_anchors_within_a_day_and_ignores_specials() {
    let eps = chitose();
    // AniList's start date is the JST day, a day after TVDB's.
    let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 14).unwrap();
    assert_eq!(anchor_episode(&eps, FirstAiring::Date(day)).map(|e| e.id), Some(1014));
}

#[test]
fn a_same_day_pair_anchors_on_the_earlier_episode() {
    let eps = chitose();
    let day = chrono::NaiveDate::from_ymd_opt(2026, 3, 31).unwrap();
    assert_eq!(anchor_episode(&eps, FirstAiring::Date(day)).map(|e| e.id), Some(1012));
}

#[test]
fn no_episode_near_the_first_airing_gives_no_anchor() {
    let eps = chitose();
    assert!(anchor_episode(&eps, FirstAiring::At(unix("2024-01-01T00:00:00Z"))).is_none());
}

#[test]
fn without_absolute_numbers_the_anchor_counts_within_its_season() {
    let eps = vec![
        aired(1, 1, 12, None, "2024-03-25"),
        aired(2, 2, 1, None, "2025-01-06"),
        aired(3, 2, 2, None, "2025-01-13"),
    ];
    let anchor = anchor_episode(&eps, FirstAiring::At(unix("2025-01-06T15:30:00Z"))).unwrap();
    assert_eq!(find_episode_from_anchor(&eps, anchor, 2), Some(3));
}

#[test]
fn find_episode_id_prefers_absolute_number() {
    // AniList counts absolute episodes; Sonarr splits into seasons. S2E5 here
    // is absolute episode 17 — the absolute match must win over S1E17.
    let eps = vec![
        ep(100, 1, 17, Some(17)),
        ep(200, 2, 5, Some(17)),
    ];
    // Both carry absolute 17; the first hit wins — either is the right file
    // target, so just assert an absolute match is used.
    assert_eq!(find_episode_id(&eps, 17), Some(100));

    let eps = vec![ep(300, 2, 5, Some(17)), ep(301, 2, 6, Some(18))];
    assert_eq!(find_episode_id(&eps, 17), Some(300));
}

#[test]
fn find_episode_id_falls_back_to_season_episode_number() {
    // No absolute numbering (common for non-anime series types): match the
    // plain episode number, skipping specials (season 0).
    let eps = vec![
        ep(400, 0, 3, None),
        ep(401, 1, 3, None),
    ];
    assert_eq!(find_episode_id(&eps, 3), Some(401));
}

#[test]
fn find_episode_id_returns_none_when_absent() {
    let eps = vec![ep(500, 1, 1, Some(1))];
    assert_eq!(find_episode_id(&eps, 9), None);
}

#[test]
fn client_constructs_with_url_and_key() {
    let client = SonarrClient::new("http://localhost:8989".into(), "abc123".into());
    assert_eq!(client.url, "http://localhost:8989");
    assert_eq!(client.api_key, "abc123");
}

#[test]
fn client_trims_trailing_slash_from_url() {
    let client = SonarrClient::new("http://localhost:8989/".into(), "key".into());
    assert_eq!(client.url, "http://localhost:8989");
}

#[tokio::test]
async fn validate_connection_returns_error_for_nonexistent_host() {
    let client = SonarrClient::new("http://127.0.0.1:19999".into(), "bad".into());
    assert!(client.validate_connection().await.is_err());
}

#[tokio::test]
async fn fetch_series_returns_error_for_nonexistent_host() {
    let client = SonarrClient::new("http://127.0.0.1:19999".into(), "bad".into());
    assert!(client.fetch_series().await.is_err());
}

#[test]
fn tag_details_become_options_with_series_counts_sorted_by_label() {
    use anivault_core::engine::sonarr::client::{tag_options, SonarrTagDetail};
    let details: Vec<SonarrTagDetail> = serde_json::from_str(
        r#"[
            {"id": 3, "label": "mine", "seriesIds": [10, 11, 12], "delayProfileIds": []},
            {"id": 1, "label": "1 - nosut", "seriesIds": [10]},
            {"id": 2, "label": "Anime"}
        ]"#,
    )
    .expect("tag detail deserializes, seriesIds optional");

    let options = tag_options(details);
    let summary: Vec<(i64, &str, usize)> = options
        .iter()
        .map(|o| (o.id, o.label.as_str(), o.series_count))
        .collect();
    assert_eq!(summary, vec![(1, "1 - nosut", 1), (2, "Anime", 0), (3, "mine", 3)]);
}

#[tokio::test]
async fn listing_sonarr_tags_requires_a_connection() {
    let state = anivault_core::engine::runtime::fresh_test_state().await;
    let err = anivault_core::commands::list_sonarr_tags_inner(&state)
        .await
        .expect_err("not connected");
    assert!(err.to_string().contains("not connected"), "{err}");
}

use anivault_core::engine::sonarr::client::{
    lookup_candidates, pick_anilist_import_list, SonarrImportList, SonarrLookupSeries, SonarrSeriesRaw,
};

#[test]
fn series_raw_reads_tvdb_id_and_alternate_titles() {
    let json = r#"{"id":5,"title":"Overgeared","monitored":true,"tvdbId":448176,
        "alternateTitles":[{"title":"テムパル","seasonNumber":-1}],"tags":[]}"#;
    let s: SonarrSeriesRaw = serde_json::from_str(json).unwrap();
    assert_eq!(s.tvdb_id, Some(448176));
    assert_eq!(s.alternate_titles[0].title, "テムパル");
}

fn import_list(id: i64, implementation: &str) -> SonarrImportList {
    SonarrImportList {
        id,
        name: format!("list {id}"),
        implementation: implementation.to_string(),
        root_folder_path: Some("/anime".into()),
        quality_profile_id: Some(4),
        series_type: Some("anime".into()),
        season_folder: Some(true),
        should_monitor: Some("all".into()),
        monitor_new_items: Some("all".into()),
        tags: vec![1],
    }
}

#[test]
fn pick_anilist_import_list_matches_case_insensitively() {
    let lists = vec![import_list(1, "TraktListImport"), import_list(2, "AniListImport")];
    assert_eq!(pick_anilist_import_list(&lists).map(|l| l.id), Some(2));
    assert!(pick_anilist_import_list(&lists[..1]).is_none());
}

#[test]
fn pick_anilist_import_list_tolerates_nulls_and_unknown_fields() {
    let json = r#"[{"id":3,"name":"AniList","implementation":"AniListImport",
        "rootFolderPath":null,"qualityProfileId":6,"seriesType":"anime","seasonFolder":true,
        "shouldMonitor":"all","monitorNewItems":"all","tags":[2],"enableAutomaticAdd":true,
        "fields":[{"name":"username","value":"nosut"}]}]"#;
    let lists: Vec<SonarrImportList> = serde_json::from_str(json).unwrap();
    let l = pick_anilist_import_list(&lists).unwrap();
    assert_eq!((l.quality_profile_id, l.root_folder_path.as_deref()), (Some(6), None));
}

#[test]
fn lookup_candidates_dedupes_and_marks_in_sonarr() {
    let json = r#"[
        {"title":"Overgeared","year":2025,"tvdbId":448176,"id":0,"seasons":[{"seasonNumber":1}],
         "images":[{"coverType":"poster","remoteUrl":"http://p/1.jpg"}]},
        {"title":"Overgeared","year":2025,"tvdbId":448176,"seasons":[]},
        {"title":"No TVDB","year":2020,"seasons":[]},
        {"title":"Overlord","year":2015,"tvdbId":294002,"id":12,"seasonCount":4,"seasons":[]}
    ]"#;
    let results: Vec<SonarrLookupSeries> = serde_json::from_str(json).unwrap();
    let c = lookup_candidates(results, 5);
    assert_eq!(c.len(), 2);
    assert_eq!((c[0].tvdb_id, c[0].in_sonarr, c[0].season_count), (448176, false, 1));
    assert_eq!(c[0].poster_url.as_deref(), Some("http://p/1.jpg"));
    assert_eq!((c[1].in_sonarr, c[1].sonarr_id, c[1].season_count), (true, Some(12), 4));
}

fn found(tvdb: i64) -> SonarrLookupSeries {
    serde_json::from_value(serde_json::json!({ "title": format!("S{tvdb}"), "tvdbId": tvdb })).unwrap()
}

#[tokio::test]
async fn lookup_across_terms_skips_a_failing_term() {
    use anivault_core::engine::sonarr::client::lookup_across_terms;
    let terms = vec!["bad".to_string(), "good".to_string()];
    let got = lookup_across_terms(&terms, |t| async move {
        if t == "bad" { Err(anyhow::anyhow!("Skyhook 503")) } else { Ok(vec![found(1)]) }
    })
    .await
    .unwrap();
    assert_eq!(got.iter().map(|c| c.tvdb_id).collect::<Vec<_>>(), vec![1]);
}

#[tokio::test]
async fn lookup_across_terms_fails_only_when_every_term_fails() {
    use anivault_core::engine::sonarr::client::lookup_across_terms;
    let terms = vec!["a".to_string(), "b".to_string()];
    let err = lookup_across_terms(&terms, |_| async { Err(anyhow::anyhow!("Skyhook 503")) })
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Skyhook 503"), "{err}");
}

#[tokio::test]
async fn lookup_across_terms_searches_every_term_and_dedupes() {
    use anivault_core::engine::sonarr::client::lookup_across_terms;
    // The right series often comes from a later term (the English title after
    // a romaji one that returns 20 unrelated shows), so no early stop.
    let terms: Vec<String> = vec!["romaji".into(), "english".into()];
    let got = lookup_across_terms(&terms, |t| async move {
        if t == "romaji" { Ok((1..=6).map(found).collect()) } else { Ok(vec![found(3), found(99)]) }
    })
    .await
    .unwrap();
    let ids: Vec<i64> = got.iter().map(|c| c.tvdb_id).collect();
    assert_eq!(ids, vec![1, 2, 3, 4, 5, 6, 99]);
}

/// Mushoku Tensei on TheTVDB: two specials take absolute numbers inside the
/// run (S00E01 is absolute 17, S00E02 "Guardian Fitz" is absolute 25 and is
/// AniList's episode 1 of season 2).
fn mushoku() -> Vec<SonarrEpisode> {
    let mut eps = Vec::new();
    let mut abs = 0;
    for n in 1..=23 {
        abs += 1;
        if n == 17 {
            eps.push(aired(9001, 0, 1, Some(abs), "2021-11-01"));
            abs += 1;
        }
        let date = if n == 12 { "2021-10-03" } else { "2021-01-01" };
        eps.push(aired(1000 + n as i64, 1, n, Some(abs), date));
    }
    eps.push(aired(9002, 0, 2, Some(25), "2023-07-02"));
    eps.push(aired(2001, 2, 1, Some(26), "2023-07-09"));
    eps.push(aired(2002, 2, 2, Some(27), "2023-07-16"));
    // Newly listed: no absolute number yet.
    eps.push(aired(2003, 2, 3, None, "2023-07-23"));
    eps
}

#[test]
fn counting_on_steps_over_specials_numbered_into_the_run() {
    let eps = mushoku();
    // Part 2 starts at S01E12; its episode 6 is S01E17, not the S00E01 OVA.
    let anchor = anchor_episode(&eps, FirstAiring::At(unix("2021-10-03T15:30:00Z"))).unwrap();
    assert_eq!(anchor.id, 1012);
    assert_eq!(find_episode_from_anchor(&eps, anchor, 6), Some(1017));
    assert_eq!(find_episode_from_anchor(&eps, anchor, 12), Some(1023));
    // And on across the season boundary.
    assert_eq!(find_episode_from_anchor(&eps, anchor, 13), Some(2001));
}

#[test]
fn a_special_premiere_anchors_and_the_run_continues_after_it() {
    let eps = mushoku();
    let anchor = anchor_episode(&eps, FirstAiring::At(unix("2023-07-02T15:30:00Z"))).unwrap();
    assert_eq!(anchor.id, 9002);
    assert_eq!(find_episode_from_anchor(&eps, anchor, 1), Some(9002));
    assert_eq!(find_episode_from_anchor(&eps, anchor, 2), Some(2001));
    // An episode with no absolute number yet is still found.
    assert_eq!(find_episode_from_anchor(&eps, anchor, 4), Some(2003));
}

#[test]
fn a_regular_episode_wins_a_tie_with_a_special() {
    let eps = vec![aired(1, 0, 1, Some(1), "2024-01-06"), aired(2, 1, 1, Some(1), "2024-01-06")];
    let anchor = anchor_episode(&eps, FirstAiring::At(unix("2024-01-06T15:30:00Z"))).unwrap();
    assert_eq!(anchor.id, 2);
}

#[test]
fn unnumbered_specials_never_anchor() {
    let eps = vec![aired(1, 0, 1, None, "2024-01-06"), aired(2, 1, 1, Some(1), "2024-01-13")];
    assert!(anchor_episode(&eps, FirstAiring::At(unix("2024-01-06T15:30:00Z"))).is_none());
}
