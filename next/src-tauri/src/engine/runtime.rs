use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::AppHandle;
use tokio::sync::watch;

use crate::engine::event_bus::EventBus;
use crate::engine::storage::Storage;

pub async fn fresh_test_state() -> EngineState {
    let storage = crate::engine::storage::Tests::new_in_memory().await;
    EngineState {
        storage,
        events: EventBus::default(),
        database_path: PathBuf::from(":memory:"),
        tracking: Arc::new(std::sync::Mutex::new(TrackingControl::default())),
        tracking_paused: Arc::new(AtomicBool::new(false)),
        app_handle: None,
        library_folders_changed: Arc::new(tokio::sync::Notify::new()),
        problems: Default::default(),
    }
}

#[derive(Clone)]
pub struct EngineState {
    pub storage: Storage,
    pub events: EventBus,
    pub database_path: PathBuf,
    pub tracking: Arc<std::sync::Mutex<TrackingControl>>,
    pub tracking_paused: Arc<AtomicBool>,
    pub app_handle: Option<AppHandle>,
    /// Notified whenever the configured library folders change, so the
    /// filesystem watcher can rebuild its watch list.
    pub library_folders_changed: Arc<tokio::sync::Notify>,
    /// Background failures the user should see; see `problems`.
    pub problems: crate::engine::problems::ProblemBoard,
}

impl EngineState {
    /// The list status of a show, if it has a list entry.
    pub async fn list_status(&self, anime_id: i64) -> Option<String> {
        self.storage.get_list_entry(anime_id).await.ok().flatten().map(|e| e.status)
    }

    /// Publish `StatusChanged` if the show's status is no longer `from`.
    /// Call with the status read before the change.
    pub async fn announce_status_change(&self, anime_id: i64, from: Option<String>) {
        let Some(to) = self.list_status(anime_id).await else { return };
        if from.as_deref() == Some(to.as_str()) {
            return;
        }
        let title = self
            .storage
            .coverage_candidates_for(&[anime_id])
            .await
            .ok()
            .and_then(|rows| rows.into_iter().next())
            .map(|r| crate::engine::sonarr::coverage::display_title(&r.titles_json))
            .unwrap_or_else(|| format!("Anime #{anime_id}"));
        self.events.publish(crate::engine::events::EngineEvent::StatusChanged {
            anime_id,
            title,
            from,
            to,
        });
    }
}

#[derive(Debug, Clone)]
pub struct TrackingControl {
    pub active: bool,
    pub watching: Option<ActivePlaybackPub>,
    pub cancel_tx: Option<watch::Sender<bool>>,
    /// Bumped by every start. A loop only writes tracking state while this
    /// still matches the generation it was started with, so a stopped loop
    /// winding down can't clobber the loop that replaced it.
    pub generation: u64,
}

impl Default for TrackingControl {
    fn default() -> Self {
        Self {
            active: false,
            watching: None,
            cancel_tx: None,
            generation: 0,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActivePlaybackPub {
    pub player_name: String,
    pub file_path: Option<String>,
    pub window_title: Option<String>,
    pub episode_guess: Option<i32>,
}

pub fn sqlite_url_for_path(path: &Path) -> String {
    format!("sqlite:///{}", path.to_string_lossy().replace('\\', "/"))
}

pub async fn initialize_engine_at(
    database_path: PathBuf,
    app_handle: Option<AppHandle>,
) -> anyhow::Result<EngineState> {
    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let database_url = sqlite_url_for_path(&database_path);
    let storage = Storage::connect(&database_url).await?;
    storage.migrate().await?;

    Ok(EngineState {
        storage,
        events: EventBus::default(),
        database_path,
        tracking: Arc::new(std::sync::Mutex::new(TrackingControl::default())),
        tracking_paused: Arc::new(AtomicBool::new(false)),
        app_handle,
        library_folders_changed: Arc::new(tokio::sync::Notify::new()),
        problems: Default::default(),
    })
}

/// Run a background worker, restarting it if it panics. Without this, one
/// panic would stop syncing or scanning for the rest of the session with
/// nothing on screen. Each restart is reported on the problem board.
pub fn spawn_supervised<F, Fut>(
    state: &EngineState,
    name: &'static str,
    make: F,
) -> tauri::async_runtime::JoinHandle<()>
where
    F: Fn(EngineState) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    supervise_with_delay(
        state,
        name,
        make,
        std::time::Duration::from_secs(30),
        std::time::Duration::from_secs(600),
    )
}

/// `delay` before a restart; once a restarted run has lasted `stable`, its
/// notice is cleared.
fn supervise_with_delay<F, Fut>(
    state: &EngineState,
    name: &'static str,
    make: F,
    delay: std::time::Duration,
    stable: std::time::Duration,
) -> tauri::async_runtime::JoinHandle<()>
where
    F: Fn(EngineState) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let state = state.clone();
    // One area per worker, so a crash in one doesn't overwrite another's.
    let area = format!("{}:{name}", crate::engine::problems::area::WORKER);
    tauri::async_runtime::spawn(async move {
        loop {
            let mut run = tauri::async_runtime::spawn(make(state.clone()));
            let finished = tokio::select! {
                result = &mut run => Some(result),
                _ = tokio::time::sleep(stable) => None,
            };
            let result = match finished {
                Some(result) => result,
                None => {
                    // Survived long enough: the restart worked.
                    state.problems.resolve(&area);
                    run.await
                }
            };
            match result {
                Ok(()) => return,
                Err(e) => {
                    state.problems.report(
                        &area,
                        format!("{name} stopped unexpectedly and was restarted."),
                        e,
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }
    })
}

#[cfg(test)]
mod supervisor_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn a_panicking_worker_is_restarted_and_reported() {
        let state = fresh_test_state().await;
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = runs.clone();
        let handle = supervise_with_delay(
            &state,
            "Test worker",
            move |_state| {
                let counter = counter.clone();
                async move {
                    if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                        panic!("first run fails");
                    }
                }
            },
            std::time::Duration::ZERO,
            std::time::Duration::from_secs(600),
        );
        handle.await.unwrap();

        assert_eq!(runs.load(Ordering::SeqCst), 2);
        let problems = state.problems.current();
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].summary, "Test worker stopped unexpectedly and was restarted.");
    }

    #[tokio::test]
    async fn a_restarted_worker_that_keeps_running_clears_its_notice() {
        let state = fresh_test_state().await;
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = runs.clone();
        let _handle = supervise_with_delay(
            &state,
            "Test worker",
            move |_state| {
                let counter = counter.clone();
                async move {
                    if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                        panic!("first run fails");
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                }
            },
            std::time::Duration::ZERO,
            std::time::Duration::from_millis(50),
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert_eq!(state.problems.current()[0].area, "worker:Test worker");
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(state.problems.current().is_empty());
    }
}
