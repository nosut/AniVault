use sqlx::Row;

use crate::engine::models::ParseResult;
use crate::engine::storage::Storage;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MatchResult {
    pub anime_id: i64,
    pub title: String,
    pub confidence: u8,
    pub source: String,
}

pub async fn build_fts_index(storage: &Storage) -> anyhow::Result<()> {
    storage.ensure_fts_index().await?;
    sqlx::query("DELETE FROM anime_fts")
        .execute(storage.pool())
        .await?;
    sqlx::query(
        "INSERT INTO anime_fts(anime_id, title, synonyms)
         SELECT id,
                COALESCE(json_extract(titles_json, '$.romaji'), ''),
                COALESCE(json_extract(titles_json, '$.english'), '') || ' ' ||
                COALESCE((SELECT group_concat(value, ' ') FROM json_each(json_extract(titles_json, '$.synonyms'))), '')
         FROM anime",
    )
    .execute(storage.pool())
    .await?;
    Ok(())
}

pub async fn search_local(storage: &Storage, parsed: &ParseResult) -> anyhow::Result<Option<MatchResult>> {
    let title = parsed.title.trim();
    if title.is_empty() {
        return Ok(None);
    }

    if let Some(result) = search_exact(storage, title).await? {
        return Ok(Some(result));
    }

    search_fuzzy(storage, title).await
}

async fn search_exact(storage: &Storage, title: &str) -> anyhow::Result<Option<MatchResult>> {
    let row = sqlx::query(
        "SELECT anime_id, title, synonyms FROM anime_fts
         WHERE lower(title) = lower(?1) OR lower(synonyms) LIKE lower(?2)
         LIMIT 1",
    )
    .bind(title)
    .bind(format!("%{}%", title))
    .fetch_optional(storage.pool())
    .await?;

    Ok(row.map(|r| {
        let matched_title: String = r.get(1);
        let confidence = if matched_title.eq_ignore_ascii_case(title) { 100 } else { 85 };
        MatchResult {
            anime_id: r.get(0),
            title: matched_title,
            confidence,
            source: if confidence == 100 { "local_exact" } else { "local_synonym" }.to_string(),
        }
    }))
}

async fn search_fuzzy(storage: &Storage, title: &str) -> anyhow::Result<Option<MatchResult>> {
    let query = fts_query(title);
    let row = sqlx::query(
        "SELECT anime_id, title FROM anime_fts
         WHERE anime_fts MATCH ?1
         LIMIT 1",
    )
    .bind(query)
    .fetch_optional(storage.pool())
    .await?;

    Ok(row.map(|r| MatchResult {
        anime_id: r.get(0),
        title: r.get(1),
        confidence: 75,
        source: "local_fuzzy".to_string(),
    }))
}

fn fts_query(title: &str) -> String {
    title
        .split_whitespace()
        .map(|part| part.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
