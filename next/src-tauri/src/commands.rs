use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::engine::migration::MigrationReport;
use crate::engine::models::TrackingStatus;

#[derive(Debug, Clone, serde::Serialize)]
pub struct EngineStatus {
    pub ok: bool,
    pub database: &'static str,
}

#[tauri::command]
pub fn get_engine_status() -> EngineStatus {
    EngineStatus {
        ok: true,
        database: "uninitialized",
    }
}

#[tauri::command]
pub fn preview_migration_report() -> MigrationReport {
    MigrationReport::default()
}

#[derive(Debug, Clone, Default)]
pub struct TrackingRuntime {
    is_running: Arc<AtomicBool>,
    current_anime: Arc<Mutex<Option<String>>>,
}

impl TrackingRuntime {
    pub fn mark_running(&self) {
        self.is_running.store(true, Ordering::Relaxed);
    }

    pub fn set_current_anime(&self, current_anime: Option<String>) {
        *self.current_anime.lock().expect("tracking runtime poisoned") = current_anime;
    }

    pub fn status(&self) -> TrackingStatus {
        TrackingStatus {
            is_running: self.is_running.load(Ordering::Relaxed),
            current_anime: self.current_anime.lock().expect("tracking runtime poisoned").clone(),
        }
    }
}

#[tauri::command]
pub fn get_tracking_status(runtime: tauri::State<'_, TrackingRuntime>) -> TrackingStatus {
    runtime.status()
}

#[tauri::command]
pub fn app_exit(app: tauri::AppHandle) {
    app.exit(0);
}
