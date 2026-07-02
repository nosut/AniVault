use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::watch;

use crate::engine::anilist::auth;
use crate::engine::anilist::client::AniListClient;
use crate::engine::anilist::import::{import_library, ImportReport};
use crate::engine::events::EngineEvent;
use crate::engine::matcher::{confirm_identification as matcher_confirm, recognize_file, RecognitionResult};
use crate::engine::migration::MigrationReport;
use crate::engine::runtime::EngineState;
use crate::engine::storage::{FileIndexRow, LibraryRow, LibraryStats, WatchHistoryRow};
use crate::engine::sonarr::client::SonarrClient;
use crate::engine::tracker::run_tracking_loop;
use tauri_plugin_notification::NotificationExt;

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

#[derive(Debug, Clone, serde::Serialize)]
pub struct AnimeDetailResponse {
    pub anime_id: i64,
    pub titles_json: String,
    pub episode_count: Option<i32>,
    pub image_url: Option<String>,
    pub synopsis: Option<String>,
    pub anime_status: Option<String>,
    pub last_modified: i64,
    pub list_status: Option<String>,
    pub watched_episodes: Option<i32>,
    pub score: Option<i32>,
    pub notes: Option<String>,
    pub local_updated: Option<i64>,
    pub remote_updated: Option<i64>,
    pub tracker_id: Option<String>,
    pub recent_history: Vec<WatchHistoryRow>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionState {
    pub paused: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SonarrStatus {
    pub connected: bool,
    pub series_count: i64,
    pub mapped_count: i64,
    pub last_sync_at: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SonarrAvailabilityResponse {
    pub sonarr_id: i64,
    pub sonarr_title: String,
    pub monitored: bool,
    pub episode_count: i32,
    pub episode_file_count: i32,
    pub next_airing: Option<i64>,
    pub path: Option<String>,
    pub season_count: i32,
    pub sonarr_status: Option<String>,
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

pub async fn get_session_state_inner(state: &EngineState) -> anyhow::Result<SessionState> {
    Ok(SessionState {
        paused: state.tracking_paused.load(std::sync::atomic::Ordering::Relaxed),
    })
}

pub async fn toggle_pause_tracking_inner(state: &EngineState) -> anyhow::Result<SessionState> {
    let current = state.tracking_paused.load(std::sync::atomic::Ordering::Relaxed);
    state
        .tracking_paused
        .store(!current, std::sync::atomic::Ordering::Relaxed);
    Ok(SessionState { paused: !current })
}

pub async fn get_launch_on_startup_inner(state: &EngineState) -> anyhow::Result<bool> {
    let val: Option<String> = state.storage.get_setting("startup.launch_on_startup").await?;
    match val {
        Some(s) => {
            let v: serde_json::Value = serde_json::from_str(&s)?;
            Ok(v.as_bool().unwrap_or(false))
        }
        None => Ok(false),
    }
}

pub async fn set_launch_on_startup_inner(enabled: bool, state: &EngineState) -> anyhow::Result<()> {
    let value_json = serde_json::to_string(&serde_json::Value::Bool(enabled))?;
    state
        .storage
        .set_setting("startup.launch_on_startup", &value_json, unix_now_inner()?)
        .await?;
    // Only write registry when a real Tauri app handle exists (not in tests)
    if state.app_handle.is_some() {
        let exe_path = std::env::current_exe()?.to_string_lossy().to_string();
        if enabled {
            let output = std::process::Command::new("reg")
                .args([
                    "add",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v",
                    "AniVault",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &exe_path,
                    "/f",
                ])
                .output()?;
            if !output.status.success() {
                anyhow::bail!("Failed to write registry key");
            }
        } else {
            // Delete key; ignore error if key doesn't exist
            let _ = std::process::Command::new("reg")
                .args([
                    "delete",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v",
                    "AniVault",
                    "/f",
                ])
                .output()?;
        }
    }
    Ok(())
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

    if !state.tracking_paused.load(std::sync::atomic::Ordering::Relaxed) {
        if let Some(ref handle) = state.app_handle {
            let title = state
                .storage
                .anime_detail(anime_id)
                .await
                .map(|d| {
                    serde_json::from_str::<serde_json::Value>(&d.titles_json)
                        .ok()
                        .and_then(|v| v.get("romaji").and_then(|r| r.as_str()).map(String::from))
                        .unwrap_or_else(|| format!("Anime #{}", anime_id))
                })
                .unwrap_or_else(|_| format!("Anime #{}", anime_id));

            let _ = handle
                .notification()
                .builder()
                .title(title)
                .body(format!("Episode {} watched", episode))
                .show();
        }
    }

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

// ── Library command inner functions ─────────────────────────────────────────

pub async fn search_library_inner(
    query: String,
    status_filter: Option<String>,
    limit: i64,
    offset: i64,
    state: &EngineState,
) -> anyhow::Result<Vec<LibraryRow>> {
    state
        .storage
        .search_library(&query, status_filter.as_deref(), limit, offset)
        .await
}

pub async fn get_library_stats_inner(state: &EngineState) -> anyhow::Result<LibraryStats> {
    state.storage.library_stats().await
}

pub async fn fetch_anime_detail_inner(
    anime_id: i64,
    state: &EngineState,
) -> anyhow::Result<AnimeDetailResponse> {
    let detail = state.storage.anime_detail(anime_id).await?;
    let recent_history = state.storage.list_recent_watch_history(10).await?;
    Ok(AnimeDetailResponse {
        anime_id: detail.anime_id,
        titles_json: detail.titles_json,
        episode_count: detail.episode_count,
        image_url: detail.image_url,
        synopsis: detail.synopsis,
        anime_status: detail.anime_status,
        last_modified: detail.last_modified,
        list_status: detail.list_status,
        watched_episodes: detail.watched_episodes,
        score: detail.score,
        notes: detail.notes,
        local_updated: detail.local_updated,
        remote_updated: detail.remote_updated,
        tracker_id: detail.tracker_id,
        recent_history,
    })
}

pub async fn update_list_entry_inner(
    anime_id: i64,
    status: Option<String>,
    watched_episodes: Option<i32>,
    score: Option<i32>,
    state: &EngineState,
) -> anyhow::Result<()> {
    state
        .storage
        .update_list_entry_partial(anime_id, status.as_deref(), watched_episodes, score)
        .await
}

// ── Sonarr command inner functions ──────────────────────────────────────────

fn load_sonarr_connection(state: &EngineState) -> Option<(String, String)> {
    let state1 = state.clone();
    let url: Option<String> = std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            state1.storage.get_setting("sonarr.url").await.ok().flatten()
        })
    }).join().ok().flatten();

    let url = url?;
    let url = serde_json::from_str::<String>(&url).ok()?;

    let state2 = state.clone();
    let encrypted: Option<String> = std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            state2.storage.get_setting("sonarr.api_key").await.ok().flatten()
        })
    }).join().ok().flatten();

    let encrypted = encrypted?;
    let api_key = crate::engine::secrets::unprotect_secret(&encrypted).ok()?;

    Some((url, api_key))
}

pub async fn connect_sonarr_inner(url: &str, api_key: &str, state: &EngineState) -> anyhow::Result<()> {
    let client = SonarrClient::new(url.to_string(), api_key.to_string());

    // Validate connection
    client.validate_connection().await?;

    // Store settings
    let url_json = serde_json::to_string(url)?;
    let encrypted_key = crate::engine::secrets::protect_secret(api_key)?;
    let encrypted_json = serde_json::to_string(&encrypted_key)?;
    let now = unix_now().map_err(|e| anyhow::anyhow!(e))?;

    state.storage.set_setting("sonarr.url", &url_json, now).await?;
    state.storage.set_setting("sonarr.api_key", &encrypted_json, now).await?;

    // Import series
    crate::engine::sonarr::import::import_sonarr_series(&client, &state.storage).await?;

    Ok(())
}

pub async fn disconnect_sonarr_inner(state: &EngineState) -> anyhow::Result<()> {
    state.storage.delete_setting("sonarr.url").await?;
    state.storage.delete_setting("sonarr.api_key").await?;
    state.storage.sonarr_mapping_delete_all().await?;
    state.storage.sonarr_series_delete_all().await?;
    Ok(())
}

pub async fn get_sonarr_status_inner(state: &EngineState) -> anyhow::Result<SonarrStatus> {
    let connected = state.storage.get_setting("sonarr.api_key").await?.is_some();
    let series_count = if connected {
        state.storage.sonarr_series_count().await?
    } else {
        0
    };
    let mapped_count = if connected {
        state.storage.sonarr_mapping_count().await?
    } else {
        0
    };
    let last_sync_at = if connected {
        state.storage.get_setting("sonarr.last_sync_at").await?
            .and_then(|v| serde_json::from_str::<i64>(&v).ok())
    } else {
        None
    };

    Ok(SonarrStatus {
        connected,
        series_count,
        mapped_count,
        last_sync_at,
    })
}

pub async fn import_sonarr_series_inner(state: &EngineState) -> anyhow::Result<crate::engine::sonarr::import::ImportReport> {
    let (url, api_key) = load_sonarr_connection(state)
        .ok_or_else(|| anyhow::anyhow!("Sonarr not connected"))?;
    let client = SonarrClient::new(url, api_key);
    let report = crate::engine::sonarr::import::import_sonarr_series(&client, &state.storage).await?;

    // Update last sync time
    let now = unix_now().map_err(|e| anyhow::anyhow!(e))?;
    let now_json = serde_json::to_string(&now)?;
    state.storage.set_setting("sonarr.last_sync_at", &now_json, now).await?;

    Ok(report)
}

pub async fn get_sonarr_availability_inner(
    anime_id: i64,
    state: &EngineState,
) -> anyhow::Result<Option<SonarrAvailabilityResponse>> {
    let row = state.storage.sonarr_availability(anime_id).await?;
    Ok(row.map(|r| SonarrAvailabilityResponse {
        sonarr_id: r.sonarr_id,
        sonarr_title: r.sonarr_title,
        monitored: r.monitored,
        episode_count: r.episode_count,
        episode_file_count: r.episode_file_count,
        next_airing: r.next_airing,
        path: r.path,
        season_count: r.season_count,
        sonarr_status: r.sonarr_status,
    }))
}

pub async fn remap_sonarr_inner(
    sonarr_id: i64,
    anime_id: Option<i64>,
    state: &EngineState,
) -> anyhow::Result<()> {
    let now = unix_now().map_err(|e| anyhow::anyhow!(e))?;

    // Update existing mapping or insert new
    let mapping = crate::engine::storage::SonarrMappingDb {
        id: None,
        sonarr_id,
        anime_id,
        title_match: "manual".into(),
        confidence: if anime_id.is_some() { 100 } else { 0 },
        mapped_at: now,
        user_confirmed: true,
    };
    state.storage.sonarr_mapping_upsert(&mapping).await?;
    Ok(())
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

// ── Library command wrappers ───────────────────────────────────────────────

#[tauri::command]
pub async fn search_library(
    query: String,
    status_filter: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<LibraryRow>, String> {
    search_library_inner(query, status_filter, limit.unwrap_or(50), offset.unwrap_or(0), &state)
        .await
        .map_err(command_error)
}

#[tauri::command]
pub async fn get_library_stats(
    state: tauri::State<'_, EngineState>,
) -> Result<LibraryStats, String> {
    get_library_stats_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn fetch_anime_detail(
    anime_id: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<AnimeDetailResponse, String> {
    fetch_anime_detail_inner(anime_id, &state).await.map_err(command_error)
}

#[tauri::command]
pub async fn update_list_entry(
    anime_id: i64,
    status: Option<String>,
    watched_episodes: Option<i32>,
    score: Option<i32>,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    update_list_entry_inner(anime_id, status, watched_episodes, score, &state)
        .await
        .map_err(command_error)
}

// ── Sonarr command wrappers ─────────────────────────────────────────────────

#[tauri::command]
pub async fn connect_sonarr(
    url: String,
    api_key: String,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    connect_sonarr_inner(&url, &api_key, &state)
        .await
        .map_err(command_error)
}

#[tauri::command]
pub async fn disconnect_sonarr(
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    disconnect_sonarr_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn get_sonarr_status(
    state: tauri::State<'_, EngineState>,
) -> Result<SonarrStatus, String> {
    get_sonarr_status_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn import_sonarr_series(
    state: tauri::State<'_, EngineState>,
) -> Result<crate::engine::sonarr::import::ImportReport, String> {
    import_sonarr_series_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn get_sonarr_availability(
    anime_id: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<Option<SonarrAvailabilityResponse>, String> {
    get_sonarr_availability_inner(anime_id, &state)
        .await
        .map_err(command_error)
}

#[tauri::command]
pub async fn remap_sonarr(
    sonarr_id: i64,
    anime_id: Option<i64>,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    remap_sonarr_inner(sonarr_id, anime_id, &state)
        .await
        .map_err(command_error)
}

// ── Session commands ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_session_state(
    state: tauri::State<'_, EngineState>,
) -> Result<SessionState, String> {
    get_session_state_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn toggle_pause_tracking(
    state: tauri::State<'_, EngineState>,
) -> Result<SessionState, String> {
    toggle_pause_tracking_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn get_launch_on_startup(
    state: tauri::State<'_, EngineState>,
) -> Result<bool, String> {
    get_launch_on_startup_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn set_launch_on_startup(
    enabled: bool,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    set_launch_on_startup_inner(enabled, &state)
        .await
        .map_err(command_error)
}
