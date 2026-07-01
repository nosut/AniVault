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
