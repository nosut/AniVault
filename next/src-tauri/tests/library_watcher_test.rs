use anivault_core::engine::events::EngineEvent;
use anivault_core::engine::library_scanner::set_library_folders;
use anivault_core::engine::library_watcher::run_auto_scan;
use anivault_core::engine::library_watcher::{affected_dirs, take_quiet_dirs};
use anivault_core::engine::runtime::fresh_test_state;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

#[test]
fn affected_dirs_filters_to_video_parents() {
    let paths = vec![
        PathBuf::from("C:\\Lib\\ShowA\\ep1.mkv"),
        PathBuf::from("C:\\Lib\\ShowA\\ep1.mkv"), // duplicate event
        PathBuf::from("C:\\Lib\\ShowA\\ep2.MP4"), // extension is case-insensitive
        PathBuf::from("C:\\Lib\\ShowB\\notes.txt"), // not a video
        PathBuf::from("C:\\Lib\\ShowC\\ep1.mkv.part"), // in-progress download
    ];
    let dirs = affected_dirs(&paths);
    assert_eq!(dirs, vec![PathBuf::from("C:\\Lib\\ShowA")]);
}

#[test]
fn take_quiet_dirs_respects_debounce() {
    let base = Instant::now();
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
    pending.insert(PathBuf::from("C:\\Lib\\Quiet"), base);
    pending.insert(PathBuf::from("C:\\Lib\\Busy"), base + Duration::from_secs(4));

    // 5 seconds after `base`: Quiet has been silent 5s (ready), Busy only 1s.
    let ready = take_quiet_dirs(&mut pending, base + Duration::from_secs(5), Duration::from_secs(5));
    assert_eq!(ready, vec![PathBuf::from("C:\\Lib\\Quiet")]);
    assert!(pending.contains_key(&PathBuf::from("C:\\Lib\\Busy")));
    assert!(!pending.contains_key(&PathBuf::from("C:\\Lib\\Quiet")));
}
