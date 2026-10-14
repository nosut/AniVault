use anivault_core::engine::storage::Storage;

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

#[tokio::test]
async fn settings_with_prefix_returns_only_matching_keys() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    storage.set_setting("ui.a", r#""1""#, 1).await.unwrap();
    storage.set_setting("ui.b", r#""2""#, 1).await.unwrap();
    storage.set_setting("uix", r#""no""#, 1).await.unwrap();
    storage.set_setting("tracking.enabled", "true", 1).await.unwrap();

    let mut rows = storage.settings_with_prefix("ui.").await.unwrap();
    rows.sort();
    assert_eq!(
        rows,
        vec![
            ("ui.a".to_string(), r#""1""#.to_string()),
            ("ui.b".to_string(), r#""2""#.to_string()),
        ]
    );
}

#[tokio::test]
async fn insert_settings_if_absent_keeps_existing_values() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    storage.set_setting("ui.kept", r#""newer""#, 1).await.unwrap();
    storage
        .insert_settings_if_absent(
            &[
                ("ui.kept".to_string(), r#""older""#.to_string()),
                ("ui.added".to_string(), r#""x""#.to_string()),
            ],
            2,
        )
        .await
        .unwrap();

    assert_eq!(storage.get_setting("ui.kept").await.unwrap(), Some(r#""newer""#.to_string()));
    assert_eq!(storage.get_setting("ui.added").await.unwrap(), Some(r#""x""#.to_string()));
}

#[tokio::test]
async fn a_damaged_library_folder_list_is_an_error_not_an_empty_list() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.set_setting("library.folders", "not json", 1).await.unwrap();

    let err = anivault_core::engine::library_scanner::get_library_folders(&storage).await.unwrap_err();
    assert!(err.to_string().contains("library folder list is damaged"));
}
