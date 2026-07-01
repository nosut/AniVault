/// AniList GraphQL search and auto-add integration.
/// Uses the public AniList API (no auth required for search).
/// Token from OAuth used only for rate-limit advantages.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AniListSearchResult {
    pub anilist_id: i64,
    pub title_romaji: String,
    pub title_english: Option<String>,
    pub synonyms: Vec<String>,
    pub episode_count: Option<i32>,
}

pub fn parse_anilist_search_response(json: &str) -> anyhow::Result<Vec<AniListSearchResult>> {
    let parsed: serde_json::Value = serde_json::from_str(json)?;
    let media = parsed["data"]["Page"]["media"].as_array();

    let Some(media) = media else {
        return Ok(Vec::new());
    };

    media
        .iter()
        .map(|m| {
            Ok(AniListSearchResult {
                anilist_id: m["id"].as_i64().ok_or_else(|| anyhow::anyhow!("missing id"))?,
                title_romaji: m["title"]["romaji"].as_str().unwrap_or("").to_string(),
                title_english: m["title"]["english"].as_str().map(String::from),
                synonyms: m["synonyms"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                episode_count: m["episodes"].as_i64().map(|e| e as i32),
            })
        })
        .collect()
}

pub fn score_anilist_match(query: &str, result: &AniListSearchResult) -> u8 {
    let query_lower = query.to_ascii_lowercase();

    if result.title_romaji.to_ascii_lowercase() == query_lower {
        return 95;
    }

    if let Some(ref eng) = result.title_english {
        if eng.to_ascii_lowercase() == query_lower {
            return 80;
        }
    }

    for synonym in &result.synonyms {
        if synonym.to_ascii_lowercase() == query_lower {
            return 60;
        }
    }

    let romaji_lower = result.title_romaji.to_ascii_lowercase();
    if romaji_lower.contains(&query_lower) || query_lower.contains(&romaji_lower) {
        return 55;
    }

    0
}

pub async fn search_anilist_graphql(title: &str) -> anyhow::Result<Vec<AniListSearchResult>> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://graphql.anilist.co")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&serde_json::json!({
            "query": "query ($search: String) { Page(perPage: 3) { media(search: $search, type: ANIME) { id title { romaji english } synonyms episodes } } }",
            "variables": { "search": title }
        }))
        .send()
        .await?;

    let body = resp.text().await?;
    parse_anilist_search_response(&body)
}

pub async fn auto_add_anime(
    storage: &crate::engine::storage::Storage,
    result: &AniListSearchResult,
) -> anyhow::Result<i64> {
    let synonyms: Vec<&str> = result.synonyms.iter().map(String::as_str).collect();

    storage
        .insert_minimal_anime_with_synonyms(result.anilist_id, &result.title_romaji, &synonyms)
        .await?;

    if let Some(ep_count) = result.episode_count {
        sqlx::query("UPDATE anime SET episode_count = ?1 WHERE id = ?2")
            .bind(ep_count)
            .bind(result.anilist_id)
            .execute(storage.pool())
            .await?;
    }

    Ok(result.anilist_id)
}

pub async fn search_anilist_fallback(
    storage: &crate::engine::storage::Storage,
    title: &str,
) -> anyhow::Result<Option<crate::engine::recognition::matcher::MatchResult>> {
    let results = search_anilist_graphql(title).await?;

    let best = results
        .into_iter()
        .map(|r| {
            let confidence = score_anilist_match(title, &r);
            (r, confidence)
        })
        .filter(|(_, c)| *c >= 50)
        .max_by_key(|(_, c)| *c);

    let Some((best_result, confidence)) = best else {
        return Ok(None);
    };

    if confidence < 85 {
        return Ok(None);
    }

    let anime_id = auto_add_anime(storage, &best_result).await?;
    crate::engine::recognition::matcher::build_fts_index(storage).await?;

    Ok(Some(crate::engine::recognition::matcher::MatchResult {
        anime_id,
        title: best_result.title_romaji,
        confidence,
        source: "anilist_search".to_string(),
    }))
}
