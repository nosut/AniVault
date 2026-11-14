//! Re-imports Sonarr on a timer, so monitoring changes, new series and new
//! downloads show up without pressing Import Series.

use std::time::Duration;

use crate::engine::events::EngineEvent;
use crate::engine::problems::area;
use crate::engine::runtime::EngineState;

/// Setting holding the minutes between automatic imports; 0 turns them off.
pub const AUTO_SYNC_MINUTES_KEY: &str = "sonarr.auto_sync_minutes";
pub const DEFAULT_AUTO_SYNC_MINUTES: i64 = 30;

/// How often the worker wakes to see whether an import is due. Short, so a
/// changed interval or a fresh connection takes effect within a minute.
const TICK: Duration = Duration::from_secs(60);

/// Whether an automatic import is due. `last_run` is the later of the last
/// finished import (manual or automatic) and the last automatic attempt, so a
/// failing Sonarr is retried once per interval, not every tick.
pub fn sync_due(interval_minutes: i64, last_run: Option<i64>, now: i64) -> bool {
    if interval_minutes <= 0 {
        return false;
    }
    match last_run {
        None => true,
        Some(last) => now - last >= interval_minutes * 60,
    }
}

async fn read_i64(state: &EngineState, key: &str) -> Option<i64> {
    let raw = state.storage.get_setting(key).await.ok().flatten()?;
    serde_json::from_str::<i64>(&raw).ok()
}

/// Run one import if Sonarr is connected and one is due. Returns the time of
/// the attempt when it made one.
async fn run_if_due(state: &EngineState, last_attempt: Option<i64>) -> Option<i64> {
    let connected = matches!(state.storage.get_setting("sonarr.url").await, Ok(Some(_)));
    if !connected {
        state.problems.resolve(area::SONARR_SYNC);
        return None;
    }
    let interval = read_i64(state, AUTO_SYNC_MINUTES_KEY)
        .await
        .unwrap_or(DEFAULT_AUTO_SYNC_MINUTES);
    if interval <= 0 {
        state.problems.resolve(area::SONARR_SYNC);
        return None;
    }
    let last_sync = read_i64(state, "sonarr.last_sync_at").await;
    let last_run = match (last_sync, last_attempt) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    };
    let now = crate::commands::unix_now_inner().ok()?;
    if !sync_due(interval, last_run, now) {
        return None;
    }
    match crate::commands::import_sonarr_series_inner(state).await {
        Ok(report) => {
            state.problems.resolve(area::SONARR_SYNC);
            state.events.publish(EngineEvent::SonarrSynced {
                imported: report.imported,
                auto_mapped: report.auto_mapped,
                unmapped: report.unmapped,
            });
        }
        Err(e) => state.problems.report(
            area::SONARR_SYNC,
            "Sonarr couldn't be checked, so its series may be out of date in AniVault.",
            e,
        ),
    }
    Some(now)
}

pub fn spawn_sonarr_sync_worker(state: &EngineState) -> tauri::async_runtime::JoinHandle<()> {
    crate::engine::runtime::spawn_supervised(state, "The automatic Sonarr check", move |state| async move {
        // Let startup, tracking and the first library scan settle first.
        tokio::time::sleep(Duration::from_secs(45)).await;
        let mut last_attempt = None;
        loop {
            if let Some(at) = run_if_due(&state, last_attempt).await {
                last_attempt = Some(at);
            }
            tokio::time::sleep(TICK).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::sync_due;

    #[test]
    fn off_never_runs() {
        assert!(!sync_due(0, None, 10_000));
        assert!(!sync_due(-5, Some(0), 10_000));
    }

    #[test]
    fn never_synced_runs_now() {
        assert!(sync_due(30, None, 10_000));
    }

    #[test]
    fn waits_out_the_interval() {
        assert!(!sync_due(30, Some(10_000), 10_000 + 29 * 60));
        assert!(sync_due(30, Some(10_000), 10_000 + 30 * 60));
    }
}
