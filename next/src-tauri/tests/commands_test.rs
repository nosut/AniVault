use anivault_core::commands::{
    delete_setting_inner, drain_engine_events_inner, get_calendar_inner, get_engine_status_inner,
    get_setting_inner, set_setting_inner,
};
use anivault_core::engine::events::EngineEvent;
use anivault_core::engine::runtime::initialize_engine_at;

async fn test_state(name: &str) -> anivault_core::engine::runtime::EngineState {
    let root = std::env::temp_dir().join(format!("anivault-command-test-{}-{name}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    initialize_engine_at(root.join("anivault.db"), None).await.unwrap()
}

fn calendar_cache_json(fetched_at: i64, airing_at: i64) -> String {
    serde_json::json!({
        "fetched_at": fetched_at,
        "entries": [{
            "anime_id": 5, "title": "Cached Show", "image_url": null, "episode_count": 12,
            "progress": null, "next_episode": 3, "airing_at": airing_at,
            "time_until_airing": 0, "has_file": false
        }]
    })
    .to_string()
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[tokio::test]
async fn calendar_serves_fresh_cache_and_marks_downloads_live() {
    let state = test_state("calfresh").await;
    let now = unix_now();

    // A fresh cache (1 min old) with one aired episode; the file for it exists
    // locally, so has_file must be recomputed fresh even on the cache path.
    state
        .storage
        .set_setting("calendar.cache", &calendar_cache_json(now - 60, now - 3600), now)
        .await
        .unwrap();
    state.storage.insert_minimal_anime(5, "Cached Show").await.unwrap();
    state
        .storage
        .upsert_file_index(
            "Y:/Anime/Cached Show/ep3.mkv",
            Some(5),
            3,
            100,
            anivault_core::engine::storage::MappingSource::Manual,
            now,
        )
        .await
        .unwrap();

    let entries = get_calendar_inner(&state).await.unwrap();

    assert_eq!(entries.len(), 1, "the cached calendar should be served");
    assert_eq!(entries[0].title, "Cached Show");
    assert!(entries[0].has_file, "download status is computed fresh, not cached");
}

#[tokio::test]
async fn calendar_falls_back_to_stale_cache_when_remote_is_empty() {
    let state = test_state("calstale").await;
    let now = unix_now();

    // Cache is a day old (expired); with no AniList token and no Sonarr the
    // remote fetch yields nothing — stale data beats an empty calendar.
    state
        .storage
        .set_setting("calendar.cache", &calendar_cache_json(now - 86_400, now + 3600), now)
        .await
        .unwrap();

    let entries = get_calendar_inner(&state).await.unwrap();

    assert_eq!(entries.len(), 1, "stale cache should be served when remote sources return nothing");
    assert_eq!(entries[0].title, "Cached Show");
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
