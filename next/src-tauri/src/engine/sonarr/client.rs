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
    #[serde(rename = "tvdbId", default)]
    pub tvdb_id: Option<i64>,
    #[serde(rename = "alternateTitles", default)]
    pub alternate_titles: Vec<SonarrAlternateTitle>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrTag {
    pub id: i64,
    pub label: String,
}

/// A tag from Sonarr's `/api/v3/tag/detail`, which also lists what uses it.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrTagDetail {
    pub id: i64,
    pub label: String,
    #[serde(rename = "seriesIds", default)]
    pub series_ids: Vec<i64>,
}

/// A Sonarr tag as offered in the import filter.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SonarrTagOption {
    pub id: i64,
    pub label: String,
    pub series_count: usize,
}

/// Tag options sorted by label (case-insensitive), with how many series carry each.
pub fn tag_options(details: Vec<SonarrTagDetail>) -> Vec<SonarrTagOption> {
    let mut options: Vec<SonarrTagOption> = details
        .into_iter()
        .map(|d| SonarrTagOption {
            id: d.id,
            series_count: d.series_ids.len(),
            label: d.label,
        })
        .collect();
    options.sort_by_key(|o| o.label.to_lowercase());
    options
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrAlternateTitle {
    pub title: String,
}

/// An import list from `/api/v3/importlist`; only the fields a new series copies.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SonarrImportList {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub implementation: String,
    pub root_folder_path: Option<String>,
    pub quality_profile_id: Option<i64>,
    pub series_type: Option<String>,
    pub season_folder: Option<bool>,
    pub should_monitor: Option<String>,
    pub monitor_new_items: Option<String>,
    #[serde(default)]
    pub tags: Vec<i64>,
}

/// The AniList import list, whose settings new series copy.
pub fn pick_anilist_import_list(lists: &[SonarrImportList]) -> Option<&SonarrImportList> {
    lists
        .iter()
        .find(|l| l.implementation.to_lowercase().contains("anilist"))
}

/// A series from `/api/v3/series/lookup`. `id > 0` means Sonarr already has it.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SonarrLookupSeries {
    #[serde(default)]
    pub id: Option<i64>,
    pub title: String,
    pub year: Option<i32>,
    pub tvdb_id: Option<i64>,
    pub season_count: Option<i32>,
    pub overview: Option<String>,
    #[serde(default)]
    pub images: Vec<SonarrImageRaw>,
    #[serde(default)]
    pub seasons: Vec<serde_json::Value>,
}

/// A lookup result offered in the Add dialog.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SonarrCandidate {
    pub tvdb_id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub season_count: i32,
    pub poster_url: Option<String>,
    pub overview: Option<String>,
    pub in_sonarr: bool,
    pub sonarr_id: Option<i64>,
}

/// Lookup results as dialog candidates: TVDB-less results dropped, duplicates
/// (same `tvdbId`) collapsed to the first, input order kept, capped at `limit`.
pub fn lookup_candidates(results: Vec<SonarrLookupSeries>, limit: usize) -> Vec<SonarrCandidate> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for r in results {
        let Some(tvdb_id) = r.tvdb_id else { continue };
        if !seen.insert(tvdb_id) {
            continue;
        }
        let sonarr_id = r.id.filter(|&id| id > 0);
        let poster_url = r
            .images
            .iter()
            .find(|i| i.cover_type.as_deref() == Some("poster"))
            .and_then(|i| i.remote_url.clone());
        out.push(SonarrCandidate {
            tvdb_id,
            title: r.title,
            year: r.year,
            season_count: r.season_count.unwrap_or(r.seasons.len() as i32),
            poster_url,
            overview: r.overview,
            in_sonarr: sonarr_id.is_some(),
            sonarr_id,
        });
        if out.len() == limit {
            break;
        }
    }
    out
}

/// Run `lookup` for each search term in order and collect the unique
/// candidates, in the order found. A term that fails is logged and skipped;
/// the search fails only when every term failed. Every term is tried: the
/// right series often comes from a later term.
pub async fn lookup_across_terms<F, Fut>(terms: &[String], mut lookup: F) -> anyhow::Result<Vec<SonarrCandidate>>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Vec<SonarrLookupSeries>>>,
{
    let mut results = Vec::new();
    let mut last_error = None;
    let mut any_ok = false;
    for term in terms {
        match lookup(term.clone()).await {
            Ok(found) => {
                any_ok = true;
                results.extend(found);
            }
            Err(e) => {
                tracing::warn!("Sonarr lookup for {term:?} failed: {e}");
                last_error = Some(e);
            }
        }
    }
    match last_error {
        Some(e) if !any_ok => Err(e),
        _ => Ok(lookup_candidates(results, usize::MAX)),
    }
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
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build reqwest client"),
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

    /// Fetch tags with the series that use them (`/api/v3/tag/detail`).
    pub async fn fetch_tag_details(&self) -> anyhow::Result<Vec<SonarrTagDetail>> {
        let resp = self
            .http
            .get(format!("{}/api/v3/tag/detail", self.url))
            .headers(self.headers())
            .send()
            .await?;
        if resp.status().is_client_error() || resp.status().is_server_error() {
            return Err(anyhow::anyhow!("Sonarr returned HTTP {}", resp.status()));
        }
        Ok(resp.json().await?)
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

    /// All episodes of a series, for resolving an absolute episode number to a
    /// Sonarr episode id.
    pub async fn list_episodes(&self, series_id: i64) -> anyhow::Result<Vec<SonarrEpisode>> {
        let url = format!("{}/api/v3/episode?seriesId={}", self.url, series_id);
        let resp = self.http.get(&url).headers(self.headers()).send().await?;
        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Sonarr episodes HTTP {}: {}", status, body));
        }
        Ok(resp.json().await?)
    }

    /// Ask Sonarr to search its indexers for the given episodes.
    pub async fn search_episodes(&self, episode_ids: &[i64]) -> anyhow::Result<()> {
        let url = format!("{}/api/v3/command", self.url);
        let resp = self
            .http
            .post(&url)
            .headers(self.headers())
            .json(&serde_json::json!({ "name": "EpisodeSearch", "episodeIds": episode_ids }))
            .send()
            .await?;
        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Sonarr episode search HTTP {}: {}", status, body));
        }
        Ok(())
    }

    /// Import lists configured in Sonarr.
    pub async fn fetch_import_lists(&self) -> anyhow::Result<Vec<SonarrImportList>> {
        let url = format!("{}/api/v3/importlist", self.url);
        let resp = self.http.get(&url).headers(self.headers()).send().await?;
        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Sonarr import lists HTTP {}: {}", status, body));
        }
        Ok(resp.json().await?)
    }

    /// Search Sonarr's metadata source for series matching `term`.
    pub async fn lookup_series(&self, term: &str) -> anyhow::Result<Vec<SonarrLookupSeries>> {
        let url = format!("{}/api/v3/series/lookup", self.url);
        let resp = self
            .http
            .get(&url)
            .headers(self.headers())
            .query(&[("term", term)])
            .send()
            .await?;
        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Sonarr lookup HTTP {}: {}", status, body));
        }
        Ok(resp.json().await?)
    }

    /// Add a series; returns its new Sonarr id. Sonarr's error text (e.g.
    /// "This series has already been added") is passed through.
    pub async fn add_series(&self, body: &serde_json::Value) -> anyhow::Result<i64> {
        let url = format!("{}/api/v3/series", self.url);
        let resp = self.http.post(&url).headers(self.headers()).json(body).send().await?;
        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Sonarr add series HTTP {}: {}", status, body));
        }
        let created: serde_json::Value = resp.json().await?;
        created["id"]
            .as_i64()
            .ok_or_else(|| anyhow::anyhow!("Sonarr did not return the new series id"))
    }

    /// Turn monitoring of a series on or off. Sonarr's PUT takes the whole
    /// series resource, so fetch it, flip the flag, and send it back.
    pub async fn set_series_monitored(&self, series_id: i64, monitored: bool) -> anyhow::Result<()> {
        let url = format!("{}/api/v3/series/{}", self.url, series_id);
        let resp = self.http.get(&url).headers(self.headers()).send().await?;
        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Sonarr series HTTP {}: {}", status, body));
        }
        let mut series: serde_json::Value = resp.json().await?;
        series["monitored"] = serde_json::Value::Bool(monitored);
        let resp = self
            .http
            .put(&url)
            .headers(self.headers())
            .json(&series)
            .send()
            .await?;
        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Sonarr update series HTTP {}: {}", status, body));
        }
        Ok(())
    }

    /// Remove a series from Sonarr, optionally deleting its files on the
    /// Sonarr host too. A series Sonarr no longer has counts as removed.
    pub async fn delete_series(&self, series_id: i64, delete_files: bool) -> anyhow::Result<()> {
        let url = format!("{}/api/v3/series/{}", self.url, series_id);
        let resp = self
            .http
            .delete(&url)
            .headers(self.headers())
            .query(&[("deleteFiles", delete_files.to_string())])
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }
        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Sonarr delete series HTTP {}: {}", status, body));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrEpisode {
    pub id: i64,
    #[serde(rename = "seasonNumber")]
    pub season_number: i32,
    #[serde(rename = "episodeNumber")]
    pub episode_number: i32,
    #[serde(rename = "absoluteEpisodeNumber")]
    #[serde(default)]
    pub absolute_episode_number: Option<i32>,
}

/// Sonarr episode id for an AniList (absolute) episode number: prefer the
/// absolute-number match; fall back to the plain episode number in a regular
/// season (skipping season-0 specials) for series without absolute numbering.
pub fn find_episode_id(episodes: &[SonarrEpisode], target: i32) -> Option<i64> {
    episodes
        .iter()
        .find(|e| e.absolute_episode_number == Some(target))
        .or_else(|| {
            episodes
                .iter()
                .find(|e| e.season_number > 0 && e.episode_number == target)
        })
        .map(|e| e.id)
}
