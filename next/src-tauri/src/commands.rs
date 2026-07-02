use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::watch;

use crate::engine::anilist::auth;
use crate::engine::anilist::client::AniListClient;
use crate::engine::anilist::import::{import_library, ImportReport};
use crate::engine::events::EngineEvent;
use crate::engine::matcher::{confirm_identification as matcher_confirm, recognize_file, RecognitionResult};
use crate::engine::migration::MigrationReport;
use crate::engine::runtime::EngineState;
use crate::engine::storage::{FileIndexRow, WatchHistoryRow};
use crate::engine::tracker::run_tracking_loop;

#[derive(Debug, serde::Serialize)]
pub struct SyncStatus {
    pub pending: i64,
    pub failed: i64,
    pub blocked: i64,
    pub last_sync_at: Option<i64>,
}

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

pub fn unix_now_inner() -> anyhow::Result<i64> {
    unix_now().map_err(|e| anyhow::anyhow!(e))
}

pub async fn store_anilist_token_inner(token: &str, state: &EngineState) -> anyhow::Result<()> {
    auth::store_token(&state.storage, token).await?;
    Ok(())
}

pub async fn disconnect_anilist_inner(state: &EngineState) -> anyhow::Result<()> {
    auth::delete_token(&state.storage).await?;
    state.storage.delete_tracker_mappings("anilist").await?;
    Ok(())
}

pub async fn import_anilist_library_inner(state: &EngineState) -> anyhow::Result<ImportReport> {
    let token = auth::load_token(&state.storage)
        .await?
        .ok_or_else(|| anyhow::anyhow!("not connected"))?;
    let client = AniListClient::new(token);
    import_library(&client, &state.storage).await
}

pub async fn get_sync_status_inner(state: &EngineState) -> anyhow::Result<SyncStatus> {
    let (pending, failed, blocked) = state.storage.sync_status_counts("anilist").await?;
    Ok(SyncStatus {
        pending,
        failed,
        blocked,
        last_sync_at: None,
    })
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackingStatus {
    pub active: bool,
    pub watching: Option<crate::engine::runtime::ActivePlaybackPub>,
}

pub async fn start_tracking_inner(state: &EngineState) -> Result<TrackingStatus, String> {
    let mut ctrl = state.tracking.lock().map_err(|e| e.to_string())?;
    if ctrl.active {
        return Ok(TrackingStatus {
            active: true,
            watching: ctrl.watching.clone(),
        });
    }

    let (tx, rx) = watch::channel(false);
    ctrl.cancel_tx = Some(tx);
    ctrl.active = true;

    let state_clone = state.clone();
    tokio::spawn(async move {
        run_tracking_loop(state_clone, 2000, rx).await;
    });

    Ok(TrackingStatus {
        active: true,
        watching: ctrl.watching.clone(),
    })
}

pub async fn stop_tracking_inner(state: &EngineState) -> Result<TrackingStatus, String> {
    let mut ctrl = state.tracking.lock().map_err(|e| e.to_string())?;
    if let Some(tx) = ctrl.cancel_tx.take() {
        let _ = tx.send(true);
    }
    ctrl.active = false;
    ctrl.watching = None;

    Ok(TrackingStatus {
        active: false,
        watching: None,
    })
}

pub async fn get_tracking_status_inner(state: &EngineState) -> Result<TrackingStatus, String> {
    let ctrl = state.tracking.lock().map_err(|e| e.to_string())?;
    Ok(TrackingStatus {
        active: ctrl.active,
        watching: ctrl.watching.clone(),
    })
}

pub async fn mark_episode_watched_inner(
    anime_id: i64,
    episode: i32,
    state: &EngineState,
) -> Result<(), String> {
    state
        .storage
        .append_watch_history(anime_id, episode, None, Some("manual"), unix_now()?)
        .await
        .map_err(command_error)?;

    state
        .storage
        .upsert_list_entry_progress(anime_id, "Watching", episode, unix_now()?)
        .await
        .map_err(command_error)?;

    state.events.publish(EngineEvent::ProgressAdvanced {
        anime_id,
        old_episode: episode.saturating_sub(1),
        new_episode: episode,
        source: "manual".to_string(),
    });

    Ok(())
}

pub async fn list_recent_history_inner(
    limit: i64,
    state: &EngineState,
) -> Result<Vec<WatchHistoryRow>, String> {
    state
        .storage
        .list_recent_watch_history(limit)
        .await
        .map_err(command_error)
}

// Recognition commands

pub async fn identify_file_inner(
    file_path: &str,
    window_title: Option<&str>,
    state: &EngineState,
) -> Result<RecognitionResult, String> {
    recognize_file(file_path, window_title, &state.storage)
        .await
        .map_err(command_error)
}

pub async fn confirm_identification_inner(
    file_path: &str,
    anime_id: i64,
    episode: i32,
    state: &EngineState,
) -> Result<(), String> {
    matcher_confirm(state, file_path, anime_id, episode)
        .await
        .map_err(command_error)
}

pub async fn list_known_files_inner(
    limit: i64,
    state: &EngineState,
) -> Result<Vec<FileIndexRow>, String> {
    state
        .storage
        .list_file_index(limit, 0)
        .await
        .map_err(command_error)
}

// Tauri command wrappers

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

#[tauri::command]
pub async fn start_tracking(
    state: tauri::State<'_, EngineState>,
) -> Result<TrackingStatus, String> {
    start_tracking_inner(&state).await
}

#[tauri::command]
pub async fn stop_tracking(
    state: tauri::State<'_, EngineState>,
) -> Result<TrackingStatus, String> {
    stop_tracking_inner(&state).await
}

#[tauri::command]
pub async fn get_tracking_status(
    state: tauri::State<'_, EngineState>,
) -> Result<TrackingStatus, String> {
    get_tracking_status_inner(&state).await
}

#[tauri::command]
pub async fn mark_episode_watched(
    anime_id: i64,
    episode: i32,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    mark_episode_watched_inner(anime_id, episode, &state).await
}

#[tauri::command]
pub async fn list_recent_history(
    limit: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<WatchHistoryRow>, String> {
    list_recent_history_inner(limit, &state).await
}

#[tauri::command]
pub async fn identify_file(
    file_path: String,
    window_title: Option<String>,
    state: tauri::State<'_, EngineState>,
) -> Result<RecognitionResult, String> {
    identify_file_inner(&file_path, window_title.as_deref(), &state).await
}

#[tauri::command]
pub async fn confirm_identification(
    file_path: String,
    anime_id: i64,
    episode: i32,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    confirm_identification_inner(&file_path, anime_id, episode, &state).await
}

#[tauri::command]
pub async fn list_known_files(
    limit: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<FileIndexRow>, String> {
    list_known_files_inner(limit, &state).await
}

// ── AniList command wrappers ────────────────────────────────────────────────

#[tauri::command]
pub async fn store_anilist_token(
    state: tauri::State<'_, EngineState>,
    token: String,
) -> Result<(), String> {
    store_anilist_token_inner(&token, &state)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disconnect_anilist(
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    disconnect_anilist_inner(&state)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_anilist_library(
    state: tauri::State<'_, EngineState>,
) -> Result<ImportReport, String> {
    import_anilist_library_inner(&state)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_sync_status(
    state: tauri::State<'_, EngineState>,
) -> Result<SyncStatus, String> {
    get_sync_status_inner(&state)
        .await
        .map_err(|e| e.to_string())
}
