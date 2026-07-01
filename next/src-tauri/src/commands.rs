use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::events::EngineEvent;
use crate::engine::migration::MigrationReport;
use crate::engine::runtime::EngineState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EngineStatus {
    pub ok: bool,
    pub database: String,
    pub database_path: String,
    pub migration_count: i64,
}

fn command_error(error: anyhow::Error) -> String {
    error.to_string()
}

fn unix_now() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?;
    Ok(duration.as_secs() as i64)
}

pub async fn get_engine_status_inner(state: &EngineState) -> Result<EngineStatus, String> {
    let migration_count = state
        .storage
        .migration_count()
        .await
        .map_err(command_error)?;

    Ok(EngineStatus {
        ok: true,
        database: "ready".to_string(),
        database_path: state.database_path.to_string_lossy().to_string(),
        migration_count,
    })
}

pub async fn get_setting_inner(
    key: &str,
    state: &EngineState,
) -> Result<Option<serde_json::Value>, String> {
    let Some(value_json) = state.storage.get_setting(key).await.map_err(command_error)? else {
        return Ok(None);
    };
    let value = serde_json::from_str(&value_json).map_err(|error| error.to_string())?;
    Ok(Some(value))
}

pub async fn set_setting_inner(
    key: &str,
    value: serde_json::Value,
    state: &EngineState,
) -> Result<(), String> {
    let value_json = serde_json::to_string(&value).map_err(|error| error.to_string())?;
    state
        .storage
        .set_setting(key, &value_json, unix_now()?)
        .await
        .map_err(command_error)
}

pub async fn delete_setting_inner(key: &str, state: &EngineState) -> Result<bool, String> {
    state.storage.delete_setting(key).await.map_err(command_error)
}

pub async fn drain_engine_events_inner(state: &EngineState) -> Result<Vec<EngineEvent>, String> {
    Ok(state.events.drain())
}

#[tauri::command]
pub async fn get_engine_status(
    state: tauri::State<'_, EngineState>,
) -> Result<EngineStatus, String> {
    get_engine_status_inner(&state).await
}

#[tauri::command]
pub async fn preview_migration_report() -> Result<MigrationReport, String> {
    Ok(MigrationReport::default())
}

#[tauri::command]
pub async fn get_setting(
    key: String,
    state: tauri::State<'_, EngineState>,
) -> Result<Option<serde_json::Value>, String> {
    get_setting_inner(&key, &state).await
}

#[tauri::command]
pub async fn set_setting(
    key: String,
    value: serde_json::Value,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    set_setting_inner(&key, value, &state).await
}

#[tauri::command]
pub async fn delete_setting(
    key: String,
    state: tauri::State<'_, EngineState>,
) -> Result<bool, String> {
    delete_setting_inner(&key, &state).await
}

#[tauri::command]
pub async fn drain_engine_events(
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<EngineEvent>, String> {
    drain_engine_events_inner(&state).await
}
