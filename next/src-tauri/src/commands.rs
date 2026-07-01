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
