use anivault_core::engine::sonarr::client::{find_episode_id, SonarrClient, SonarrEpisode};

fn ep(id: i64, season: i32, number: i32, absolute: Option<i32>) -> SonarrEpisode {
    SonarrEpisode { id, season_number: season, episode_number: number, absolute_episode_number: absolute }
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
    let got = lookup_across_terms(&terms, 5, |t| async move {
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
    let err = lookup_across_terms(&terms, 5, |_| async { Err(anyhow::anyhow!("Skyhook 503")) })
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Skyhook 503"), "{err}");
}

#[tokio::test]
async fn lookup_across_terms_stops_once_it_has_enough() {
    use anivault_core::engine::sonarr::client::lookup_across_terms;
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let terms: Vec<String> = (0..8).map(|i| format!("t{i}")).collect();
    let c = calls.clone();
    let got = lookup_across_terms(&terms, 2, move |_| {
        let n = c.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as i64;
        async move { Ok(vec![found(n * 10), found(n * 10 + 1)]) }
    })
    .await
    .unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}
