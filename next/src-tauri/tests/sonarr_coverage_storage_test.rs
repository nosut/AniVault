use anivault_core::engine::storage::{CoverageLinkDb, Tests};

#[tokio::test]
async fn coverage_candidates_are_watching_and_planning_with_format() {
    let s = Tests::new_in_memory().await;
    for (id, status) in [(1, "watching"), (2, "plan_to_watch"), (3, "completed")] {
        s.upsert_anime_full(id, r#"{"romaji":"X"}"#, 12, Some("img"), None, Some("ANIME"), None, 1)
            .await
            .unwrap();
        s.upsert_list_entry_full(id, status, 0, None, "", 1, 1).await.unwrap();
    }
    s.set_anime_format(2, Some("MOVIE")).await.unwrap();
    s.set_anime_format(2, None).await.unwrap(); // None never clears

    let rows = s.coverage_candidates().await.unwrap();
    let got: Vec<_> = rows
        .iter()
        .map(|r| (r.anime_id, r.list_status.as_str(), r.format.as_deref()))
        .collect();
    assert_eq!(got, vec![(1, "watching", None), (2, "plan_to_watch", Some("MOVIE"))]);
}

#[tokio::test]
async fn coverage_links_set_ignore_and_delete() {
    let s = Tests::new_in_memory().await;
    for id in [1, 2] {
        s.upsert_anime_full(id, r#"{"romaji":"X"}"#, 12, None, None, None, None, 1)
            .await
            .unwrap();
    }
    s.coverage_link_set(1, 77, 10).await.unwrap();
    s.coverage_ignore(2, 10).await.unwrap();
    assert_eq!(
        s.coverage_links().await.unwrap(),
        vec![
            CoverageLinkDb { anime_id: 1, sonarr_id: Some(77), ignored: false },
            CoverageLinkDb { anime_id: 2, sonarr_id: None, ignored: true },
        ]
    );

    // Linking an ignored entry un-ignores it.
    s.coverage_link_set(2, 88, 11).await.unwrap();
    s.coverage_link_delete(1).await.unwrap();
    assert_eq!(
        s.coverage_links().await.unwrap(),
        vec![CoverageLinkDb { anime_id: 2, sonarr_id: Some(88), ignored: false }]
    );
    s.coverage_links_delete_all().await.unwrap();
    assert!(s.coverage_links().await.unwrap().is_empty());
}

#[tokio::test]
async fn sonarr_mapped_pairs_skips_unmapped_rows() {
    use anivault_core::engine::storage::{SonarrMappingDb, SonarrSeriesDb};
    let s = Tests::new_in_memory().await;
    s.upsert_anime_full(5, r#"{"romaji":"X"}"#, 12, None, None, None, None, 1).await.unwrap();
    for sid in [10, 11] {
        s.sonarr_series_upsert(&SonarrSeriesDb {
            sonarr_id: sid, title: "T".into(), season_count: 1, episode_count: 1, episode_file_count: 0,
            monitored: true, next_airing: None, path: None, poster_url: None, overview: None,
            network: None, status: None, added: 1, last_synced: 1,
        }).await.unwrap();
    }
    for (sid, aid) in [(10, Some(5)), (11, None)] {
        s.sonarr_mapping_upsert(&SonarrMappingDb {
            id: None, sonarr_id: sid, anime_id: aid, title_match: "T".into(), confidence: 90,
            mapped_at: 1, user_confirmed: false,
        }).await.unwrap();
    }
    assert_eq!(s.sonarr_mapped_pairs().await.unwrap(), vec![(5, 10)]);
}

#[tokio::test]
async fn coverage_candidates_for_returns_the_asked_shows_whatever_their_status() {
    let s = Tests::new_in_memory().await;
    for (id, status) in [(1, "dropped"), (2, "watching"), (3, "completed")] {
        s.upsert_anime_full(id, r#"{"romaji":"X"}"#, 12, None, None, None, None, 1)
            .await
            .unwrap();
        s.upsert_list_entry_full(id, status, 0, None, "", 1, 1).await.unwrap();
    }
    let rows = s.coverage_candidates_for(&[1, 3]).await.unwrap();
    let got: Vec<_> = rows.iter().map(|r| (r.anime_id, r.list_status.as_str())).collect();
    assert_eq!(got, vec![(1, "dropped"), (3, "completed")]);
    assert!(s.coverage_candidates_for(&[]).await.unwrap().is_empty());
}
