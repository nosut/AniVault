use serde::{Deserialize, Serialize};
use serde_json::Value;

const ANILIST_API_URL: &str = "https://graphql.anilist.co";

/// A single GraphQL error returned by AniList.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphQLError {
    pub message: String,
    #[serde(default)]
    pub status: Option<i32>,
}

// ── fetch_user_list response types ────────────────────────────────────────────

/// Top-level wrapper returned by `fetch_user_list`.
#[derive(Debug, Deserialize)]
pub struct MediaListCollectionRaw {
    pub data: Option<MediaListCollectionData>,
    pub errors: Option<Vec<GraphQLError>>,
}

#[derive(Debug, Deserialize)]
pub struct MediaListCollectionData {
    #[serde(rename = "MediaListCollection")]
    pub media_list_collection: Option<MediaListCollection>,
}

#[derive(Debug, Deserialize)]
pub struct MediaListCollection {
    pub lists: Option<Vec<MediaListGroup>>,
}

#[derive(Debug, Deserialize)]
pub struct MediaListGroup {
    pub entries: Option<Vec<MediaListEntry>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MediaListEntry {
    pub media: Option<Media>,
    pub status: Option<String>,
    pub score: Option<f64>,
    pub progress: Option<i32>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<i64>,
    pub notes: Option<String>,
    #[serde(rename = "startedAt")]
    pub started_at: Option<AniListDate>,
    #[serde(rename = "completedAt")]
    pub completed_at: Option<AniListDate>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Media {
    pub id: i64,
    pub title: Option<MediaTitle>,
    pub episodes: Option<i32>,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "coverImage")]
    pub cover_image: Option<CoverImage>,
    pub description: Option<String>,
    #[serde(rename = "nextAiringEpisode")]
    pub next_airing_episode: Option<AiringEpisode>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AiringEpisode {
    #[serde(rename = "airingAt")]
    pub airing_at: i64,
    #[serde(rename = "timeUntilAiring")]
    pub time_until_airing: i64,
    pub episode: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MediaTitle {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CoverImage {
    pub large: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AniListDate {
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub day: Option<i32>,
}

/// Hand-rolled GraphQL client for the AniList API.
pub struct AniListClient {
    pub token: String,
    http: reqwest::Client,
}

impl AniListClient {
    /// Create a new client with the given access token.
    pub fn new(token: String) -> Self {
        Self {
            token,
            http: reqwest::Client::new(),
        }
    }

    /// Execute a generic GraphQL query/mutation against AniList.
    ///
    /// * `query_str` — the raw GraphQL operation string
    /// * `variables` — a JSON object mapping variable names to values
    ///
    /// Returns the deserialized response body on success, or an `anyhow::Error`
    /// describing the failure (HTTP error, network error, or GraphQL errors).
    pub async fn query<T: serde::de::DeserializeOwned>(
        &self,
        query_str: &str,
        variables: Value,
    ) -> anyhow::Result<T> {
        let body = serde_json::json!({
            "query": query_str,
            "variables": variables,
        });

        let response = self
            .http
            .post(ANILIST_API_URL)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let bytes = response.bytes().await?;

        // Return error for HTTP client/server errors.
        if status.is_client_error() || status.is_server_error() {
            let text = String::from_utf8_lossy(&bytes);
            return Err(anyhow::anyhow!(
                "AniList HTTP {}: {}",
                status.as_u16(),
                text
            ));
        }

        // Deserialize as Value first to check for GraphQL errors.
        let raw: Value = serde_json::from_slice(&bytes)?;

        if let Some(errors) = raw.get("errors").and_then(|e| e.as_array()) {
            if !errors.is_empty() {
                let messages: Vec<String> = errors
                    .iter()
                    .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                    .map(String::from)
                    .collect();
                return Err(anyhow::anyhow!(
                    "AniList GraphQL errors: {}",
                    messages.join("; ")
                ));
            }
        }

        // Deserialize the full response as T.
        Ok(serde_json::from_value(raw)?)
    }

    /// Fetch the authenticated user's full anime list.
    ///
    /// Pass `Some(user_name)` to fetch a specific user's list, or `None` to
    /// use the authenticated user (inferred from the access token).
    pub async fn fetch_user_list(
        &self,
        user_name: Option<&str>,
    ) -> anyhow::Result<MediaListCollectionRaw> {
        // When no user_name specified, get the authenticated user's ID first
        let query_str: String = if let Some(name) = user_name {
            format!(r#"
query {{
  MediaListCollection(userName: "{}", type: ANIME) {{
    lists {{ entries {{
      media {{ id title {{ romaji english native }} episodes type status coverImage {{ large }} description }}
      status score progress updatedAt notes
      startedAt {{ year month day }}
      completedAt {{ year month day }}
    }}}}
  }}
}}
"#, name)
        } else {
            // Query Viewer to get authenticated user ID
            let viewer_raw: serde_json::Value = self.query(
                "query { Viewer { id name }}",
                serde_json::json!({}),
            ).await?;
            let user_id = viewer_raw
                .get("data")
                .and_then(|d| d.get("Viewer"))
                .and_then(|v| v.get("id"))
                .and_then(|id| id.as_i64())
                .ok_or_else(|| anyhow::anyhow!("Could not get authenticated user ID"))?;

            format!(
                "query {{ MediaListCollection(userId: {}, type: ANIME) {{ lists {{ entries {{ \
                 media {{ id title {{ romaji english native }} episodes type status coverImage {{ large }} description }} \
                 status score progress updatedAt notes \
                 startedAt {{ year month day }} \
                 completedAt {{ year month day }} \
                 }}}} }}}}",
                user_id
            )
        };
        let variables = serde_json::json!({});
        self.query(&query_str, variables).await
    }

    /// Push episode progress for a given anime.
    ///
    /// Returns `Ok(())` when the mutation succeeds without GraphQL errors.
    pub async fn push_progress(&self, anime_id: i64, episode: i32) -> anyhow::Result<()> {
        let query_str = r#"
mutation ($mediaId: Int, $progress: Int) {
  SaveMediaListEntry(mediaId: $mediaId, progress: $progress) { id progress updatedAt }
}
"#;
        let variables = serde_json::json!({
            "mediaId": anime_id,
            "progress": episode,
        });

        // We don't need to deserialize the actual mutation response body,
        // just check that it succeeded. Using serde_json::Value as a discard.
        self.query::<serde_json::Value>(query_str, variables).await?;
        Ok(())
    }

    /// Fetch the authenticated user's currently-watching anime with airing schedule.
    /// Uses auth token only (no userName) — avoids Viewer query issues.
    pub async fn fetch_airing_schedule(&self) -> anyhow::Result<Vec<AiringEntryRaw>> {
        let query_str = r#"query {
  MediaListCollection(type: ANIME) {
    lists { entries {
      media {
        id title { romaji english native }
        coverImage { large }
        episodes
        nextAiringEpisode { airingAt timeUntilAiring episode }
      }
      progress
    }}
  }
}"#;

        let raw: serde_json::Value = self.query(query_str, serde_json::json!({})).await?;

        // Debug: log response structure for diagnostics
        let entry_count = raw
            .get("data").and_then(|d| d.get("MediaListCollection"))
            .and_then(|c| c.get("lists")).and_then(|l| l.as_array())
            .map_or(0, |a| {
                a.iter().filter_map(|g| g.get("entries")?.as_array()).map(|e| e.len()).sum()
            });
        let has_errors = raw.get("errors").is_some();
        tracing::info!("Calendar: {} entries returned, has_errors={}, raw={}", entry_count, has_errors, raw);

        if let Some(errors) = raw.get("errors") {
            let msg = errors.as_array()
                .map(|a| a.iter().filter_map(|e| e.get("message").and_then(|m| m.as_str())).collect::<Vec<_>>().join("; "))
                .unwrap_or_default();
            return Err(anyhow::anyhow!("AniList error: {}", msg));
        }

        let mut entries = Vec::new();
        if let Some(lists) = raw
            .get("data")
            .and_then(|d| d.get("MediaListCollection"))
            .and_then(|c| c.get("lists"))
            .and_then(|l| l.as_array())
        {
            for group in lists {
                if let Some(group_entries) = group.get("entries").and_then(|e| e.as_array()) {
                    for e in group_entries {
                        if let Ok(entry) = serde_json::from_value::<AiringEntryRaw>(e.clone()) {
                            entries.push(entry);
                        }
                    }
                }
            }
        }
        Ok(entries)
    }

    /// Fetch anime from a specific season with optional genre filter.
    /// season: "WINTER", "SPRING", "SUMMER", "FALL"
    pub async fn fetch_season_anime(
        &self,
        season: &str,
        year: i32,
        genre: Option<&str>,
    ) -> anyhow::Result<Vec<SeasonAnime>> {
        let genre_filter = if let Some(g) = genre {
            format!("genre: \"{}\", ", g)
        } else {
            String::new()
        };
        let query_str = format!(
            "query {{ Page(page: 1, perPage: 50) {{ media(season: {}, seasonYear: {}, {}type: ANIME, sort: POPULARITY_DESC) {{ id title {{ romaji english }} coverImage {{ large }} episodes status format averageScore popularity }} }} }}",
            season, year, genre_filter
        );

        let raw: serde_json::Value = self.query(&query_str, serde_json::json!({})).await?;
        let media_list = raw.get("data").and_then(|d| d.get("Page")).and_then(|p| p.get("media")).and_then(|m| m.as_array()).cloned().unwrap_or_default();
        let entries: Vec<SeasonAnime> = media_list.into_iter().filter_map(|m| serde_json::from_value::<SeasonAnime>(m).ok()).collect();
        Ok(entries)
    }

    /// Fetch related anime for a given anime ID (sequels, prequels, side stories, etc.).
    pub async fn fetch_anime_relations(&self, anime_id: i64) -> anyhow::Result<Vec<RelationEdge>> {
        let query_str = format!(
            "query {{ Media(id: {}, type: ANIME) {{ relations {{ edges {{ relationType node {{ id title {{ romaji english }} type format status coverImage {{ large }} }} }} }} }} }}",
            anime_id
        );
        let raw: serde_json::Value = self.query(&query_str, serde_json::json!({})).await?;
        let edges: Vec<RelationEdge> = raw
            .get("data").and_then(|d| d.get("Media"))
            .and_then(|m| m.get("relations"))
            .and_then(|r| r.get("edges"))
            .and_then(|e| e.as_array())
            .map(|arr| arr.iter().filter_map(|e| serde_json::from_value::<RelationEdge>(e.clone()).ok()).collect())
            .unwrap_or_default();
        Ok(edges)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AiringEntryRaw {
    pub media: Option<Media>,
    pub progress: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SeasonAnime {
    pub id: i64,
    pub title: Option<MediaTitle>,
    #[serde(rename = "coverImage")]
    pub cover_image: Option<CoverImage>,
    pub episodes: Option<i32>,
    pub status: Option<String>,
    pub format: Option<String>,
    #[serde(rename = "averageScore")]
    pub average_score: Option<i32>,
    pub popularity: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnimeRelation {
    pub id: i64,
    pub title: Option<MediaTitle>,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    pub format: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "coverImage")]
    pub cover_image: Option<CoverImage>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationEdge {
    #[serde(rename = "relationType")]
    pub relation_type: String,
    pub node: Option<AnimeRelation>,
}
