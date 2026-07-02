use taiga_next::commands::{
    get_sonarr_status_inner, disconnect_sonarr_inner, remap_sonarr_inner,
};
use taiga_next::engine::runtime::fresh_test_state;

#[tokio::test]
async fn sonarr_status_not_connected_when_no_key() {
    let state = fresh_test_state().await;
    let status = get_sonarr_status_inner(&state).await.unwrap();
    assert!(!status.connected);
    assert_eq!(status.series_count, 0);
    assert_eq!(status.mapped_count, 0);
}

#[tokio::test]
async fn disconnect_sonarr_cleans_up() {
    let state = fresh_test_state().await;

    // Set fake connection settings
    state.storage.set_setting("sonarr.url", r#""http://localhost:8989""#, 1)
        .await
        .unwrap();

    let _ = disconnect_sonarr_inner(&state).await;
    let status = get_sonarr_status_inner(&state).await.unwrap();
    assert!(!status.connected);
}

#[tokio::test]
async fn remap_sonarr_updates_mapping() {
    let state = fresh_test_state().await;

    // Insert a series + unmapped mapping
    let series = taiga_next::engine::storage::SonarrSeriesDb {
        sonarr_id: 42,
        title: "Test Series".into(),
        season_count: 1,
        episode_count: 12,
        episode_file_count: 0,
        monitored: true,
        next_airing: None,
        path: None,
        poster_url: None,
        overview: None,
        network: None,
        status: None,
        added: 1,
        last_synced: 1,
    };
    state.storage.sonarr_series_upsert(&series).await.unwrap();

    let mapping = taiga_next::engine::storage::SonarrMappingDb {
        id: None,
        sonarr_id: 42,
        anime_id: None,
        title_match: "Test".into(),
        confidence: 20,
        mapped_at: 1,
        user_confirmed: false,
    };
    state.storage.sonarr_mapping_upsert(&mapping).await.unwrap();

    // Insert a test anime
    state.storage.insert_minimal_anime(99, "Test Anime").await.unwrap();

    // Remap
    remap_sonarr_inner(42, Some(99), &state).await.unwrap();

    let updated = state.storage.sonarr_mapping_by_anime(99).await.unwrap();
    assert!(updated.is_some());
    assert_eq!(updated.unwrap().sonarr_id, 42);
}
