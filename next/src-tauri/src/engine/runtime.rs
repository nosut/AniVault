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
}

#[derive(Debug, Clone)]
pub struct TrackingControl {
    pub active: bool,
    pub watching: Option<ActivePlaybackPub>,
    pub cancel_tx: Option<watch::Sender<bool>>,
}

impl Default for TrackingControl {
    fn default() -> Self {
        Self {
            active: false,
            watching: None,
            cancel_tx: None,
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
    })
}
