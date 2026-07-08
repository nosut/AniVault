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
    let last_sync_at = state
        .storage
        .get_setting("anilist.last_sync_at")
        .await?
        .and_then(|v| v.parse::<i64>().ok());
    Ok(SyncStatus {
        pending,
        failed,
        blocked,
        last_sync_at,
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

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE_NAME: &str = "AniVault";

/// The value the HKCU Run entry should hold, or `None` when the entry should be
/// absent. When `start_in_tray` is set the app launches with `--minimized` so
/// autostart goes straight to the tray. Pure so the toggle path and the
/// launch-time reconcile share one source of truth and it can be unit-tested
/// without touching the registry.
pub fn desired_run_value(enabled: bool, start_in_tray: bool, exe_path: &str) -> Option<String> {
    if !enabled {
        return None;
    }
    Some(if start_in_tray {
        format!("\"{exe_path}\" --minimized")
    } else {
        format!("\"{exe_path}\"")
    })
}

/// Write (`Some`) or remove (`None`) the AniVault HKCU Run entry via `reg`.
fn write_run_key(value: Option<&str>) -> anyhow::Result<()> {
    match value {
        Some(v) => {
            let output = std::process::Command::new("reg")
                .args([
                    "add", RUN_KEY, "/v", RUN_VALUE_NAME, "/t", "REG_SZ", "/d", v, "/f",
                ])
                .output()?;
            if !output.status.success() {
                anyhow::bail!("Failed to write registry key");
            }
        }
        None => {
            // Deleting an absent value is a no-op that returns a nonzero exit;
            // ignore it so turning the feature off never surfaces an error.
            let _ = std::process::Command::new("reg")
                .args(["delete", RUN_KEY, "/v", RUN_VALUE_NAME, "/f"])
                .output()?;
        }
    }
    Ok(())
}

/// Write (or remove) the HKCU Run registry entry to match the requested state,
/// pointing at the current exe.
fn apply_startup_registry(
    enabled: bool,
    start_in_tray: bool,
    state: &EngineState,
) -> anyhow::Result<()> {
    // Only touch the registry when a real Tauri app handle exists (not in tests).
    if state.app_handle.is_none() {
        return Ok(());
    }
    let exe_path = std::env::current_exe()?.to_string_lossy().to_string();
    write_run_key(desired_run_value(enabled, start_in_tray, &exe_path).as_deref())
}

/// Reconcile the HKCU Run entry with the persisted launch-on-startup setting on
/// every launch. This self-heals a stale exe path left by a reinstall, an app
/// move, or a bundle-identifier change — the entry is always rewritten to point
/// at the exe that is actually running. Best-effort: logs and swallows errors so
/// a registry hiccup never blocks startup. No-op when the feature is disabled
/// (the toggle already removed the entry) or in headless/test contexts.
pub async fn reconcile_startup_registry(state: &EngineState) {
    if state.app_handle.is_none() {
        return;
    }
    if !get_launch_on_startup_inner(state).await.unwrap_or(false) {
        return;
    }
    let start_in_tray = get_start_in_tray_inner(state).await.unwrap_or(false);
    let exe_path = match std::env::current_exe() {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(e) => {
            tracing::warn!("startup reconcile: cannot resolve current exe: {e}");
            return;
        }
    };
    match write_run_key(desired_run_value(true, start_in_tray, &exe_path).as_deref()) {
        Ok(()) => tracing::info!("launch-on-startup reconciled to {exe_path}"),
        Err(e) => tracing::warn!("startup registry reconcile failed: {e}"),
    }
}

pub async fn set_launch_on_startup_inner(enabled: bool, state: &EngineState) -> anyhow::Result<()> {
    let value_json = serde_json::to_string(&serde_json::Value::Bool(enabled))?;
    state
        .storage
        .set_setting("startup.launch_on_startup", &value_json, unix_now_inner()?)
        .await?;
    let tray = get_start_in_tray_inner(state).await?;
    apply_startup_registry(enabled, tray, state)
}

pub async fn get_start_in_tray_inner(state: &EngineState) -> anyhow::Result<bool> {
    let val: Option<String> = state.storage.get_setting("startup.start_in_tray").await?;
    match val {
        Some(s) => Ok(serde_json::from_str::<serde_json::Value>(&s)?
            .as_bool()
            .unwrap_or(false)),
        None => Ok(false),
    }
}

pub async fn set_start_in_tray_inner(enabled: bool, state: &EngineState) -> anyhow::Result<()> {
    let value_json = serde_json::to_string(&serde_json::Value::Bool(enabled))?;
    state
        .storage
        .set_setting("startup.start_in_tray", &value_json, unix_now_inner()?)
        .await?;
    // Re-apply the registry so the --minimized flag matches, if autostart is on.
    let launch = get_launch_on_startup_inner(state).await?;
    apply_startup_registry(launch, enabled, state)
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
        .append_watch_history(anime_id, episode, None, Some("manual"), "manual", unix_now()?)
        .await
        .map_err(command_error)?;

    state
        .storage
        .upsert_list_entry_progress(anime_id, "watching", episode, unix_now()?)
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

    // Push status + progress back to AniList (best-effort, queued), mirroring
    // the auto-detect path. After the auto-complete above so the queued status
    // is the final one.
    crate::engine::sync_worker::enqueue_anilist_sync(state, anime_id).await;

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

pub async fn rematch_unmapped_files_inner(state: &EngineState) -> anyhow::Result<usize> {
    let files = state.storage.list_file_index(100_000, 0).await?;
    let mut rematched = 0usize;
    let now = unix_now_inner()?;

    for file in &files {
        // Never touch tombstoned files or user-confirmed / exact (100%) mappings.
        if file.ignored || file.confidence >= 100 {
            continue;
        }

        // Re-evaluate with the same scored logic the scanner uses, storing the
        // real confidence. This both matches previously-unmatched files and
        // corrects/demotes stale low-confidence auto-matches from older runs.
        let path = std::path::Path::new(&file.file_path);
        let (anime_id, confidence, episode) =
            crate::engine::library_scanner::match_file(&state.storage, path).await?;
        let episode = episode.unwrap_or(file.episode.unwrap_or(0));

        // Only write when something actually changed, to avoid needless churn.
        if anime_id != file.anime_id || confidence != file.confidence {
            state
                .storage
                .upsert_file_index(&file.file_path, anime_id, episode, confidence, now)
                .await?;
        }

        if anime_id.is_some() {
            rematched += 1;
        }
    }

    Ok(rematched)
}

#[tauri::command]
pub async fn rematch_unmapped_files(
    state: tauri::State<'_, EngineState>,
) -> Result<usize, String> {
    rematch_unmapped_files_inner(&state).await.map_err(command_error)
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

/// Fetch a single anime's metadata from AniList by id and upsert it into the
/// local `anime` table. Shared by detail auto-import and the file manager's
/// "search AniList" mapping flow. Errors if AniList isn't connected or the id
/// isn't found.
pub async fn import_anime_from_anilist(state: &EngineState, anime_id: i64) -> anyhow::Result<()> {
    let token = crate::engine::anilist::auth::load_token(&state.storage)
        .await?
        .ok_or_else(|| anyhow::anyhow!("not connected to AniList"))?;
    let client = crate::engine::anilist::client::AniListClient::new(token);
    let query_str = format!(
        "query {{ Media(id: {}, type: ANIME) {{ id title {{ romaji english native }} episodes type status coverImage {{ large }} description season seasonYear }} }}",
        anime_id
    );
    let raw = client
        .query::<serde_json::Value>(&query_str, serde_json::json!({}))
        .await?;
    let media = raw
        .get("data")
        .and_then(|d| d.get("Media"))
        .filter(|m| !m.is_null())
        .ok_or_else(|| anyhow::anyhow!("anime #{anime_id} not found on AniList"))?;

    let title = media.get("title");
    let titles_json = serde_json::json!({
        "romaji": title.and_then(|t| t.get("romaji")).and_then(|r| r.as_str()).unwrap_or(""),
        "english": title.and_then(|t| t.get("english")).and_then(|e| e.as_str()),
        "japanese": title.and_then(|t| t.get("native")).and_then(|n| n.as_str()),
        "synonyms": [],
    })
    .to_string();
    let ep_count = media.get("episodes").and_then(|e| e.as_i64()).unwrap_or(0) as i32;
    let image_url = media.get("coverImage").and_then(|c| c.get("large")).and_then(|l| l.as_str());
    let synopsis = media.get("description").and_then(|d| d.as_str());
    let anime_type = media.get("type").and_then(|t| t.as_str());
    let anime_status = media.get("status").and_then(|s| s.as_str());
    let season = media.get("season").and_then(|s| s.as_str());
    let season_year = media.get("seasonYear").and_then(|y| y.as_i64()).map(|y| y as i32);
    let now = unix_now_inner()?;

    state
        .storage
        .upsert_anime_full(
            anime_id,
            &titles_json,
            ep_count,
            image_url,
            synopsis,
            anime_type,
            anime_status,
            now,
        )
        .await?;
    state
        .storage
        .set_anime_season(anime_id, season, season_year)
        .await?;
    Ok(())
}

pub async fn fetch_anime_detail_inner(
    anime_id: i64,
    state: &EngineState,
) -> anyhow::Result<AnimeDetailResponse> {
    let mut detail = match state.storage.anime_detail(anime_id).await {
        Ok(d) => d,
        Err(_) => {
            // Auto-import from AniList if not found locally (best-effort).
            let _ = import_anime_from_anilist(state, anime_id).await;
            // Retry — should succeed after insert above
            state.storage.anime_detail(anime_id).await?
        }
    };

    // Backfill a missing synopsis from AniList. Shows imported via the fast
    // bulk-match path have no description (the search endpoint doesn't return
    // one); fetch it on first detail view. Best-effort — stays empty if offline.
    if detail.synopsis.as_deref().unwrap_or("").trim().is_empty()
        && import_anime_from_anilist(state, anime_id).await.is_ok()
    {
        if let Ok(d) = state.storage.anime_detail(anime_id).await {
            detail = d;
        }
    }

    let recent_history = state
        .storage
        .list_recent_watch_history_for_anime(anime_id, 10)
        .await?;
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct NextAiring {
    pub episode: i32,
    pub airing_at: i64,
    pub time_until_airing: i64,
}

pub async fn get_next_airing_inner(
    state: &EngineState,
    anime_id: i64,
) -> anyhow::Result<Option<NextAiring>> {
    let token = match auth::load_token(&state.storage).await? {
        Some(t) => t,
        None => return Ok(None),
    };
    let client = AniListClient::new(token);
    let query_str = format!(
        "query {{ Media(id: {anime_id}, type: ANIME) {{ nextAiringEpisode {{ airingAt timeUntilAiring episode }} }} }}"
    );
    let raw: serde_json::Value = client.query(&query_str, serde_json::json!({})).await?;
    let n = raw
        .get("data")
        .and_then(|d| d.get("Media"))
        .and_then(|m| m.get("nextAiringEpisode"))
        .filter(|v| !v.is_null());
    Ok(n.map(|n| NextAiring {
        episode: n.get("episode").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        airing_at: n.get("airingAt").and_then(|v| v.as_i64()).unwrap_or(0),
        time_until_airing: n.get("timeUntilAiring").and_then(|v| v.as_i64()).unwrap_or(0),
    }))
}

/// Fetch the next airing episode for an anime from AniList (for the detail page
/// air-date + countdown). Returns None when not connected or not currently airing.
#[tauri::command]
pub async fn get_next_airing(
    anime_id: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<Option<NextAiring>, String> {
    get_next_airing_inner(&state, anime_id)
        .await
        .map_err(command_error)
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
        .await?;
    // When progress advances to the episode cap (and the caller didn't set an
    // explicit status), auto-move the show to "completed".
    if status.is_none() && watched_episodes.is_some() {
        state.storage.auto_complete_if_capped(anime_id).await?;
    }
    // Push the change (status + progress) back to AniList (best-effort, queued).
    crate::engine::sync_worker::enqueue_anilist_sync(state, anime_id).await;
    Ok(())
}

pub async fn delete_anime_inner(anime_id: i64, state: &EngineState) -> anyhow::Result<()> {
    state.storage.delete_anime(anime_id).await
}

/// Completely remove an anime from the library (list entry, history, mappings).
#[tauri::command]
pub async fn delete_anime(
    anime_id: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    delete_anime_inner(anime_id, &state)
        .await
        .map_err(command_error)
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

pub async fn rescan_anime_files_inner(
    state: &EngineState,
    anime_id: i64,
) -> anyhow::Result<library_scanner::LibraryScanReport> {
    library_scanner::rescan_anime_dirs(&state.storage, anime_id).await
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

// ── AniList Search ─────────────────────────────────────────────────────────────

pub async fn search_anime_inner(state: &EngineState, query: String) -> anyhow::Result<Vec<SeasonAnimeEntry>> {
    let token = crate::engine::anilist::auth::load_token(&state.storage).await?.ok_or_else(|| anyhow::anyhow!("not connected"))?;
    let client = AniListClient::new(token);
    let results = client.search_anime(&query).await?;
    Ok(results.into_iter().map(|r| {
        let title = r.title.as_ref()
            .and_then(|t| t.romaji.clone())
            .or_else(|| r.title.as_ref().and_then(|t| t.english.clone()))
            .unwrap_or_else(|| format!("#{}", r.id));
        SeasonAnimeEntry {
            id: r.id,
            title,
            image_url: r.cover_image.and_then(|c| c.large),
            episodes: r.episodes,
            status: r.status,
            format: r.format,
            average_score: r.average_score,
            popularity: None,
        }
    }).collect())
}

#[tauri::command]
pub async fn search_anime(query: String, state: tauri::State<'_, EngineState>) -> Result<Vec<SeasonAnimeEntry>, String> {
    search_anime_inner(&state, query).await.map_err(command_error)
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

// ── Sync queue helpers ───────────────────────────────────────────────────────

pub async fn queue_anilist_sync_inner(state: &EngineState, anime_id: i64, episode: i32) -> anyhow::Result<()> {
    let payload = serde_json::json!({"episode": episode, "status": "plan_to_watch"}).to_string();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    state.storage.queue_sync(anime_id, "anilist", "update", &payload, now).await?;
    Ok(())
}

#[tauri::command]
pub async fn queue_anilist_sync(anime_id: i64, episode: i32, state: tauri::State<'_, EngineState>) -> Result<(), String> {
    queue_anilist_sync_inner(&state, anime_id, episode).await.map_err(command_error)
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
    // Universe of the airing calendar: the shows you're watching or plan to watch.
    let calendar_ids: Vec<i64> = state.storage.calendar_anime_ids().await.unwrap_or_default();
    let calendar_id_set: std::collections::HashSet<i64> = calendar_ids.iter().copied().collect();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    // Cover the previous ~month through ~2 months ahead so the month grid the
    // user pages through (prev / current / next) is populated.
    let window_start = now - 31 * 86_400;
    let window_end = now + 60 * 86_400;

    // One entry per airing episode (a show can appear on many days). AniList is
    // primary; Sonarr only fills shows AniList returned nothing for.
    let mut result: Vec<CalendarEntry> = Vec::new();
    let mut anilist_covered: std::collections::HashSet<i64> = std::collections::HashSet::new();
    // Guards against the same episode appearing twice across sources.
    let mut seen: std::collections::HashSet<(i64, i32)> = std::collections::HashSet::new();

    // ── PRIMARY: AniList full airing schedule for followed shows ──────────────
    if !calendar_ids.is_empty() {
        if let Some(token) = crate::engine::anilist::auth::load_token(&state.storage).await? {
            let client = crate::engine::anilist::client::AniListClient::new(token);
            match client
                .fetch_airing_schedule_range(&calendar_ids, window_start, window_end)
                .await
            {
                Ok(items) => {
                    for it in items {
                        if !calendar_id_set.contains(&it.media_id) {
                            continue;
                        }
                        anilist_covered.insert(it.media_id);
                        if !seen.insert((it.media_id, it.episode)) {
                            continue;
                        }
                        result.push(CalendarEntry {
                            anime_id: it.media_id,
                            title: it.title,
                            image_url: it.image_url,
                            episode_count: it.episode_count,
                            progress: None,
                            next_episode: Some(it.episode),
                            airing_at: Some(it.airing_at),
                            time_until_airing: Some(it.time_until_airing),
                        });
                    }
                    tracing::info!(
                        "Calendar AniList primary: {} episodes across {} shows",
                        result.len(),
                        anilist_covered.len()
                    );
                }
                Err(e) => tracing::warn!("AniList calendar failed: {}, relying on Sonarr", e),
            }
        }
    }

    // ── FALLBACK: Sonarr fills followed shows AniList had no airing for ───────
    let url_raw = state.storage.get_setting("sonarr.url").await.ok().flatten();
    let api_key_enc = state.storage.get_setting("sonarr.api_key").await.ok().flatten();
    if let (Some(url_raw), Some(api_key_enc)) = (url_raw, api_key_enc) {
        if let (Ok(url), Ok(api_key)) = (
            serde_json::from_str::<String>(&url_raw),
            crate::engine::secrets::unprotect_secret(&api_key_enc),
        ) {
            let client = crate::engine::sonarr::client::SonarrClient::new(url, api_key);
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            let future = (chrono::Local::now() + chrono::Duration::days(60))
                .format("%Y-%m-%d")
                .to_string();

            match client.fetch_calendar(&today, &future).await {
                Ok(entries) => {
                    let mut filled = 0i64;
                    for e in entries {
                        let anime_id = match e.series_id {
                            Some(sid) => state
                                .storage
                                .sonarr_mapping_by_sonarr_id(sid)
                                .await
                                .ok()
                                .flatten()
                                .and_then(|m| m.anime_id)
                                .unwrap_or(0),
                            None => 0,
                        };

                        // Only followed shows that AniList didn't already cover.
                        if anime_id <= 0
                            || !calendar_id_set.contains(&anime_id)
                            || anilist_covered.contains(&anime_id)
                        {
                            continue;
                        }
                        let episode = e.episode_number.unwrap_or(0);
                        if !seen.insert((anime_id, episode)) {
                            continue;
                        }

                        let air_ts = e
                            .air_date_utc
                            .as_deref()
                            .and_then(crate::engine::sonarr::import::parse_sonarr_date);
                        result.push(CalendarEntry {
                            anime_id,
                            title: e
                                .series
                                .as_ref()
                                .and_then(|s| s.title.as_deref())
                                .unwrap_or("Unknown")
                                .to_string(),
                            image_url: None,
                            episode_count: None,
                            progress: None,
                            next_episode: e.episode_number,
                            airing_at: air_ts,
                            time_until_airing: air_ts.map(|air| (air - now).max(0)),
                        });
                        filled += 1;
                    }
                    tracing::info!("Calendar Sonarr fallback: {} episodes filled", filled);
                }
                Err(e) => tracing::warn!("Sonarr calendar failed: {}", e),
            }
        }
    }

    result.sort_by_key(|e| e.airing_at.unwrap_or(i64::MAX));

    // ── LAST RESORT: no airing data from either source → local watching list ──
    if result.is_empty() {
        let watching = state.storage.continue_watching(50).await?;
        result = watching
            .into_iter()
            .map(|w| CalendarEntry {
                anime_id: w.anime_id,
                title: w.anime_title,
                image_url: w.image_url,
                episode_count: w.episode_count,
                progress: Some(w.watched_episodes),
                next_episode: None,
                airing_at: None,
                time_until_airing: None,
            })
            .collect();
        tracing::info!("Calendar fallback: {} watching entries from local DB", result.len());
    }

    Ok(result)
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
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
) -> Result<String, String> {
    restore_database_inner(&state, backup_path).await?;
    // The pool is closed after a restore; relaunch so the app comes back up on
    // the restored database instead of erroring until a manual restart.
    app.restart()
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

/// Persistently ignore (tombstone) or un-ignore a known file so the library
/// scanner and rematch never re-index/re-match it.
#[tauri::command]
pub async fn set_known_file_ignored(
    file_path: String,
    ignored: bool,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    state
        .storage
        .set_file_index_ignored(&file_path, ignored)
        .await
        .map_err(command_error)
}

/// Delete a file from the index. Note: a file still present on disk will be
/// re-indexed on the next library scan — use `set_known_file_ignored` to
/// suppress a file permanently.
#[tauri::command]
pub async fn delete_known_file(
    file_path: String,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    state
        .storage
        .delete_file_index(&file_path)
        .await
        .map_err(command_error)
}

/// Manually map a known file to an anime + episode at full confidence, then
/// re-match the unmatched files in the same folder so siblings inherit the
/// mapping immediately. Unlike `confirm_identification`, this is a management
/// write and does not emit a playback identification event.
pub async fn set_known_file_mapping_inner(
    state: &EngineState,
    file_path: &str,
    anime_id: i64,
    episode: i32,
) -> anyhow::Result<()> {
    let now = unix_now_inner()?;
    state
        .storage
        .upsert_file_index(file_path, Some(anime_id), episode, 100, now)
        .await?;
    let dirs = crate::engine::library_scanner::parent_dirs(&[file_path.to_string()]);
    crate::engine::library_scanner::rematch_unmatched_in_dirs(&state.storage, &dirs).await?;
    Ok(())
}

#[tauri::command]
pub async fn set_known_file_mapping(
    file_path: String,
    anime_id: i64,
    episode: i32,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    set_known_file_mapping_inner(&state, &file_path, anime_id, episode)
        .await
        .map_err(command_error)
}

/// One entry of a bulk mapping request.
#[derive(serde::Deserialize)]
pub struct FileMappingInput {
    pub file_path: String,
    pub anime_id: i64,
    pub episode: i32,
}

/// Bulk manual mapping — map many files to anime + episode at once (one
/// transaction), then sweep unmatched siblings in the affected folders.
pub async fn set_known_file_mappings_inner(
    state: &EngineState,
    mappings: Vec<FileMappingInput>,
) -> anyhow::Result<usize> {
    let now = unix_now_inner()?;
    let tuples: Vec<(String, i64, i32)> = mappings
        .into_iter()
        .map(|m| (m.file_path, m.anime_id, m.episode))
        .collect();
    let count = tuples.len();
    state.storage.upsert_file_mappings(&tuples, now).await?;
    let paths: Vec<String> = tuples.into_iter().map(|(p, _, _)| p).collect();
    let dirs = crate::engine::library_scanner::parent_dirs(&paths);
    crate::engine::library_scanner::rematch_unmatched_in_dirs(&state.storage, &dirs).await?;
    Ok(count)
}

#[tauri::command]
pub async fn set_known_file_mappings(
    mappings: Vec<FileMappingInput>,
    state: tauri::State<'_, EngineState>,
) -> Result<usize, String> {
    set_known_file_mappings_inner(&state, mappings)
        .await
        .map_err(command_error)
}

/// Bulk ignore / un-ignore.
#[tauri::command]
pub async fn set_known_files_ignored(
    file_paths: Vec<String>,
    ignored: bool,
    state: tauri::State<'_, EngineState>,
) -> Result<usize, String> {
    let count = file_paths.len();
    state
        .storage
        .set_file_indexes_ignored(&file_paths, ignored)
        .await
        .map_err(command_error)?;
    Ok(count)
}

/// Bulk delete of index rows.
#[tauri::command]
pub async fn delete_known_files(
    file_paths: Vec<String>,
    state: tauri::State<'_, EngineState>,
) -> Result<usize, String> {
    let count = file_paths.len();
    state
        .storage
        .delete_file_indexes(&file_paths)
        .await
        .map_err(command_error)?;
    Ok(count)
}

/// Unmap files from their anime (returns them to the Unmapped pool). Used by the
/// detail page to remove wrongly-mapped episode files.
#[tauri::command]
pub async fn unmap_known_files(
    file_paths: Vec<String>,
    state: tauri::State<'_, EngineState>,
) -> Result<usize, String> {
    let count = file_paths.len();
    state
        .storage
        .unmap_file_indexes(&file_paths)
        .await
        .map_err(command_error)?;
    Ok(count)
}

/// Show a native folder-picker dialog; returns the chosen path (or None if cancelled).
#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |res| {
        let _ = tx.send(res);
    });
    let picked = rx.await.map_err(|e| e.to_string())?;
    Ok(picked
        .and_then(|fp| fp.into_path().ok())
        .map(|p| p.to_string_lossy().to_string()))
}

/// Map every video file inside `folder` (recursively) to `anime_id` at full
/// confidence, with the episode number parsed from each filename. Used by the
/// detail page's "Map folder" so a series/season folder tracks to that anime.
pub async fn map_folder_to_anime_inner(
    folder: &str,
    anime_id: i64,
    state: &EngineState,
) -> anyhow::Result<usize> {
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    let mut errs: Vec<String> = Vec::new();
    library_scanner::find_video_files(std::path::Path::new(folder), &mut files, &mut errs);

    let mappings: Vec<(String, i64, i32)> = files
        .iter()
        .map(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            let episode = crate::engine::parser::parse_filename(name, None)
                .map(|pf| pf.episode_number)
                .filter(|e| *e > 0)
                .unwrap_or(0);
            (p.to_string_lossy().to_string(), anime_id, episode)
        })
        .collect();

    let now = unix_now_inner()?;
    state.storage.upsert_file_mappings(&mappings, now).await?;
    Ok(mappings.len())
}

#[tauri::command]
pub async fn map_folder_to_anime(
    folder: String,
    anime_id: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<usize, String> {
    map_folder_to_anime_inner(&folder, anime_id, &state)
        .await
        .map_err(command_error)
}

pub async fn import_anilist_anime_inner(state: &EngineState, anime_id: i64) -> anyhow::Result<()> {
    import_anime_from_anilist(state, anime_id).await?;
    // Add to the library as "watching" — but only if it isn't already tracked,
    // so an existing status / progress is never downgraded.
    if state.storage.get_list_entry(anime_id).await?.is_none() {
        let now = unix_now_inner()?;
        state
            .storage
            .upsert_list_entry_progress(anime_id, "watching", 0, now)
            .await?;
    }
    Ok(())
}

/// Import an anime from AniList by id (used when the file manager's "search
/// AniList" flow maps a file to a show not yet in the local library) and add it
/// to the library as "watching".
#[tauri::command]
pub async fn import_anilist_anime(
    anime_id: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    import_anilist_anime_inner(&state, anime_id)
        .await
        .map_err(command_error)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeepMatchReport {
    pub groups_total: usize,
    pub groups_matched: usize,
    pub files_mapped: usize,
    pub unmatched: Vec<String>,
}

/// Does a token look like a `SxxExx` season/episode marker (e.g. `S01E02`,
/// `S01E01-02-03`)? Used to split a series name off a Sonarr-style filename.
fn is_season_episode_token(tok: &str) -> bool {
    let b = tok.trim().as_bytes();
    let mut i = 0;
    if i >= b.len() || !(b[i] == b'S' || b[i] == b's') {
        return false;
    }
    i += 1;
    let s_digits = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == s_digits {
        return false;
    }
    if i >= b.len() || !(b[i] == b'E' || b[i] == b'e') {
        return false;
    }
    i += 1;
    let e_digits = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    i != e_digits
}

/// Derive a series key from a file path: the filename portion before the
/// `SxxExx` marker (e.g. "Cyberpunk - Edgerunners"), falling back to the parent
/// folder name, then the bare filename.
fn series_key_from_path(path: &str) -> String {
    let stem = std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path);
    let parts: Vec<&str> = stem.split(" - ").collect();
    let mut series_parts: Vec<&str> = Vec::new();
    for p in &parts {
        if is_season_episode_token(p) {
            break;
        }
        series_parts.push(p);
    }
    let key = series_parts.join(" - ").trim().to_string();
    if !key.is_empty() && key != stem {
        return key;
    }
    if let Some(parent) = std::path::Path::new(path)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
    {
        if !parent.is_empty() {
            return parent.to_string();
        }
    }
    stem.to_string()
}

fn titles_json_from_search(r: &crate::engine::anilist::client::SearchAnimeResult) -> String {
    let t = r.title.as_ref();
    serde_json::json!({
        "romaji": t.and_then(|x| x.romaji.clone()).unwrap_or_default(),
        "english": t.and_then(|x| x.english.clone()),
        "japanese": t.and_then(|x| x.native.clone()),
        "synonyms": [],
    })
    .to_string()
}

/// Upsert an anime into the library directly from a search result — no extra
/// `Media(id)` round-trip — and add a "watching" list entry if it isn't already
/// tracked. Used by bulk deep-match so a matched series costs only the (batched)
/// search request, not a second per-series AniList call.
async fn import_search_result_as_watching(
    state: &EngineState,
    r: &crate::engine::anilist::client::SearchAnimeResult,
    now: i64,
) -> anyhow::Result<()> {
    let titles_json = titles_json_from_search(r);
    let image = r.cover_image.as_ref().and_then(|c| c.large.as_deref());
    state
        .storage
        .upsert_anime_full(
            r.id,
            &titles_json,
            r.episodes.unwrap_or(0),
            image,
            None,
            Some("ANIME"),
            r.status.as_deref(),
            now,
        )
        .await?;
    if state.storage.get_list_entry(r.id).await?.is_none() {
        state
            .storage
            .upsert_list_entry_progress(r.id, "watching", 0, now)
            .await?;
    }
    Ok(())
}

async fn search_multi_with_retry(
    client: &AniListClient,
    chunk: &[String],
) -> anyhow::Result<Vec<Vec<crate::engine::anilist::client::SearchAnimeResult>>> {
    let mut attempt = 0u8;
    loop {
        match client.search_anime_multi(chunk).await {
            Ok(v) => return Ok(v),
            Err(e) => {
                if e.to_string().contains("429") && attempt < 3 {
                    attempt += 1;
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
                return Err(e);
            }
        }
    }
}

pub async fn deep_match_via_anilist_inner(state: &EngineState) -> anyhow::Result<DeepMatchReport> {
    let token = auth::load_token(&state.storage)
        .await?
        .ok_or_else(|| anyhow::anyhow!("not connected to AniList"))?;
    let client = AniListClient::new(token);
    let now = unix_now_inner()?;

    // Group unmapped, non-ignored files by series.
    let files = state.storage.list_file_index(100_000, 0).await?;
    let mut groups: std::collections::HashMap<String, Vec<(String, i32)>> =
        std::collections::HashMap::new();
    for f in &files {
        if f.ignored || f.anime_id.is_some() {
            continue;
        }
        let episode = match f.episode {
            Some(e) if e > 0 => e,
            _ => {
                let name = std::path::Path::new(&f.file_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&f.file_path);
                crate::engine::parser::parse_filename(name, None)
                    .map(|p| p.episode_number)
                    .filter(|e| *e > 0)
                    .unwrap_or(1)
            }
        };
        let key = series_key_from_path(&f.file_path);
        groups.entry(key).or_default().push((f.file_path.clone(), episode));
    }

    let groups_total = groups.len();
    let mut groups_matched = 0usize;
    let mut files_mapped = 0usize;
    let mut unmatched: Vec<String> = Vec::new();

    const BATCH: usize = 8;
    const AUTO_THRESHOLD: u8 = 80;

    let keys: Vec<String> = groups.keys().cloned().collect();
    let total_chunks = keys.len().div_ceil(BATCH);
    for (chunk_i, chunk) in keys.chunks(BATCH).enumerate() {
        let results = search_multi_with_retry(&client, chunk).await?;
        for (ki, key) in chunk.iter().enumerate() {
            let mut best: Option<&crate::engine::anilist::client::SearchAnimeResult> = None;
            let mut best_score: u8 = 0;
            if let Some(candidates) = results.get(ki) {
                for c in candidates {
                    let score =
                        crate::engine::matcher::score_titles_json(key, &titles_json_from_search(c));
                    if score > best_score {
                        best_score = score;
                        best = Some(c);
                    }
                }
            }

            match (best_score >= AUTO_THRESHOLD, best) {
                (true, Some(r)) => {
                    // Import straight from the search result — no extra Media(id)
                    // AniList call — then map the whole series locally.
                    import_search_result_as_watching(state, r, now).await?;
                    if let Some(group_files) = groups.get(key) {
                        let mappings: Vec<(String, i64, i32)> = group_files
                            .iter()
                            .map(|(p, ep)| (p.clone(), r.id, *ep))
                            .collect();
                        state.storage.upsert_file_mappings(&mappings, now).await?;
                        groups_matched += 1;
                        files_mapped += group_files.len();
                    }
                }
                _ => unmatched.push(key.clone()),
            }
        }

        // Throttle between batches to stay under AniList's ~30 req/min limit
        // (~24/min here). `query()` additionally honors 429 Retry-After as a
        // backstop if we still get limited.
        if chunk_i + 1 < total_chunks {
            tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
        }
    }

    unmatched.sort();
    Ok(DeepMatchReport {
        groups_total,
        groups_matched,
        files_mapped,
        unmatched,
    })
}

/// Bulk auto-match every unmapped file against AniList search: group by series,
/// import strong matches into the library ("watching") and map all their files.
#[tauri::command]
pub async fn deep_match_via_anilist(
    state: tauri::State<'_, EngineState>,
) -> Result<DeepMatchReport, String> {
    deep_match_via_anilist_inner(&state)
        .await
        .map_err(command_error)
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
pub async fn rescan_anime_files(
    anime_id: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<library_scanner::LibraryScanReport, String> {
    rescan_anime_files_inner(&state, anime_id)
        .await
        .map_err(command_error)
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

#[tauri::command]
pub async fn open_containing_folder(path: String) -> Result<(), String> {
    library_scanner::open_containing_folder(&path).map_err(command_error)
}

// ── Sonarr command wrappers ─────────────────────────────────────────────────

#[tauri::command]
pub async fn list_sonarr_series(
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<crate::engine::storage::SonarrSeriesListRow>, String> {
    state.storage.list_sonarr_series().await.map_err(command_error)
}

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
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
) -> Result<SessionState, String> {
    let session = toggle_pause_tracking_inner(&state).await.map_err(command_error)?;
    // Keep the tray menu's pause label in sync when toggled from the UI.
    crate::update_tray_pause_label(&app, session.paused);
    Ok(session)
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

#[tauri::command]
pub async fn get_start_in_tray(
    state: tauri::State<'_, EngineState>,
) -> Result<bool, String> {
    get_start_in_tray_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn set_start_in_tray(
    enabled: bool,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    set_start_in_tray_inner(enabled, &state)
        .await
        .map_err(command_error)
}
