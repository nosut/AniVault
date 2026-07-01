use anivault_core::commands::TrackingRuntime;
use anivault_core::engine::models::{DetectionConfig, TrackingStatus};
use anivault_core::engine::settings::detection_config_from_settings;

#[test]
fn detection_settings_empty_json_uses_defaults() {
    let config = detection_config_from_settings("").unwrap();

    assert_eq!(config.folders, DetectionConfig::default().folders);
    assert_eq!(config.poll_interval_ms, 2_000);
}

#[test]
fn detection_settings_decodes_config_json() {
    let config = detection_config_from_settings(
        r#"{"folders":["E:\\Anime","F:\\Shows"],"poll_interval_ms":750}"#,
    )
    .unwrap();

    assert_eq!(config.folders, vec!["E:\\Anime", "F:\\Shows"]);
    assert_eq!(config.poll_interval_ms, 750);
}

#[test]
fn tracking_runtime_reports_running_status() {
    let runtime = TrackingRuntime::default();
    assert_eq!(runtime.status(), TrackingStatus { is_running: false, current_anime: None });

    runtime.mark_running();
    runtime.set_current_anime(Some("Spy x Family".to_string()));

    assert_eq!(
        runtime.status(),
        TrackingStatus { is_running: true, current_anime: Some("Spy x Family".to_string()) }
    );
}
