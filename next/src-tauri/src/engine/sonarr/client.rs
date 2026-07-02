use reqwest::header::{HeaderMap, HeaderValue};

#[derive(Debug, Clone)]
pub struct SonarrClient {
    pub url: String,
    pub api_key: String,
    http: reqwest::Client,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrSystemStatus {
    pub version: Option<String>,
    pub app_name: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrSeriesRaw {
    pub id: i64,
    pub title: String,
    pub season_count: Option<i32>,
    #[serde(default)]
    pub seasons: Vec<SonarrSeasonRaw>,
    pub monitored: bool,
    pub next_airing: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub images: Vec<SonarrImageRaw>,
    pub overview: Option<String>,
    pub network: Option<String>,
    pub status: Option<String>,
    pub added: Option<String>,
    #[serde(default)]
    pub statistics: Option<SonarrStatisticsRaw>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrSeasonRaw {
    #[serde(default)]
    pub statistics: Option<SonarrStatisticsRaw>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrStatisticsRaw {
    #[serde(default)]
    pub episode_count: i32,
    #[serde(default)]
    pub episode_file_count: i32,
    #[serde(default)]
    pub total_episode_count: i32,
    pub next_airing: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrImageRaw {
    pub cover_type: Option<String>,
    pub remote_url: Option<String>,
}

impl SonarrClient {
    pub fn new(url: String, api_key: String) -> Self {
        let url = url.trim_end_matches('/').to_string();
        Self {
            url,
            api_key,
            http: reqwest::Client::new(),
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Api-Key",
            HeaderValue::from_str(&self.api_key).expect("invalid API key for header"),
        );
        headers
    }

    pub async fn validate_connection(&self) -> anyhow::Result<SonarrSystemStatus> {
        let resp = self
            .http
            .get(format!("{}/api/v3/system/status", self.url))
            .headers(self.headers())
            .send()
            .await?;

        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            return Err(anyhow::anyhow!("Sonarr returned HTTP {}", status));
        }

        let body: SonarrSystemStatus = resp.json().await?;
        Ok(body)
    }

    pub async fn fetch_series(&self) -> anyhow::Result<Vec<SonarrSeriesRaw>> {
        let resp = self
            .http
            .get(format!("{}/api/v3/series", self.url))
            .headers(self.headers())
            .send()
            .await?;

        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            return Err(anyhow::anyhow!("Sonarr returned HTTP {}", status));
        }

        let body: Vec<SonarrSeriesRaw> = resp.json().await?;
        Ok(body)
    }
}
