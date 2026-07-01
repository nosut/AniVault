/// Pending low-confidence match queue stored in settings table as JSON array.
/// Entries are created when AniList returns a match with confidence 50-84.

use sqlx::Row;

const SETTING_KEY_PENDING: &str = "pending_matches";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingMatch {
    pub anilist_id: i64,
    pub title_romaji: String,
    pub title_english: Option<String>,
    pub synonyms: Vec<String>,
    pub episode_count: Option<i32>,
    pub parsed_title: String,
    pub confidence: u8,
}

pub async fn get_pending_matches(
    storage: &crate::engine::storage::Storage,
) -> anyhow::Result<Vec<PendingMatch>> {
    let row = sqlx::query("SELECT value_json FROM settings WHERE key = ?1")
        .bind(SETTING_KEY_PENDING)
        .fetch_optional(storage.pool())
        .await?;

    let Some(row) = row else {
        return Ok(Vec::new());
    };

    let value: String = row.get(0);
    if value.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&value).unwrap_or_default())
}

pub async fn store_pending_match(
    storage: &crate::engine::storage::Storage,
    result: &crate::engine::anilist::AniListSearchResult,
    parsed_title: &str,
    confidence: u8,
) -> anyhow::Result<()> {
    let mut pending = get_pending_matches(storage).await?;

    // Don't duplicate
    if pending.iter().any(|p| p.anilist_id == result.anilist_id) {
        return Ok(());
    }

    pending.push(PendingMatch {
        anilist_id: result.anilist_id,
        title_romaji: result.title_romaji.clone(),
        title_english: result.title_english.clone(),
        synonyms: result.synonyms.clone(),
        episode_count: result.episode_count,
        parsed_title: parsed_title.to_string(),
        confidence,
    });

    save_pending_matches(storage, &pending).await
}

pub async fn confirm_pending_match(
    storage: &crate::engine::storage::Storage,
    anilist_id: i64,
) -> anyhow::Result<()> {
    let pending = get_pending_matches(storage).await?;
    let Some(found) = pending.iter().find(|p| p.anilist_id == anilist_id) else {
        anyhow::bail!("no pending match for id {anilist_id}");
    };

    let synonyms: Vec<&str> = found.synonyms.iter().map(String::as_str).collect();
    storage
        .insert_minimal_anime_with_synonyms(found.anilist_id, &found.title_romaji, &synonyms)
        .await?;

    if let Some(ep_count) = found.episode_count {
        sqlx::query("UPDATE anime SET episode_count = ?1 WHERE id = ?2")
            .bind(ep_count)
            .bind(found.anilist_id)
            .execute(storage.pool())
            .await?;
    }

    crate::engine::recognition::matcher::build_fts_index(storage).await?;

    let remaining: Vec<PendingMatch> = pending.into_iter().filter(|p| p.anilist_id != anilist_id).collect();
    save_pending_matches(storage, &remaining).await
}

pub async fn reject_pending_match(
    storage: &crate::engine::storage::Storage,
    anilist_id: i64,
) -> anyhow::Result<()> {
    let pending = get_pending_matches(storage).await?;
    let remaining: Vec<PendingMatch> = pending.into_iter().filter(|p| p.anilist_id != anilist_id).collect();
    save_pending_matches(storage, &remaining).await
}

async fn save_pending_matches(
    storage: &crate::engine::storage::Storage,
    pending: &[PendingMatch],
) -> anyhow::Result<()> {
    let value = serde_json::to_string(pending)?;
    sqlx::query(
        "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, unixepoch())
         ON CONFLICT(key) DO UPDATE SET value_json = ?2, updated_at = unixepoch()",
    )
    .bind(SETTING_KEY_PENDING)
    .bind(&value)
    .execute(storage.pool())
    .await?;
    Ok(())
}
