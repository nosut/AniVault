//! Automatic library maintenance: a startup + hourly scan worker (this file)
//! and, added alongside it, a filesystem watcher for near-real-time pickup.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use notify::Watcher;

use crate::engine::events::EngineEvent;
use crate::engine::library_scanner;
use crate::engine::runtime::EngineState;

/// Run one automatic library scan pass. Publishes `LibraryUpdated` only when
/// the index actually changed; silent no-op when no folders are configured.
/// Errors never propagate, so the loop keeps running, but they go on the
/// problem board: an offline folder or a failed scan is otherwise invisible.
pub async fn run_auto_scan(state: &EngineState) {
    use crate::engine::problems::area;
    let folders = match library_scanner::get_library_folders(&state.storage).await {
        Ok(f) => {
            state.problems.resolve(area::LIBRARY_FOLDERS);
            f
        }
        Err(e) => {
            state.problems.report(
                area::LIBRARY_FOLDERS,
                "Your library folders couldn't be read, so automatic scans are paused.",
                e,
            );
            return;
        }
    };
    if folders.is_empty() {
        state.problems.resolve(area::LIBRARY_SCAN);
        return;
    }
    let result = library_scanner::scan_library_folders(&state.storage).await;
    note_scan_result(state, &result, true);
    if let Ok(r) = result {
        if r.indexed > 0 || r.removed > 0 {
            tracing::info!(indexed = r.indexed, removed = r.removed, "auto-scan changed the index");
            state.events.publish(EngineEvent::LibraryUpdated {
                indexed: r.indexed,
                removed: r.removed,
            });
        }
    }
}

/// Put a full library scan's outcome on the problem board. The automatic scan
/// reports and clears; the Scan button (`report: false`) only clears, since
/// its own failures are already shown where it was pressed.
pub fn note_scan_result(
    state: &EngineState,
    result: &anyhow::Result<library_scanner::LibraryScanReport>,
    report: bool,
) {
    use crate::engine::problems::area;
    // A scan that ran at all read the folder list, so that problem is over.
    if result.is_ok() {
        state.problems.resolve(area::LIBRARY_FOLDERS);
    }
    if !report {
        if matches!(result, Ok(r) if r.errors.is_empty()) {
            state.problems.resolve(area::LIBRARY_SCAN);
        }
        return;
    }
    match result {
        // The folder list itself is unreadable: that is the folder-list
        // problem, not a second "scan didn't finish" notice for one cause.
        Err(e) if e.to_string().contains("library folder list is damaged") => state.problems.report(
            area::LIBRARY_FOLDERS,
            "Your library folders couldn't be read, so automatic scans are paused.",
            e,
        ),
        Ok(r) if r.errors.is_empty() => state.problems.resolve(area::LIBRARY_SCAN),
        Ok(r) => state.problems.report(
            area::LIBRARY_SCAN,
            "Some library folders couldn't be scanned.",
            r.errors.join("\n"),
        ),
        Err(e) => state.problems.report(area::LIBRARY_SCAN, "The library scan didn't finish.", e),
    }
}

/// Spawn the startup + hourly automatic scan: one pass shortly after launch
/// (delayed so tracking/sync startup settles first), then every hour.
pub fn spawn_library_scan_worker(state: &EngineState) -> tauri::async_runtime::JoinHandle<()> {
    crate::engine::runtime::spawn_supervised(state, "The automatic library scan", move |state| async move {
        tokio::time::sleep(Duration::from_secs(20)).await;
        loop {
            run_auto_scan(&state).await;
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    })
}

/// How long a directory must stay quiet after its last filesystem event before
/// we scan it — rides out multi-file moves and in-progress downloads.
const DEBOUNCE_QUIET: Duration = Duration::from_secs(5);

/// Directories worth rescanning for a batch of event paths: the parent of
/// every touched video file, deduplicated, in first-seen order.
pub fn affected_dirs(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for p in paths {
        if !library_scanner::is_video_file(p) {
            continue;
        }
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() && !dirs.contains(&parent.to_path_buf()) {
                dirs.push(parent.to_path_buf());
            }
        }
    }
    dirs
}

/// Directories to rescan for one filesystem event. Video-file paths always
/// queue their parent. For remove/rename events, a non-video path is usually
/// a directory (deleted/renamed folder): queue ITS parent, so the prune pass
/// runs from the surviving ancestor and clears the vanished rows.
pub fn dirs_to_rescan(kind: &notify::EventKind, paths: &[PathBuf]) -> Vec<PathBuf> {
    use notify::event::ModifyKind;
    use notify::EventKind;

    let mut dirs = affected_dirs(paths);
    let removal_like = matches!(kind, EventKind::Remove(_))
        || matches!(kind, EventKind::Modify(ModifyKind::Name(_)));
    if removal_like {
        for p in paths {
            if library_scanner::is_video_file(p) {
                continue; // already handled by affected_dirs
            }
            if let Some(parent) = p.parent() {
                if !parent.as_os_str().is_empty() && !dirs.contains(&parent.to_path_buf()) {
                    dirs.push(parent.to_path_buf());
                }
            }
        }
    }
    dirs
}

/// Remove and return the pending directories whose last event is at least
/// `quiet` ago — they're ready to scan. Recently-busy directories stay pending.
pub fn take_quiet_dirs(
    pending: &mut HashMap<PathBuf, Instant>,
    now: Instant,
    quiet: Duration,
) -> Vec<PathBuf> {
    let mut ready: Vec<PathBuf> = pending
        .iter()
        .filter(|(_, t)| now.saturating_duration_since(**t) >= quiet)
        .map(|(d, _)| d.clone())
        .collect();
    ready.sort();
    for d in &ready {
        pending.remove(d);
    }
    ready
}

/// Watch the configured library folders and run targeted scans when video
/// files change. Rebuilds its watch list when `library_folders_changed` fires.
/// Folders that are offline or fail to watch are logged and left to the hourly
/// scan as fallback.
pub fn spawn_library_watcher(state: &EngineState) -> tauri::async_runtime::JoinHandle<()> {
    crate::engine::runtime::spawn_supervised(state, "The library folder watcher", move |state| async move {
        use crate::engine::problems::area;
        const NOT_WATCHING: &str =
            "New files in your library won't be picked up straight away; the hourly scan will still find them.";
        loop {
            let folders = match library_scanner::get_library_folders(&state.storage).await {
                Ok(f) => f,
                Err(e) => {
                    // Reported as a library folders problem by the scan
                    // worker; wait for the folders to be set again.
                    tracing::warn!("library watcher: cannot read library folders: {e}");
                    state.library_folders_changed.notified().await;
                    continue;
                }
            };

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<notify::Event>();
            let board = state.problems.clone();
            let mut watcher = match notify::recommended_watcher(
                move |res: Result<notify::Event, notify::Error>| match res {
                    Ok(ev) => {
                        let _ = tx.send(ev);
                    }
                    Err(e) => board.report(area::LIBRARY_WATCHER, NOT_WATCHING, e),
                },
            ) {
                Ok(w) => w,
                Err(e) => {
                    state.problems.report(area::LIBRARY_WATCHER, NOT_WATCHING, e);
                    // Still honor folder-change signals so a later config change retries.
                    state.library_folders_changed.notified().await;
                    continue;
                }
            };

            // Offline folders are reported by the scan; only a folder that is
            // there but can't be watched is a watcher problem.
            let mut failed: Vec<String> = Vec::new();
            for f in &folders {
                let path = std::path::Path::new(f);
                if !path.exists() {
                    tracing::debug!("not watching offline library folder {f}");
                    continue;
                }
                if let Err(e) = watcher.watch(path, notify::RecursiveMode::Recursive) {
                    failed.push(format!("{f}: {e}"));
                }
            }
            if failed.is_empty() {
                state.problems.resolve(area::LIBRARY_WATCHER);
            } else {
                state.problems.report(area::LIBRARY_WATCHER, NOT_WATCHING, failed.join("\n"));
            }

            let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
            loop {
                tokio::select! {
                    ev = rx.recv() => {
                        match ev {
                            Some(event) => {
                                for d in dirs_to_rescan(&event.kind, &event.paths) {
                                    pending.insert(d, Instant::now());
                                }
                            }
                            None => break, // watcher gone; rebuild
                        }
                    }
                    _ = state.library_folders_changed.notified() => break, // rebuild with new folders
                    _ = tokio::time::sleep(Duration::from_secs(1)), if !pending.is_empty() => {
                        let ready = take_quiet_dirs(&mut pending, Instant::now(), DEBOUNCE_QUIET);
                        if ready.is_empty() {
                            continue;
                        }
                        let dirs: Vec<String> =
                            ready.iter().map(|d| d.to_string_lossy().to_string()).collect();
                        match library_scanner::scan_specific_dirs(&state.storage, &dirs).await {
                            Ok(r) if r.indexed > 0 || r.removed > 0 => {
                                tracing::info!(
                                    indexed = r.indexed,
                                    removed = r.removed,
                                    "watcher scan changed the index"
                                );
                                state.events.publish(EngineEvent::LibraryUpdated {
                                    indexed: r.indexed,
                                    removed: r.removed,
                                });
                            }
                            Ok(_) => {}
                            // The hourly scan retries these folders and reports
                            // if they stay unreadable.
                            Err(e) => tracing::warn!("watcher scan failed: {e}"),
                        }
                    }
                }
            }
            drop(watcher);
        }
    })
}
