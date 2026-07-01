use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::engine::migration::MigrationReport;
use crate::engine::models::{OAuthStatus, TrackingStatus};

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
    current_anime_id: Arc<Mutex<Option<i64>>>,
    current_episode: Arc<Mutex<Option<i32>>>,
}

impl TrackingRuntime {
    pub fn mark_running(&self) {
        self.is_running.store(true, Ordering::Relaxed);
    }

    pub fn set_current_anime(&self, current_anime: Option<String>) {
        *self.current_anime.lock().expect("tracking runtime poisoned") = current_anime;
    }

    pub fn set_tracking_info(&self, anime: Option<String>, anime_id: Option<i64>, episode: Option<i32>) {
        *self.current_anime.lock().expect("tracking runtime poisoned") = anime;
        *self.current_anime_id.lock().expect("tracking runtime poisoned") = anime_id;
        *self.current_episode.lock().expect("tracking runtime poisoned") = episode;
    }

    pub fn status(&self) -> TrackingStatus {
        TrackingStatus {
            is_running: self.is_running.load(Ordering::Relaxed),
            current_anime: self.current_anime.lock().expect("tracking runtime poisoned").clone(),
            current_anime_id: *self.current_anime_id.lock().expect("tracking runtime poisoned"),
            current_episode: *self.current_episode.lock().expect("tracking runtime poisoned"),
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

// ── OAuth ──

#[derive(Debug, Clone, Default)]
pub struct OAuthRuntime {
    callback_code: Arc<Mutex<Option<String>>>,
}

impl OAuthRuntime {
    pub fn set_callback_code(&self, code: String) {
        *self.callback_code.lock().unwrap() = Some(code);
    }

    pub fn take_callback_code(&self) -> Option<String> {
        self.callback_code.lock().unwrap().take()
    }
}

#[tauri::command]
pub async fn start_oauth(
    runtime: tauri::State<'_, OAuthRuntime>,
) -> Result<u16, String> {
    let db_url = local_db_url();
    let storage = crate::engine::storage::Storage::connect(&db_url)
        .await
        .map_err(|e| format!("db connect: {e}"))?;
    storage.migrate().await.map_err(|e| format!("migrate: {e}"))?;

    let port = find_free_port()?;
    let redirect_uri = format!("http://localhost:{port}/callback");
    let state = crate::engine::oauth::OAuthState::new(redirect_uri.clone());

    crate::engine::oauth::store_oauth_state(&storage, &state)
        .await
        .map_err(|e| format!("store state: {e}"))?;

    let callback_handle =
        crate::engine::oauth::start_redirect_listener(port)
            .await
            .map_err(|e| format!("listener: {e}"))?;

    // Poll for code in background, write to runtime when received
    let rt = (*runtime).clone();
    tokio::spawn(async move {
        loop {
            if let Some(code) = callback_handle.take_code() {
                rt.set_callback_code(code);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    });

    let auth_url = format!(
        "https://anilist.co/api/v2/oauth/authorize?client_id=18872&redirect_uri={}&response_type=code",
        urlencoding(&redirect_uri)
    );
    open_browser(&auth_url)?;

    Ok(port)
}

#[tauri::command]
pub async fn complete_oauth(
    runtime: tauri::State<'_, OAuthRuntime>,
) -> Result<OAuthStatus, String> {
    let Some(code) = runtime.take_callback_code() else {
        return Err("no authorization code received yet".into());
    };

    let db_url = local_db_url();
    let storage = crate::engine::storage::Storage::connect(&db_url)
        .await
        .map_err(|e| format!("db connect: {e}"))?;
    storage.migrate().await.map_err(|e| format!("migrate: {e}"))?;

    let state = crate::engine::oauth::load_oauth_state(&storage)
        .await
        .map_err(|e| format!("load state: {e}"))?
        .ok_or("no oauth state found")?;

    let token = crate::engine::oauth::finish_oauth(&storage, &code, &state)
        .await
        .map_err(|e| format!("token exchange: {e}"))?;

    Ok(OAuthStatus {
        authenticated: true,
        username: token.username,
    })
}

#[tauri::command]
pub async fn get_oauth_status(
    _runtime: tauri::State<'_, OAuthRuntime>,
) -> Result<OAuthStatus, String> {
    let db_url = local_db_url();
    let storage = crate::engine::storage::Storage::connect(&db_url)
        .await
        .map_err(|e| format!("db connect: {e}"))?;
    storage.migrate().await.map_err(|e| format!("migrate: {e}"))?;

    let token = crate::engine::oauth::load_oauth_token(&storage)
        .await
        .map_err(|e| format!("load token: {e}"))?;

    Ok(OAuthStatus {
        authenticated: token.is_some(),
        username: token.as_ref().and_then(|t| t.username.clone()),
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncStatus {
    pub pending: i64,
    pub failed: i64,
}

#[tauri::command]
pub async fn get_sync_status() -> Result<SyncStatus, String> {
    let db_url = local_db_url();
    let storage = crate::engine::storage::Storage::connect(&db_url)
        .await
        .map_err(|e| format!("db connect: {e}"))?;

    let pending = storage.pending_sync_count("anilist").await.map_err(|e| format!("count: {e}"))?;
    Ok(SyncStatus { pending, failed: 0 })
}

#[tauri::command]
pub async fn set_watched_episodes(anime_id: i64, episode: i32) -> Result<(), String> {
    let db_url = local_db_url();
    let storage = crate::engine::storage::Storage::connect(&db_url)
        .await
        .map_err(|e| format!("db connect: {e}"))?;
    storage.migrate().await.map_err(|e| format!("migrate: {e}"))?;
    storage.set_watched_episodes(anime_id, episode)
        .await
        .map_err(|e| format!("set episodes: {e}"))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PendingMatchResponse {
    pub anilist_id: i64,
    pub title_romaji: String,
    pub parsed_title: String,
    pub confidence: u8,
    pub episode_count: Option<i32>,
}

#[tauri::command]
pub async fn get_pending_matches() -> Result<Vec<PendingMatchResponse>, String> {
    let db_url = local_db_url();
    let storage = crate::engine::storage::Storage::connect(&db_url)
        .await
        .map_err(|e| format!("db connect: {e}"))?;
    storage.migrate().await.map_err(|e| format!("migrate: {e}"))?;

    let pending = crate::engine::pending::get_pending_matches(&storage)
        .await
        .map_err(|e| format!("get pending: {e}"))?;

    Ok(pending
        .into_iter()
        .map(|p| PendingMatchResponse {
            anilist_id: p.anilist_id,
            title_romaji: p.title_romaji,
            parsed_title: p.parsed_title,
            confidence: p.confidence,
            episode_count: p.episode_count,
        })
        .collect())
}

#[tauri::command]
pub async fn confirm_match(anilist_id: i64) -> Result<(), String> {
    let db_url = local_db_url();
    let storage = crate::engine::storage::Storage::connect(&db_url)
        .await
        .map_err(|e| format!("db connect: {e}"))?;
    storage.migrate().await.map_err(|e| format!("migrate: {e}"))?;

    crate::engine::pending::confirm_pending_match(&storage, anilist_id)
        .await
        .map_err(|e| format!("confirm: {e}"))
}

#[tauri::command]
pub async fn reject_match(anilist_id: i64) -> Result<(), String> {
    let db_url = local_db_url();
    let storage = crate::engine::storage::Storage::connect(&db_url)
        .await
        .map_err(|e| format!("db connect: {e}"))?;
    storage.migrate().await.map_err(|e| format!("migrate: {e}"))?;

    crate::engine::pending::reject_pending_match(&storage, anilist_id)
        .await
        .map_err(|e| format!("reject: {e}"))
}

#[tauri::command]
pub async fn get_library_anime() -> Result<Vec<crate::engine::storage::LibraryEntry>, String> {
    let db_url = local_db_url();
    let storage = crate::engine::storage::Storage::connect(&db_url)
        .await
        .map_err(|e| format!("db connect: {e}"))?;
    storage.migrate().await.map_err(|e| format!("migrate: {e}"))?;
    storage.get_library_anime()
        .await
        .map_err(|e| format!("get library: {e}"))
}

#[tauri::command]
pub async fn get_sonarr_config() -> Result<crate::engine::sonarr::SonarrConfig, String> {
    let db_url = local_db_url();
    let storage = crate::engine::storage::Storage::connect(&db_url)
        .await
        .map_err(|e| format!("db connect: {e}"))?;
    storage.migrate().await.map_err(|e| format!("migrate: {e}"))?;
    crate::engine::sonarr::get_sonarr_config(&storage)
        .await
        .map_err(|e| format!("get config: {e}"))
}

#[tauri::command]
pub async fn set_sonarr_config(url: String, api_key: String) -> Result<(), String> {
    let db_url = local_db_url();
    let storage = crate::engine::storage::Storage::connect(&db_url)
        .await
        .map_err(|e| format!("db connect: {e}"))?;
    storage.migrate().await.map_err(|e| format!("migrate: {e}"))?;
    crate::engine::sonarr::set_sonarr_config(
        &storage,
        &crate::engine::sonarr::SonarrConfig { url, api_key },
    )
    .await
    .map_err(|e| format!("set config: {e}"))
}

#[tauri::command]
pub async fn test_sonarr_connection(url: String, api_key: String) -> Result<String, String> {
    crate::engine::sonarr::test_sonarr_connection(&url, &api_key)
        .await
        .map(|_| "Connected to Sonarr".to_string())
        .map_err(|e| format!("connection failed: {e}"))
}

#[tauri::command]
pub async fn get_sonarr_mappings() -> Result<Vec<crate::engine::sonarr::SonarrMapping>, String> {
    let db_url = local_db_url();
    let storage = crate::engine::storage::Storage::connect(&db_url)
        .await
        .map_err(|e| format!("db connect: {e}"))?;
    storage.migrate().await.map_err(|e| format!("migrate: {e}"))?;
    crate::engine::sonarr::get_sonarr_mappings(&storage)
        .await
        .map_err(|e| format!("get mappings: {e}"))
}

#[tauri::command]
pub async fn map_sonarr_series(anime_id: i64, sonarr_series_id: i64, sonarr_title: String) -> Result<(), String> {
    let db_url = local_db_url();
    let storage = crate::engine::storage::Storage::connect(&db_url)
        .await
        .map_err(|e| format!("db connect: {e}"))?;
    storage.migrate().await.map_err(|e| format!("migrate: {e}"))?;
    crate::engine::sonarr::map_sonarr_series(&storage, anime_id, sonarr_series_id, &sonarr_title)
        .await
        .map_err(|e| format!("map: {e}"))
}

fn local_db_url() -> String {
    let app_data = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    let directory = app_data.join("AniVault");
    let _ = std::fs::create_dir_all(&directory);
    let normalized = directory.join("anivault.db").to_string_lossy().replace('\\', "/");
    format!("sqlite:///{}", normalized)
}

fn find_free_port() -> Result<u16, String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| format!("bind: {e}"))?;
    Ok(listener.local_addr().map_err(|e| format!("addr: {e}"))?.port())
}

fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b':' | b'/' => {
                out.push(byte as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    out
}

fn open_browser(url: &str) -> Result<(), String> {
    std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn()
        .map_err(|e| format!("open browser: {e}"))?;
    Ok(())
}
