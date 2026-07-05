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
    #[serde(default)]
    pub tags: Vec<i64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrTag {
    pub id: i64,
    pub label: String,
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

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrCalendarSeries {
    pub id: Option<i64>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrCalendarEntry {
    pub title: Option<String>,
    #[serde(rename = "seriesId")]
    pub series_id: Option<i64>,
    pub series: Option<SonarrCalendarSeries>,
    #[serde(rename = "seasonNumber")]
    pub season_number: Option<i32>,
    #[serde(rename = "episodeNumber")]
    pub episode_number: Option<i32>,
    #[serde(rename = "airDate")]
    pub air_date: Option<String>,
    #[serde(rename = "airDateUtc")]
    pub air_date_utc: Option<String>,
    #[serde(rename = "hasFile")]
    pub has_file: Option<bool>,
    pub id: Option<i64>,
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

    /// Fetch the tag definitions (id → label) configured in Sonarr.
    pub async fn fetch_tags(&self) -> anyhow::Result<Vec<SonarrTag>> {
        let resp = self
            .http
            .get(format!("{}/api/v3/tag", self.url))
            .headers(self.headers())
            .send()
            .await?;
        if resp.status().is_client_error() || resp.status().is_server_error() {
            return Err(anyhow::anyhow!("Sonarr returned HTTP {}", resp.status()));
        }
        let body: Vec<SonarrTag> = resp.json().await?;
        Ok(body)
    }

    /// Fetch upcoming calendar entries from Sonarr.
    /// start and end are ISO date strings like "2026-07-01"
    pub async fn fetch_calendar(&self, start: &str, end: &str) -> anyhow::Result<Vec<SonarrCalendarEntry>> {
        let url = format!("{}/api/v3/calendar?start={}&end={}&includeSeries=true", self.url, start, end);
        let resp = self.http.get(&url).headers(self.headers()).send().await?;
        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Sonarr calendar HTTP {}: {}", status, body));
        }
        let body: Vec<SonarrCalendarEntry> = resp.json().await?;
        Ok(body)
    }
}
