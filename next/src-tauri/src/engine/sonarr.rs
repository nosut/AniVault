use sqlx::Row;

/// Sonarr integration: config storage (URL plain, API key via DPAPI).
/// Connection test hits Sonarr `/api/v3/system/status`.

const SETTING_SONARR_URL: &str = "sonarr_url";
const SETTING_SONARR_KEY: &str = "sonarr_api_key";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SonarrConfig {
    pub url: String,
    pub api_key: String,
}

pub async fn get_sonarr_config(
    storage: &crate::engine::storage::Storage,
) -> anyhow::Result<SonarrConfig> {
    let url = get_setting(storage, SETTING_SONARR_URL).await.unwrap_or_default();

    let key_cipher = get_setting(storage, SETTING_SONARR_KEY).await.unwrap_or_default();
    let api_key = if key_cipher.is_empty() {
        String::new()
    } else {
        crate::engine::secrets::unprotect_secret(&key_cipher).unwrap_or_default()
    };

    Ok(SonarrConfig { url, api_key })
}

pub async fn set_sonarr_config(
    storage: &crate::engine::storage::Storage,
    config: &SonarrConfig,
) -> anyhow::Result<()> {
    let url = config.url.trim().to_string();
    set_setting(storage, SETTING_SONARR_URL, &url).await?;

    if config.api_key.is_empty() {
        set_setting(storage, SETTING_SONARR_KEY, "").await?;
    } else {
        let encrypted = crate::engine::secrets::protect_secret(&config.api_key)?;
        set_setting(storage, SETTING_SONARR_KEY, &encrypted).await?;
    }

    Ok(())
}

pub async fn test_sonarr_connection(url: &str, api_key: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{url}/api/v3/system/status"))
        .header("X-Api-Key", api_key)
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Sonarr responded with HTTP {}", resp.status());
    }

    Ok(())
}

async fn get_setting(storage: &crate::engine::storage::Storage, key: &str) -> Option<String> {
    let row = sqlx::query("SELECT value_json FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(storage.pool())
        .await
        .ok()??;
    Some(row.get::<String, _>(0))
}

async fn set_setting(
    storage: &crate::engine::storage::Storage,
    key: &str,
    value: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, unixepoch())
         ON CONFLICT(key) DO UPDATE SET value_json = ?2, updated_at = unixepoch()",
    )
    .bind(key)
    .bind(value)
    .execute(storage.pool())
    .await?;
    Ok(())
}
