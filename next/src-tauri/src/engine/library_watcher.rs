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
/// Errors are logged, never propagated — automatic passes must not kill loops.
pub async fn run_auto_scan(state: &EngineState) {
    let folders = match library_scanner::get_library_folders(&state.storage).await {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("auto-scan: cannot read library folders: {e}");
            return;
        }
    };
    if folders.is_empty() {
        return;
    }
    match library_scanner::scan_library_folders(&state.storage).await {
        Ok(r) if r.indexed > 0 || r.removed > 0 => {
            tracing::info!(indexed = r.indexed, removed = r.removed, "auto-scan changed the index");
            state.events.publish(EngineEvent::LibraryUpdated {
                indexed: r.indexed,
                removed: r.removed,
            });
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("auto-scan failed: {e}"),
    }
}

/// Spawn the startup + hourly automatic scan: one pass shortly after launch
/// (delayed so tracking/sync startup settles first), then every hour.
pub fn spawn_library_scan_worker(state: &EngineState) -> tauri::async_runtime::JoinHandle<()> {
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
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
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let folders = library_scanner::get_library_folders(&state.storage)
                .await
                .unwrap_or_default();

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<notify::Event>();
            let mut watcher = match notify::recommended_watcher(
                move |res: Result<notify::Event, notify::Error>| {
                    if let Ok(ev) = res {
                        let _ = tx.send(ev);
                    }
                },
            ) {
                Ok(w) => w,
                Err(e) => {
                    tracing::warn!("library watcher unavailable: {e}");
                    // Still honor folder-change signals so a later config change retries.
                    state.library_folders_changed.notified().await;
                    continue;
                }
            };

            for f in &folders {
                let path = std::path::Path::new(f);
                if !path.exists() {
                    tracing::debug!("not watching offline library folder {f}");
                    continue;
                }
                if let Err(e) = watcher.watch(path, notify::RecursiveMode::Recursive) {
                    tracing::warn!("cannot watch library folder {f}: {e}");
                }
            }

            let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
            loop {
                tokio::select! {
                    ev = rx.recv() => {
                        match ev {
                            Some(event) => {
                                for d in affected_dirs(&event.paths) {
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
                            Err(e) => tracing::warn!("watcher scan failed: {e}"),
                        }
                    }
                }
            }
            drop(watcher);
        }
    })
}
