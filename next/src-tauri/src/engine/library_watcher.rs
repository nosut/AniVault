//! Automatic library maintenance: a startup + hourly scan worker (this file)
//! and, added alongside it, a filesystem watcher for near-real-time pickup.

use std::time::Duration;

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
