use std::time::SystemTime;

use crate::engine::secrets::{protect_secret, unprotect_secret};
use crate::engine::storage::Storage;

const TOKEN_KEY: &str = "anilist_access_token";
const CLIENT_ID_KEY: &str = "anilist_client_id";
const CLIENT_SECRET_KEY: &str = "anilist_client_secret";

/// Load the stored AniList access token, if any.
///
/// Reads the DPAPI-encrypted token from the settings table and decrypts it.
/// Returns `Ok(None)` when no token has been stored.
pub async fn load_token(storage: &Storage) -> anyhow::Result<Option<String>> {
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
    Ok(())
}

/// Delete the stored AniList access token.
pub async fn delete_token(storage: &Storage) -> anyhow::Result<()> {
    storage.delete_setting(TOKEN_KEY).await?;
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
