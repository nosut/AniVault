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
    #[serde(rename = "seasonCount")]
    pub season_count: Option<i32>,
    #[serde(default)]
    pub seasons: Vec<SonarrSeasonRaw>,
    pub monitored: bool,
    #[serde(rename = "nextAiring")]
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
    #[serde(rename = "episodeCount")]
    #[serde(default)]
    pub episode_count: i32,
    #[serde(rename = "episodeFileCount")]
    #[serde(default)]
    pub episode_file_count: i32,
    #[serde(rename = "totalEpisodeCount")]
    #[serde(default)]
    pub total_episode_count: i32,
    #[serde(rename = "nextAiring")]
    pub next_airing: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrImageRaw {
    #[serde(rename = "coverType")]
    pub cover_type: Option<String>,
    #[serde(rename = "remoteUrl")]
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
        if let Ok(value) = HeaderValue::from_str(&self.api_key) {
            headers.insert("X-Api-Key", value);
        }
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
