use taiga_next::commands::{
    delete_setting_inner, drain_engine_events_inner, get_engine_status_inner, get_setting_inner,
    set_setting_inner,
};
use taiga_next::engine::events::EngineEvent;
use taiga_next::engine::runtime::initialize_engine_at;

async fn test_state(name: &str) -> taiga_next::engine::runtime::EngineState {
    let root = std::env::temp_dir().join(format!("anivault-command-test-{}-{name}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    initialize_engine_at(root.join("anivault.db")).await.unwrap()
}

#[tokio::test]
async fn engine_status_uses_runtime_state() {
    let state = test_state("status").await;
    let status = get_engine_status_inner(&state).await.unwrap();

    assert!(status.ok);
    assert_eq!(status.database, "ready");
    assert!(status.database_path.ends_with("anivault.db"));
    assert!(status.migration_count >= 1);
}

#[tokio::test]
async fn settings_commands_roundtrip_json() {
    let state = test_state("settings").await;

    assert_eq!(get_setting_inner("tracking.enabled", &state).await.unwrap(), None);

    set_setting_inner("tracking.enabled", serde_json::json!(true), &state)
        .await
        .unwrap();
    assert_eq!(
        get_setting_inner("tracking.enabled", &state).await.unwrap(),
        Some(serde_json::json!(true))
    );

    assert!(delete_setting_inner("tracking.enabled", &state).await.unwrap());
    assert_eq!(get_setting_inner("tracking.enabled", &state).await.unwrap(), None);
}

#[tokio::test]
async fn drain_engine_events_returns_and_clears_events() {
    let state = test_state("events").await;
    state.events.publish(EngineEvent::SyncQueued {
        service: "anilist".to_string(),
        anime_id: 42,
    });

    let events = drain_engine_events_inner(&state).await.unwrap();
    assert_eq!(events.len(), 1);
    assert!(drain_engine_events_inner(&state).await.unwrap().is_empty());
}
