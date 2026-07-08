use anivault_core::engine::events::EngineEvent;
use anivault_core::engine::library_scanner::set_library_folders;
use anivault_core::engine::library_watcher::run_auto_scan;
use anivault_core::engine::runtime::fresh_test_state;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("anivault_watch_{tag}_{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn auto_scan_publishes_event_only_on_change() {
    let state = fresh_test_state().await;
    let dir = unique_temp_dir("autoscan");
    fs::write(dir.join("Show - 01.mkv"), b"x").unwrap();
    set_library_folders(&state.storage, vec![dir.to_string_lossy().to_string()])
        .await
        .unwrap();

    // First pass indexes the file → event.
    run_auto_scan(&state).await;
    let events = state.events.drain();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, EngineEvent::LibraryUpdated { indexed: 1, removed: 0 })),
        "first scan must publish LibraryUpdated, got {events:?}"
    );

    // Second pass: nothing changed on disk → no event (and no index churn).
    run_auto_scan(&state).await;
    assert!(
        state.events.drain().is_empty(),
        "unchanged rescan must stay silent"
    );

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn auto_scan_without_folders_is_a_no_op() {
    let state = fresh_test_state().await;
    run_auto_scan(&state).await;
    assert!(state.events.drain().is_empty());
}
