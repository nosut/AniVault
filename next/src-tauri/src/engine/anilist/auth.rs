use std::time::SystemTime;

use crate::engine::secrets::{protect_secret, unprotect_secret};
use crate::engine::storage::Storage;

const TOKEN_KEY: &str = "anilist_access_token";
const CLIENT_ID_KEY: &str = "anilist_client_id";
const CLIENT_SECRET_KEY: &str = "anilist_client_secret";

/// Set when AniList rejected the stored token (expired or revoked).
const TOKEN_INVALID_KEY: &str = "anilist.token_invalid";

/// Load the stored AniList access token, if any.
///
/// Reads the DPAPI-encrypted token from the settings table and decrypts it.
/// Returns `Ok(None)` when no token has been stored, or when AniList has
/// rejected it — callers then stop hammering the API until the user reconnects.
pub async fn load_token(storage: &Storage) -> anyhow::Result<Option<String>> {
    if is_token_invalid(storage).await? {
        return Ok(None);
    }
    let ciphertext = storage.get_setting(TOKEN_KEY).await?;
    match ciphertext {
        Some(ct) => {
            let plaintext = unprotect_secret(&ct)?;
            Ok(Some(plaintext))
        }
        None => Ok(None),
    }
}

/// Encrypt and store an AniList access token.
pub async fn store_token(storage: &Storage, token: &str) -> anyhow::Result<()> {
    let ciphertext = protect_secret(token)?;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs() as i64;
    storage.set_setting(TOKEN_KEY, &ciphertext, now).await?;
    storage.delete_setting(TOKEN_INVALID_KEY).await?;
    Ok(())
}

/// Record that AniList rejected the stored token.
pub async fn mark_token_invalid(storage: &Storage) -> anyhow::Result<()> {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs() as i64;
    storage.set_setting(TOKEN_INVALID_KEY, "1", now).await
}

pub async fn is_token_invalid(storage: &Storage) -> anyhow::Result<bool> {
    Ok(storage.get_setting(TOKEN_INVALID_KEY).await?.is_some())
}

/// Whether a token is stored at all, valid or not. Local changes keep being
/// queued while the token is expired so they reach AniList after a reconnect.
pub async fn token_stored(storage: &Storage) -> anyhow::Result<bool> {
    Ok(storage.get_setting(TOKEN_KEY).await?.is_some())
}

/// Delete the stored AniList access token.
pub async fn delete_token(storage: &Storage) -> anyhow::Result<()> {
    storage.delete_setting(TOKEN_KEY).await?;
    storage.delete_setting(TOKEN_INVALID_KEY).await?;
    Ok(())
}

/// Check whether a valid access token is currently stored.
pub async fn is_connected(storage: &Storage) -> anyhow::Result<bool> {
    load_token(storage).await.map(|opt| opt.is_some())
}

pub async fn store_client_credentials(storage: &Storage, client_id: &str, client_secret: &str) -> anyhow::Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;
    storage.set_setting(CLIENT_ID_KEY, client_id, now).await?;
    let encrypted_secret = protect_secret(client_secret)?;
    storage.set_setting(CLIENT_SECRET_KEY, &encrypted_secret, now).await?;
    Ok(())
}

pub async fn load_client_credentials(storage: &Storage) -> anyhow::Result<Option<(String, String)>> {
    let client_id = storage.get_setting(CLIENT_ID_KEY).await?;
    let encrypted_secret = storage.get_setting(CLIENT_SECRET_KEY).await?;
    match (client_id, encrypted_secret) {
        (Some(id), Some(secret)) => {
            let secret = unprotect_secret(&secret)?;
            Ok(Some((id, secret)))
        }
        _ => Ok(None),
    }
}
