use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::Rng;
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::sync::{Arc, Mutex};

const DEFAULT_CLIENT_ID: &str = "18872";
const SETTING_CLIENT_ID: &str = "anilist_client_id";
const SETTING_KEY_OAUTH_STATE: &str = "oauth_state";
const SETTING_KEY_OAUTH_TOKEN: &str = "oauth_token";

pub async fn get_client_id(storage: &crate::engine::storage::Storage) -> String {
    let row = sqlx::query("SELECT value_json FROM settings WHERE key = ?1")
        .bind(SETTING_CLIENT_ID)
        .fetch_optional(storage.pool())
        .await
        .ok()
        .flatten();
    row.map(|r| r.get::<String, _>(0))
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string())
}

pub async fn set_client_id(storage: &crate::engine::storage::Storage, client_id: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, unixepoch())
         ON CONFLICT(key) DO UPDATE SET value_json = ?2, updated_at = unixepoch()",
    )
    .bind(SETTING_CLIENT_ID)
    .bind(client_id)
    .execute(storage.pool())
    .await?;
    Ok(())
}

// ── PKCE ──

pub fn generate_code_verifier() -> String {
    let mut rng = rand::thread_rng();
    let len = rng.gen_range(43..=128);
    let chars: Vec<u8> = (b'A'..=b'Z')
        .chain(b'a'..=b'z')
        .chain(b'0'..=b'9')
        .chain([b'-', b'_', b'.', b'~'].into_iter())
        .collect();

    (0..len)
        .map(|_| chars[rng.gen_range(0..chars.len())] as char)
        .collect()
}

pub fn code_challenge_from_verifier(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest.as_slice())
}

// ── OAuth state ──

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthState {
    pub code_verifier: String,
    pub code_challenge: String,
    pub redirect_uri: String,
}

impl OAuthState {
    pub fn new(redirect_uri: String) -> Self {
        let code_verifier = generate_code_verifier();
        let code_challenge = code_challenge_from_verifier(&code_verifier);
        Self {
            code_verifier,
            code_challenge,
            redirect_uri,
        }
    }
}

pub async fn store_oauth_state(
    storage: &crate::engine::storage::Storage,
    state: &OAuthState,
) -> anyhow::Result<()> {
    let value = serde_json::to_string(state)?;
    sqlx::query(
        "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, unixepoch())
         ON CONFLICT(key) DO UPDATE SET value_json = ?2, updated_at = unixepoch()",
    )
    .bind(SETTING_KEY_OAUTH_STATE)
    .bind(&value)
    .execute(storage.pool())
    .await?;
    Ok(())
}

pub async fn load_oauth_state(
    storage: &crate::engine::storage::Storage,
) -> anyhow::Result<Option<OAuthState>> {
    let row = sqlx::query("SELECT value_json FROM settings WHERE key = ?1")
        .bind(SETTING_KEY_OAUTH_STATE)
        .fetch_optional(storage.pool())
        .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let value: String = row.get(0);
    Ok(Some(serde_json::from_str(&value)?))
}

// ── Token storage (DPAPI) ──

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub obtained_at: i64,
    pub username: Option<String>,
}

pub async fn store_oauth_token(
    storage: &crate::engine::storage::Storage,
    token: &OAuthToken,
) -> anyhow::Result<()> {
    let plain = serde_json::to_string(token)?;
    let encrypted = crate::engine::secrets::protect_secret(&plain)?;
    sqlx::query(
        "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, unixepoch())
         ON CONFLICT(key) DO UPDATE SET value_json = ?2, updated_at = unixepoch()",
    )
    .bind(SETTING_KEY_OAUTH_TOKEN)
    .bind(&encrypted)
    .execute(storage.pool())
    .await?;
    Ok(())
}

pub async fn load_oauth_token(
    storage: &crate::engine::storage::Storage,
) -> anyhow::Result<Option<OAuthToken>> {
    let row = sqlx::query("SELECT value_json FROM settings WHERE key = ?1")
        .bind(SETTING_KEY_OAUTH_TOKEN)
        .fetch_optional(storage.pool())
        .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let ciphertext: String = row.get(0);
    let plain = crate::engine::secrets::unprotect_secret(&ciphertext)?;
    Ok(Some(serde_json::from_str(&plain)?))
}

// ── OAuth flow ──

pub async fn finish_oauth(
    storage: &crate::engine::storage::Storage,
    code: &str,
    state: &OAuthState,
    client_id: &str,
) -> anyhow::Result<OAuthToken> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://anilist.co/api/v2/oauth/token")
        .json(&serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": client_id,
            "client_secret": "",
            "redirect_uri": state.redirect_uri,
            "code": code,
            "code_verifier": state.code_verifier,
        }))
        .send()
        .await?;

    let json: serde_json::Value = resp.json().await?;
    let access_token = json["access_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no access_token in response"))?
        .to_string();
    let token_type = json["token_type"]
        .as_str()
        .unwrap_or("Bearer")
        .to_string();
    let expires_in = json["expires_in"].as_i64().unwrap_or(0);

    let obtained_at = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // Fetch username
    let username = fetch_username(&access_token).await.ok();

    let token = OAuthToken {
        access_token,
        token_type,
        expires_in,
        obtained_at,
        username,
    };

    store_oauth_token(storage, &token).await?;
    Ok(token)
}

async fn fetch_username(access_token: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://graphql.anilist.co")
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&serde_json::json!({
            "query": "{ Viewer { name } }"
        }))
        .send()
        .await?;

    let json: serde_json::Value = resp.json().await?;
    json["data"]["Viewer"]["name"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("no viewer name in response"))
}

// ── Localhost redirect listener ──

#[derive(Debug, Clone, Default)]
pub struct OAuthCallback {
    pub code: Arc<Mutex<Option<String>>>,
}

impl OAuthCallback {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn take_code(&self) -> Option<String> {
        self.code.lock().unwrap().take()
    }
}

pub async fn start_redirect_listener(port: u16) -> anyhow::Result<OAuthCallback> {
    let callback = OAuthCallback::new();
    let cb = callback.clone();

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            let (reader, mut writer) = stream.split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();
            if reader.read_line(&mut line).await.is_ok() {
                if let Some(code) = extract_code_from_request(&line) {
                    *cb.code.lock().unwrap() = Some(code);
                }
            }
            let body = if cb.code.lock().unwrap().is_some() {
                "<html><body><h1>Authorized</h1><p>You can close this tab.</p></body></html>"
            } else {
                "<html><body><h1>Error</h1><p>No authorization code found.</p></body></html>"
            };
            let _ = writer
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .as_bytes(),
                )
                .await;
            let _ = writer.shutdown().await;
        }
    });

    Ok(callback)
}

fn extract_code_from_request(request_line: &str) -> Option<String> {
    let path = request_line.split_whitespace().nth(1)?;
    let query = path.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if kv.next()? == "code" {
            return kv.next().map(|v| url_decode(v).unwrap_or_else(|| v.to_string()));
        }
    }
    None
}

fn url_decode(s: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '%' => {
                let hi = chars.next()?.to_digit(16)?;
                let lo = chars.next()?.to_digit(16)?;
                out.push(char::from_u32((hi << 4) | lo)?);
            }
            '+' => out.push(' '),
            _ => out.push(c),
        }
    }
    Some(out)
}
