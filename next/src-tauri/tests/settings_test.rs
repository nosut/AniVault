use taiga_next::engine::storage::Storage;

#[tokio::test]
async fn settings_roundtrip_json_values() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    assert_eq!(storage.get_setting("tracking.enabled").await.unwrap(), None);

    storage
        .set_setting("tracking.enabled", "true", 1_782_769_008)
        .await
        .unwrap();
    assert_eq!(
        storage.get_setting("tracking.enabled").await.unwrap(),
        Some("true".to_string())
    );

    storage
        .set_setting("tracking.enabled", "false", 1_782_769_009)
        .await
        .unwrap();
    assert_eq!(
        storage.get_setting("tracking.enabled").await.unwrap(),
        Some("false".to_string())
    );
}

#[tokio::test]
async fn settings_delete_reports_whether_row_existed() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    assert!(!storage.delete_setting("theme").await.unwrap());

    storage
        .set_setting("theme", r#""dark""#, 1_782_769_008)
        .await
        .unwrap();

    assert!(storage.delete_setting("theme").await.unwrap());
    assert_eq!(storage.get_setting("theme").await.unwrap(), None);
}

#[tokio::test]
async fn migrated_storage_reports_migration_count() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    assert!(storage.migration_count().await.unwrap() >= 1);
}
