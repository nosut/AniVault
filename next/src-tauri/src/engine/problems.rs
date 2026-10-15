//! Problems in background work the user should know about: sync stalled, a
//! library folder offline, the folder watcher not running. Kept as current
//! state rather than events, because the event buffer is drained once and
//! capped, and a problem raised before the window opened must still show.
//!
//! Workers `report` when something fails and `resolve` when it works again.
//! The frontend reads the board on its regular poll and shows each problem
//! in plain words, with the raw error behind "Show details".

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// Stable keys for each kind of background work. The frontend maps these to
/// a "Try again" action where one makes sense.
pub mod area {
    pub const SYNC: &str = "sync";
    pub const SYNC_QUEUE: &str = "sync_queue";
    pub const LIBRARY_SCAN: &str = "library_scan";
    pub const LIBRARY_FOLDERS: &str = "library_folders";
    pub const LIBRARY_WATCHER: &str = "library_watcher";
    pub const PROGRESS: &str = "progress";
    pub const TRACKING: &str = "tracking";
    pub const TRACKING_SETTING: &str = "tracking_setting";
    pub const SONARR_SETTINGS: &str = "sonarr_settings";
    pub const CALENDAR: &str = "calendar";
    pub const STARTUP_ENTRY: &str = "startup_entry";
    pub const WORKER: &str = "worker";
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Problem {
    pub area: String,
    /// What didn't happen, in plain words, e.g. "Changes aren't reaching AniList."
    pub summary: String,
    /// The raw error, for "Show details".
    pub detail: String,
    /// When this problem was first seen, so a repeat of the same failure
    /// doesn't reopen a notice the user dismissed.
    pub since: i64,
}

#[derive(Clone, Default)]
pub struct ProblemBoard {
    inner: Arc<Mutex<BTreeMap<String, Problem>>>,
}

impl ProblemBoard {
    /// Record or update the problem for `area`. A repeat with the same summary
    /// keeps its original `since`; a different summary starts a new problem.
    pub fn report(&self, area: &str, summary: impl Into<String>, detail: impl std::fmt::Display) {
        let summary = summary.into();
        let detail = detail.to_string();
        tracing::warn!("{area}: {summary} ({detail})");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or_default();
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let since = match map.get(area) {
            Some(existing) if existing.summary == summary => existing.since,
            _ => now,
        };
        map.insert(area.to_string(), Problem { area: area.to_string(), summary, detail, since });
    }

    /// The work in `area` succeeded: drop its problem, if any.
    pub fn resolve(&self, area: &str) {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if map.remove(area).is_some() {
            tracing::info!("{area}: recovered");
        }
    }

    pub fn current(&self) -> Vec<Problem> {
        let map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        map.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_keeps_since_for_a_repeat_and_resolve_clears() {
        let board = ProblemBoard::default();
        board.report(area::SYNC, "Changes aren't reaching AniList.", "boom");
        let first = board.current();
        assert_eq!(first.len(), 1);

        board.report(area::SYNC, "Changes aren't reaching AniList.", "boom again");
        let second = board.current();
        assert_eq!(second[0].since, first[0].since);
        assert_eq!(second[0].detail, "boom again");

        board.resolve(area::SYNC);
        assert!(board.current().is_empty());
    }

    #[test]
    fn areas_are_independent() {
        let board = ProblemBoard::default();
        board.report(area::SYNC, "a", "x");
        board.report(area::LIBRARY_SCAN, "b", "y");
        board.resolve(area::SYNC);
        let left = board.current();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].area, area::LIBRARY_SCAN);
    }
}
