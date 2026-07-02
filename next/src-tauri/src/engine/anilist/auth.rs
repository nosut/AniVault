use std::time::SystemTime;

use crate::engine::secrets::{protect_secret, unprotect_secret};
use crate::engine::storage::Storage;

const TOKEN_KEY: &str = "anilist_access_token";

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
