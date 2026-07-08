use anivault_core::engine::events::EngineEvent;
use anivault_core::engine::library_scanner::set_library_folders;
use anivault_core::engine::library_watcher::run_auto_scan;
use anivault_core::engine::library_watcher::{affected_dirs, dirs_to_rescan, take_quiet_dirs};
use anivault_core::engine::runtime::fresh_test_state;
use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};
use notify::EventKind;
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
fn dirs_to_rescan_queues_parent_of_removed_folder() {
    // Deleting a whole show/season folder: the event path IS the directory
    // (no video extension) — must queue its parent so the prune pass runs
    // from the surviving ancestor.
    let paths = vec![PathBuf::from("C:\\Lib\\ShowA\\Season 1")];
    let dirs = dirs_to_rescan(&EventKind::Remove(RemoveKind::Folder), &paths);
    assert_eq!(dirs, vec![PathBuf::from("C:\\Lib\\ShowA")]);
}

#[test]
fn dirs_to_rescan_ignores_non_video_non_removal_events() {
    // A non-removal event (e.g. file creation reported at a non-video path,
    // such as a partial/temp file with no extension) must not queue anything.
    let paths = vec![PathBuf::from("C:\\Lib\\ShowA\\newfile")];
    let dirs = dirs_to_rescan(&EventKind::Create(CreateKind::File), &paths);
    assert!(dirs.is_empty());
}

#[test]
fn dirs_to_rescan_does_not_double_queue_video_file_parent_on_remove() {
    // A remove event whose path is a video file is already handled by
    // affected_dirs (queues the file's own parent) — must not also push that
    // same parent a second time via the removal-like branch.
    let paths = vec![PathBuf::from("C:\\Lib\\ShowA\\ep1.mkv")];
    let dirs = dirs_to_rescan(&EventKind::Remove(RemoveKind::File), &paths);
    assert_eq!(dirs, vec![PathBuf::from("C:\\Lib\\ShowA")]);
}

#[test]
fn dirs_to_rescan_queues_parent_of_renamed_folder() {
    // Renaming a whole show/season folder shows up as Modify(Name(_)) with a
    // directory path — must also queue its parent.
    let paths = vec![PathBuf::from("C:\\Lib\\ShowA\\Old Season Name")];
    let dirs = dirs_to_rescan(&EventKind::Modify(ModifyKind::Name(RenameMode::Both)), &paths);
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
