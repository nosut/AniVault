use anivault_core::engine::sonarr::{SonarrConfig, get_sonarr_config, set_sonarr_config};
use anivault_core::engine::storage::Storage;

#[tokio::test]
async fn sonarr_config_defaults_empty() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let config = get_sonarr_config(&storage).await.unwrap();
    assert!(config.url.is_empty());
    assert!(config.api_key.is_empty());
}

#[tokio::test]
async fn sonarr_config_stores_and_retrieves() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let config = SonarrConfig {
        url: "http://localhost:8989".into(),
        api_key: "abc123secret".into(),
    };
    set_sonarr_config(&storage, &config).await.unwrap();

    let retrieved = get_sonarr_config(&storage).await.unwrap();
    assert_eq!(retrieved.url, "http://localhost:8989");
    assert_eq!(retrieved.api_key, "abc123secret");
}

#[tokio::test]
async fn sonarr_config_clears_on_empty_url() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    // Save then clear
    set_sonarr_config(&storage, &SonarrConfig {
        url: "http://localhost:8989".into(),
        api_key: "oldkey".into(),
    }).await.unwrap();

    set_sonarr_config(&storage, &SonarrConfig {
        url: "".into(),
        api_key: "".into(),
    }).await.unwrap();

    let config = get_sonarr_config(&storage).await.unwrap();
    assert!(config.url.is_empty());
    assert!(config.api_key.is_empty());
}

#[test]
fn parses_sonarr_search_response() {
    let json = r#"[
        {"title": "Spy Family", "id": 42, "year": 2022, "tvdbId": 123, "titleSlug": "spy-family", "images": [{"coverType": "poster", "remoteUrl": "https://example.com/poster.jpg"}]}
    ]"#;
    let results = anivault_core::engine::sonarr::parse_sonarr_search(json).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 42);
    assert_eq!(results[0].title, "Spy Family");
    assert_eq!(results[0].year, 2022);
}

#[tokio::test]
async fn map_anime_to_sonarr_series() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Spy x Family").await.unwrap();

    anivault_core::engine::sonarr::map_sonarr_series(&storage, 1, 42, "Spy Family").await.unwrap();

    let mappings = anivault_core::engine::sonarr::get_sonarr_mappings(&storage).await.unwrap();
    assert_eq!(mappings.len(), 1);
    assert_eq!(mappings[0].anime_id, 1);
    assert_eq!(mappings[0].sonarr_series_id, 42);
}

#[tokio::test]
async fn unmap_removes_sonarr_link() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();

    anivault_core::engine::sonarr::map_sonarr_series(&storage, 1, 99, "Cowboy Bebop").await.unwrap();
    anivault_core::engine::sonarr::unmap_sonarr_series(&storage, 1).await.unwrap();

    let mappings = anivault_core::engine::sonarr::get_sonarr_mappings(&storage).await.unwrap();
    assert!(mappings.is_empty());
}
