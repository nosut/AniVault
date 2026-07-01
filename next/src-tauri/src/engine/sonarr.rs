use sqlx::Row;

/// Sonarr integration: config, connection test, series search, and anime mapping.

const SETTING_SONARR_URL: &str = "sonarr_url";
const SETTING_SONARR_KEY: &str = "sonarr_api_key";

// ── Config ──

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

// ── Series search ──

#[derive(Debug, Clone, serde::Serialize)]
pub struct SonarrSearchResult {
    pub id: i64,
    pub title: String,
    pub year: i32,
    pub tvdb_id: Option<i64>,
    pub title_slug: String,
    pub poster_url: Option<String>,
}

pub fn parse_sonarr_search(json: &str) -> anyhow::Result<Vec<SonarrSearchResult>> {
    let parsed: Vec<serde_json::Value> = serde_json::from_str(json)?;
    Ok(parsed
        .into_iter()
        .map(|m| SonarrSearchResult {
            id: m["id"].as_i64().unwrap_or(0),
            title: m["title"].as_str().unwrap_or("").into(),
            year: m["year"].as_i64().unwrap_or(0) as i32,
            tvdb_id: m["tvdbId"].as_i64(),
            title_slug: m["titleSlug"].as_str().unwrap_or("").into(),
            poster_url: m["images"]
                .as_array()
                .and_then(|imgs| {
                    imgs.iter()
                        .find(|img| img["coverType"].as_str() == Some("poster"))
                        .and_then(|img| img["remoteUrl"].as_str().map(String::from))
                }),
        })
        .collect())
}

// ── Mapping ──

#[derive(Debug, Clone, serde::Serialize)]
pub struct SonarrMapping {
    pub anime_id: i64,
    pub sonarr_series_id: i64,
    pub sonarr_title: String,
    pub monitored: bool,
}

pub async fn get_sonarr_mappings(
    storage: &crate::engine::storage::Storage,
) -> anyhow::Result<Vec<SonarrMapping>> {
    let rows = sqlx::query(
        "SELECT sm.anime_id, sm.sonarr_series_id, sm.sonarr_title, sm.monitored
         FROM sonarr_mapping sm
         ORDER BY sm.sonarr_title",
    )
    .fetch_all(storage.pool())
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| SonarrMapping {
            anime_id: r.get(0),
            sonarr_series_id: r.get(1),
            sonarr_title: r.get(2),
            monitored: r.get::<i64, _>(3) != 0,
        })
        .collect())
}

pub async fn map_sonarr_series(
    storage: &crate::engine::storage::Storage,
    anime_id: i64,
    sonarr_series_id: i64,
    sonarr_title: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO sonarr_mapping (anime_id, sonarr_series_id, sonarr_title, monitored, updated_at)
         VALUES (?1, ?2, ?3, 0, unixepoch())",
    )
    .bind(anime_id)
    .bind(sonarr_series_id)
    .bind(sonarr_title)
    .execute(storage.pool())
    .await?;
    Ok(())
}

pub async fn unmap_sonarr_series(
    storage: &crate::engine::storage::Storage,
    anime_id: i64,
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM sonarr_mapping WHERE anime_id = ?1")
        .bind(anime_id)
        .execute(storage.pool())
        .await?;
    Ok(())
}

// ── Helpers ──

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
