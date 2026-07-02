use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::watch;

use crate::engine::anilist::auth;
use crate::engine::anilist::client::AniListClient;
use crate::engine::anilist::import::{import_library, ImportReport};
use crate::engine::anilist::oauth;
use crate::engine::events::EngineEvent;
use crate::engine::library_scanner;
use crate::engine::matcher::{confirm_identification as matcher_confirm, recognize_file, RecognitionResult};
use crate::engine::migration::{backup, discovery, importer, DuplicateStrategy, MigrationReport, V1DataPaths};
use crate::engine::runtime::EngineState;
use crate::engine::storage::{AnimeStats, ContinueWatchingRow, FileIndexRow, LibraryRow, LibraryStats, WatchHistoryFullRow, WatchHistoryRow};
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncResult {
    pub processed: usize,
    pub failed: usize,
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
    // Validate token before storing
    let client = AniListClient::new(token.to_string());
    client.fetch_user_list(None).await.map_err(|e| {
        anyhow::anyhow!(
            "Invalid AniList token. Make sure you copied the Access Token (not the Client ID) from https://anilist.co/settings/developer. Error: {}",
            e
        )
    })?;

    auth::store_token(&state.storage, token).await?;
    Ok(())
}

pub async fn connect_anilist_oauth_inner(
    client_id: String,
    client_secret: String,
    state: &EngineState,
) -> anyhow::Result<()> {
    // Start OAuth flow
    let token = oauth::start_oauth_flow(&client_id, &client_secret).await?;

    // Store credentials and token
    auth::store_client_credentials(&state.storage, &client_id, &client_secret).await?;
    auth::store_token(&state.storage, &token).await?;

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

pub async fn get_anilist_connection_status_inner(state: &EngineState) -> anyhow::Result<bool> {
    auth::is_connected(&state.storage).await
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

    // Auto-complete if last episode marked
    if let Ok(detail) = state.storage.anime_detail(anime_id).await {
        if let Some(count) = detail.episode_count {
            if count > 0 && episode >= count {
                let _ = state
                    .storage
                    .update_list_entry_partial(anime_id, Some("completed"), None, None)
                    .await;
            }
        }
    }

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
    let detail = match state.storage.anime_detail(anime_id).await {
        Ok(d) => d,
        Err(_) => {
            // Auto-import from AniList if not found locally
            if let Ok(token) = crate::engine::anilist::auth::load_token(&state.storage).await {
                if let Some(token) = token {
                    let client = crate::engine::anilist::client::AniListClient::new(token);
                    let query_str = format!(
                        "query {{ Media(id: {}, type: ANIME) {{ id title {{ romaji english native }} episodes type status coverImage {{ large }} description }} }}",
                        anime_id
                    );
                    if let Ok(raw) = client.query::<serde_json::Value>(&query_str, serde_json::json!({})).await {
                        let media = raw.get("data").and_then(|d| d.get("Media"));
                        if let Some(media) = media {
                            let title = media.get("title");
                            let titles_json = serde_json::json!({
                                "romaji": title.and_then(|t| t.get("romaji")).and_then(|r| r.as_str()).unwrap_or(""),
                                "english": title.and_then(|t| t.get("english")).and_then(|e| e.as_str()),
                                "japanese": title.and_then(|t| t.get("native")).and_then(|n| n.as_str()),
                                "synonyms": [],
                            }).to_string();
                            let ep_count = media.get("episodes").and_then(|e| e.as_i64()).unwrap_or(0) as i32;
                            let image_url = media.get("coverImage").and_then(|c| c.get("large")).and_then(|l| l.as_str());
                            let synopsis = media.get("description").and_then(|d| d.as_str());
                            let anime_type = media.get("type").and_then(|t| t.as_str());
                            let anime_status = media.get("status").and_then(|s| s.as_str());
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64;

                            let _ = state.storage.upsert_anime_full(
                                anime_id,
                                &titles_json,
                                ep_count,
                                image_url,
                                synopsis,
                                anime_type,
                                anime_status,
                                now,
                            ).await;
                        }
                    }
                }
            }
            // Retry — should succeed after insert above
            state.storage.anime_detail(anime_id).await?
        }
    };

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

// ── Library scanner command inner functions ─────────────────────────────────

pub async fn get_library_folders_inner(state: &EngineState) -> anyhow::Result<Vec<String>> {
    library_scanner::get_library_folders(&state.storage).await
}

pub async fn set_library_folders_inner(
    state: &EngineState,
    folders: Vec<String>,
) -> anyhow::Result<()> {
    library_scanner::set_library_folders(&state.storage, folders).await
}

pub async fn scan_library_folders_inner(
    state: &EngineState,
) -> anyhow::Result<library_scanner::LibraryScanReport> {
    library_scanner::scan_library_folders(&state.storage).await
}

pub async fn get_episode_files_inner(
    state: &EngineState,
    anime_id: i64,
) -> anyhow::Result<Vec<FileIndexRow>> {
    state.storage.file_index_by_anime(anime_id).await
}

pub async fn open_episode_file_inner(path: String) -> anyhow::Result<()> {
    library_scanner::open_file(&path)
}

// ── Sonarr command inner functions ──────────────────────────────────────────

async fn load_sonarr_connection(state: &EngineState) -> Option<(String, String)> {
    let url_raw = state.storage.get_setting("sonarr.url").await.ok()??;
    let url: String = serde_json::from_str(&url_raw).ok()?;

    let encrypted = state.storage.get_setting("sonarr.api_key").await.ok()??;
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
    let now = unix_now().map_err(|e| anyhow::anyhow!(e))?;

    state.storage.set_setting("sonarr.url", &url_json, now).await?;
    state.storage.set_setting("sonarr.api_key", &encrypted_key, now).await?;

    // Import series
    crate::engine::sonarr::import::import_sonarr_series(&client, &state.storage).await?;

    Ok(())
}

pub async fn disconnect_sonarr_inner(state: &EngineState) -> anyhow::Result<()> {
    state.storage.delete_setting("sonarr.url").await?;
    state.storage.delete_setting("sonarr.api_key").await?;
    state.storage.delete_setting("sonarr.last_sync_at").await?;
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
    let (url, api_key) = load_sonarr_connection(state).await
        .ok_or_else(|| anyhow::anyhow!("Sonarr not connected"))?;
    let client = SonarrClient::new(url, api_key);
    let report = crate::engine::sonarr::import::import_sonarr_series(&client, &state.storage).await?;

    // Update last sync time
    let now = unix_now().map_err(|e| anyhow::anyhow!(e))?;
    let now_json = serde_json::to_string(&now)?;
    state.storage.set_setting("sonarr.last_sync_at", &now_json, now).await?;

    Ok(report)
}

pub async fn test_sonarr_connection_inner(url: &str, api_key: &str) -> anyhow::Result<()> {
    let client = SonarrClient::new(url.to_string(), api_key.to_string());
    client.validate_connection().await?;
    Ok(())
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

// ── Migration command inner functions ──────────────────────────────────────

pub async fn discover_v1_data_inner() -> Result<V1DataPaths, String> {
    Ok(discovery::discover_v1_data())
}

pub async fn preview_migration_inner(_state: &EngineState) -> Result<MigrationReport, String> {
    let paths = discovery::discover_v1_data();
    importer::dry_run_import(&paths).await.map_err(command_error)
}

pub async fn run_migration_inner(
    state: &EngineState,
    strategy: DuplicateStrategy,
) -> Result<MigrationReport, String> {
    let paths = discovery::discover_v1_data();
    if !paths.found {
        return Err("No v1 data found. Cannot run migration.".to_string());
    }
    // Backup first
    if let Err(e) = backup::backup_database(&state.storage).await {
        tracing::warn!("Backup failed (continuing): {}", e);
    }
    importer::live_import(&state.storage, &paths, strategy)
        .await
        .map_err(command_error)
}

pub async fn backup_database_inner(state: &EngineState) -> Result<String, String> {
    backup::backup_database(&state.storage)
        .await
        .map_err(command_error)
}

pub async fn restore_database_inner(
    state: &EngineState,
    backup_path: String,
) -> Result<String, String> {
    backup::restore_database(&state.storage, &backup_path)
        .await
        .map_err(command_error)
}

pub async fn export_database_inner(state: &EngineState) -> Result<String, String> {
    backup::export_database(&state.storage)
        .await
        .map_err(command_error)
}

pub async fn import_database_inner(
    state: &EngineState,
    json: String,
) -> Result<MigrationReport, String> {
    backup::import_database(&state.storage, &json)
        .await
        .map_err(command_error)
}

// ── Season Browser ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct SeasonAnimeEntry {
    pub id: i64,
    pub title: String,
    pub image_url: Option<String>,
    pub episodes: Option<i32>,
    pub status: Option<String>,
    pub format: Option<String>,
    pub average_score: Option<i32>,
    pub popularity: Option<i32>,
}

pub async fn get_season_anime_inner(
    state: &EngineState,
    season: String,
    year: i32,
    genre: Option<String>,
) -> anyhow::Result<Vec<SeasonAnimeEntry>> {
    let token = crate::engine::anilist::auth::load_token(&state.storage).await?.ok_or_else(|| anyhow::anyhow!("not connected"))?;
    let client = AniListClient::new(token);
    let entries = client.fetch_season_anime(&season, year, genre.as_deref()).await?;
    Ok(entries.into_iter().map(|e| SeasonAnimeEntry {
        id: e.id,
        title: e.title.as_ref().and_then(|t| t.english.clone()).or_else(|| e.title.as_ref().and_then(|t| t.romaji.clone())).unwrap_or_else(|| format!("#{}", e.id)),
        image_url: e.cover_image.and_then(|c| c.large),
        episodes: e.episodes,
        status: e.status,
        format: e.format,
        average_score: e.average_score,
        popularity: e.popularity,
    }).collect())
}

#[tauri::command]
pub async fn get_season_anime(
    season: String,
    year: i32,
    genre: Option<String>,
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<SeasonAnimeEntry>, String> {
    get_season_anime_inner(&state, season, year, genre).await.map_err(command_error)
}

// ── Anime Relations ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct RelationEntry {
    pub id: i64,
    pub title: String,
    pub relation_type: String,
    pub format: Option<String>,
    pub status: Option<String>,
    pub image_url: Option<String>,
}

pub async fn get_anime_relations_inner(state: &EngineState, anime_id: i64) -> anyhow::Result<Vec<RelationEntry>> {
    let token = crate::engine::anilist::auth::load_token(&state.storage).await?.ok_or_else(|| anyhow::anyhow!("not connected"))?;
    let client = AniListClient::new(token);
    let edges = client.fetch_anime_relations(anime_id).await?;
    Ok(edges.into_iter().filter_map(|e| {
        let node = e.node?;
        let title = node.title.as_ref()
            .and_then(|t| t.romaji.clone().or_else(|| t.english.clone()))
            .unwrap_or_else(|| format!("#{}", node.id));
        Some(RelationEntry {
            id: node.id,
            title,
            relation_type: e.relation_type,
            format: node.format,
            status: node.status,
            image_url: node.cover_image.and_then(|c| c.large),
        })
    }).collect())
}

#[tauri::command]
pub async fn get_anime_relations(anime_id: i64, state: tauri::State<'_, EngineState>) -> Result<Vec<RelationEntry>, String> {
    get_anime_relations_inner(&state, anime_id).await.map_err(command_error)
}

// ── Calendar ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct CalendarEntry {
    pub anime_id: i64,
    pub title: String,
    pub image_url: Option<String>,
    pub episode_count: Option<i32>,
    pub progress: Option<i32>,
    pub next_episode: Option<i32>,
    pub airing_at: Option<i64>,
    pub time_until_airing: Option<i64>,
}

pub async fn get_calendar_inner(state: &EngineState) -> anyhow::Result<Vec<CalendarEntry>> {
    use crate::engine::anilist::client::AniListClient;
    let token = crate::engine::anilist::auth::load_token(&state.storage)
        .await?
        .ok_or_else(|| anyhow::anyhow!("AniList not connected"))?;
    let client = AniListClient::new(token);
    let entries = client.fetch_airing_schedule().await?;

    Ok(entries.into_iter().filter_map(|e| {
        let media = e.media?;
        let title = media.title.as_ref()?
            .romaji.clone()
            .or_else(|| media.title.as_ref()?.english.clone())
            .unwrap_or_else(|| format!("Anime #{}", media.id));
        let next_ep = media.next_airing_episode.as_ref();
        Some(CalendarEntry {
            anime_id: media.id,
            title,
            image_url: media.cover_image.as_ref().and_then(|c| c.large.clone()),
            episode_count: media.episodes,
            progress: e.progress,
            next_episode: next_ep.map(|e| e.episode),
            airing_at: next_ep.map(|e| e.airing_at),
            time_until_airing: next_ep.map(|e| e.time_until_airing),
        })
    }).collect())
}

pub async fn get_statistics_inner(state: &EngineState) -> anyhow::Result<AnimeStats> {
    state.storage.compute_stats().await
}

#[tauri::command]
pub async fn get_statistics(state: tauri::State<'_, EngineState>) -> Result<AnimeStats, String> {
    get_statistics_inner(&state).await.map_err(command_error)
}

pub async fn continue_watching_inner(state: &EngineState) -> anyhow::Result<Vec<ContinueWatchingRow>> {
    state.storage.continue_watching(10).await
}

#[tauri::command]
pub async fn continue_watching(state: tauri::State<'_, EngineState>) -> Result<Vec<ContinueWatchingRow>, String> {
    continue_watching_inner(&state).await.map_err(command_error)
}

// Tauri command wrappers

#[tauri::command]
pub async fn get_calendar(
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<CalendarEntry>, String> {
    get_calendar_inner(&state).await.map_err(command_error)
}

pub async fn get_watch_history_inner(
    state: &EngineState,
    query: Option<String>,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<WatchHistoryFullRow>> {
    match query.filter(|q| !q.is_empty()) {
        Some(q) => state.storage.search_watch_history(&q, limit, offset).await,
        None => state.storage.list_all_watch_history(limit, offset).await,
    }
}

#[tauri::command]
pub async fn get_watch_history(
    query: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<WatchHistoryFullRow>, String> {
    get_watch_history_inner(&state, query, limit.unwrap_or(100), offset.unwrap_or(0))
        .await
        .map_err(command_error)
}

#[tauri::command]
pub async fn get_engine_status(
    state: tauri::State<'_, EngineState>,
) -> Result<EngineStatus, String> {
    get_engine_status_inner(&state).await
}

// ── Migration command wrappers ─────────────────────────────────────────────

#[tauri::command]
pub async fn backup_database(
    state: tauri::State<'_, EngineState>,
) -> Result<String, String> {
    backup_database_inner(&state).await
}

#[tauri::command]
pub async fn discover_v1_data() -> Result<V1DataPaths, String> {
    discover_v1_data_inner().await
}

#[tauri::command]
pub async fn export_database(
    state: tauri::State<'_, EngineState>,
) -> Result<String, String> {
    export_database_inner(&state).await
}

#[tauri::command]
pub async fn import_database(
    json: String,
    state: tauri::State<'_, EngineState>,
) -> Result<MigrationReport, String> {
    import_database_inner(&state, json).await
}

#[tauri::command]
pub async fn preview_migration(
    state: tauri::State<'_, EngineState>,
) -> Result<MigrationReport, String> {
    preview_migration_inner(&state).await
}

#[tauri::command]
pub async fn restore_database(
    backup_path: String,
    state: tauri::State<'_, EngineState>,
) -> Result<String, String> {
    restore_database_inner(&state, backup_path).await
}

#[tauri::command]
pub async fn run_migration(
    strategy: DuplicateStrategy,
    state: tauri::State<'_, EngineState>,
) -> Result<MigrationReport, String> {
    run_migration_inner(&state, strategy).await
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
pub async fn connect_anilist_oauth(
    client_id: String,
    client_secret: String,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    connect_anilist_oauth_inner(client_id, client_secret, &state)
        .await
        .map_err(|e| e.to_string())
}

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
pub async fn get_anilist_connection_status(
    state: tauri::State<'_, EngineState>,
) -> Result<bool, String> {
    get_anilist_connection_status_inner(&state)
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

pub async fn trigger_sync_inner(state: &EngineState) -> anyhow::Result<SyncResult> {
    let (pending_before, _, _) = state.storage.sync_status_counts("anilist").await?;
    crate::engine::sync_worker::drain_queue(state).await?;
    let (pending_after, failed, _) = state.storage.sync_status_counts("anilist").await?;
    Ok(SyncResult {
        processed: (pending_before as usize).saturating_sub(pending_after as usize),
        failed: failed as usize,
    })
}

#[tauri::command]
pub async fn trigger_sync(state: tauri::State<'_, EngineState>) -> Result<SyncResult, String> {
    trigger_sync_inner(&state).await.map_err(command_error)
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

// ── Library scanner command wrappers ────────────────────────────────────────

#[tauri::command]
pub async fn get_library_folders(
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<String>, String> {
    get_library_folders_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn set_library_folders(
    folders: Vec<String>,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    set_library_folders_inner(&state, folders)
        .await
        .map_err(command_error)
}

#[tauri::command]
pub async fn scan_library_folders(
    state: tauri::State<'_, EngineState>,
) -> Result<library_scanner::LibraryScanReport, String> {
    scan_library_folders_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn get_episode_files(
    anime_id: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<FileIndexRow>, String> {
    get_episode_files_inner(&state, anime_id)
        .await
        .map_err(command_error)
}

#[tauri::command]
pub async fn open_episode_file(path: String) -> Result<(), String> {
    open_episode_file_inner(path).await.map_err(command_error)
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

#[tauri::command]
pub async fn test_sonarr_connection(
    url: String,
    api_key: String,
) -> Result<(), String> {
    test_sonarr_connection_inner(&url, &api_key)
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
