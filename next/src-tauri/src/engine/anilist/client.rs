use serde::Deserialize;
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

#[derive(Debug, Clone, Deserialize)]
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct MediaTitle {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
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
        let query_str = if let Some(name) = user_name {
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
            r#"
query {
  MediaListCollection(type: ANIME) {
    lists { entries {
      media { id title { romaji english native } episodes type status coverImage { large } description }
      status score progress updatedAt notes
      startedAt { year month day }
      completedAt { year month day }
    }}
  }
}
"#.to_string()
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
}
