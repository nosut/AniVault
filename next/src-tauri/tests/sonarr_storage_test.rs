use anivault_core::engine::storage::{SonarrSeriesDb, SonarrMappingDb, Tests};

#[tokio::test]
async fn sonarr_series_upsert_and_list() {
    let storage = Tests::new_in_memory().await;

    let series = SonarrSeriesDb {
        sonarr_id: 1,
        title: "Attack on Titan".into(),
        season_count: 1,
        episode_count: 25,
        episode_file_count: 25,
        monitored: true,
        next_airing: None,
        path: Some("D:\\Media\\Anime\\Attack on Titan".into()),
        poster_url: Some("https://example.com/poster.jpg".into()),
        overview: Some("Humanity fights titans.".into()),
        network: Some("NHK".into()),
        status: Some("ended".into()),
        added: 1_700_000_000,
        last_synced: 1_700_000_000,
    };

    storage.sonarr_series_upsert(&series).await.unwrap();
    assert_eq!(storage.sonarr_series_count().await.unwrap(), 1);

    let list = storage.sonarr_series_list().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].sonarr_id, 1);
    assert_eq!(list[0].title, "Attack on Titan");
}

#[tokio::test]
async fn sonarr_series_delete_all_clears_table() {
    let storage = Tests::new_in_memory().await;

    let series = SonarrSeriesDb {
        sonarr_id: 1,
        title: "Test".into(),
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
        added: 1_700_000_000,
        last_synced: 1_700_000_000,
    };
    storage.sonarr_series_upsert(&series).await.unwrap();
    assert_eq!(storage.sonarr_series_count().await.unwrap(), 1);

    storage.sonarr_series_delete_all().await.unwrap();
    assert_eq!(storage.sonarr_series_count().await.unwrap(), 0);
}

#[tokio::test]
async fn sonarr_mapping_crud_and_availability() {
    let storage = Tests::new_in_memory().await;

    // Insert a test anime and series so FKs work
    storage.insert_minimal_anime(42, "Test Anime").await.unwrap();
    let series = SonarrSeriesDb {
        sonarr_id: 1,
        title: "Test Anime".into(),
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
        added: 1_700_000_000,
        last_synced: 1_700_000_000,
    };
    storage.sonarr_series_upsert(&series).await.unwrap();

    let mapping = SonarrMappingDb {
        id: None,
        sonarr_id: 1,
        anime_id: Some(42),
        title_match: "Test Anime".into(),
        confidence: 90,
        mapped_at: 1_700_000_000,
        user_confirmed: false,
    };
    storage.sonarr_mapping_upsert(&mapping).await.unwrap();
    assert_eq!(storage.sonarr_mapping_count().await.unwrap(), 1);

    let found = storage.sonarr_mapping_by_anime(42).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().sonarr_id, 1);

    // Verify availability join returns enriched series data
    let avail = storage.sonarr_availability(42).await.unwrap();
    assert!(avail.is_some(), "availability should return Some for mapped anime");
    let avail = avail.unwrap();
    assert_eq!(avail.sonarr_title, "Test Anime");
    assert!(avail.monitored);
    assert_eq!(avail.episode_count, 12);
    assert_eq!(avail.episode_file_count, 0);
    assert!(avail.path.is_none());

    storage.sonarr_mapping_delete_all().await.unwrap();
    assert_eq!(storage.sonarr_mapping_count().await.unwrap(), 0);
}

#[tokio::test]
async fn sonarr_mapping_unmapped_returns_nulls() {
    let storage = Tests::new_in_memory().await;

    // Insert a series so FK works
    let series = SonarrSeriesDb {
        sonarr_id: 1,
        title: "Unknown".into(),
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
        added: 1_700_000_000,
        last_synced: 1_700_000_000,
    };
    storage.sonarr_series_upsert(&series).await.unwrap();

    let mapping = SonarrMappingDb {
        id: None,
        sonarr_id: 1,
        anime_id: None,
        title_match: "Unknown".into(),
        confidence: 30,
        mapped_at: 1_700_000_000,
        user_confirmed: false,
    };
    storage.sonarr_mapping_upsert(&mapping).await.unwrap();

    let unmapped = storage.sonarr_mapping_unmapped().await.unwrap();
    assert_eq!(unmapped.len(), 1);
    assert_eq!(unmapped[0].sonarr_id, 1);
    assert!(unmapped[0].anime_id.is_none());
}
