use crate::engine::anilist::client::{AniListClient, MediaListCollectionRaw};
use crate::engine::storage::Storage;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub imported: u64,
    pub merged: u64,
    pub skipped: u64,
}

/// Merge one list entry from AniList.
///
/// Compares `anilist_updated_at` against the greater of `local_updated` and
/// `remote_updated`. Returns `true` if the entry was merged (AniList won),
/// `false` if skipped (local data is newer or equal).
pub async fn merge_entry(
    storage: &Storage,
    anime_id: i64,
    status: &str,
    progress: Option<i32>,
    score: Option<i32>,
    notes: &str,
    anilist_updated_at: i64,
) -> anyhow::Result<bool> {
    let local = storage.get_list_entry_full(anime_id).await?;
    let local_updated = local.as_ref().map(|e| e.local_updated).unwrap_or(0);
    let remote_updated = local.as_ref().and_then(|e| e.remote_updated).unwrap_or(0);
    let local_max = local_updated.max(remote_updated);

    // If local entry exists and is newer or equal, skip.
    if anilist_updated_at <= local_max && local.is_some() {
        return Ok(false);
    }

    // Determine final values — prefer AniList data, fall back to local.
    let episodes = progress.unwrap_or_else(|| {
        local.as_ref().map(|e| e.watched_episodes).unwrap_or(0)
    });
    let score_val = score.or_else(|| local.as_ref().and_then(|e| e.score));
    let notes_val = if notes.is_empty() {
        local
            .as_ref()
            .and_then(|e| e.notes.clone())
            .unwrap_or_default()
    } else {
        notes.to_string()
    };

    storage
        .upsert_list_entry_full(anime_id, status, episodes, score_val, &notes_val, local_updated, anilist_updated_at)
        .await?;

    Ok(true)
}

/// Fetch the authenticated user's full AniList library and merge it into local
/// storage using most-recent-wins semantics.
pub async fn import_library(
    client: &AniListClient,
    storage: &Storage,
) -> anyhow::Result<ImportReport> {
    let raw: MediaListCollectionRaw = client.fetch_user_list(None).await?;
    let mut report = ImportReport {
        imported: 0,
        merged: 0,
        skipped: 0,
    };

    let collection = match raw.data.and_then(|d| d.media_list_collection) {
        Some(c) => c,
        None => return Ok(report),
    };

    for list in collection.lists.unwrap_or_default() {
        for entry in list.entries.unwrap_or_default() {
            let media = match entry.media {
                Some(m) => m,
                None => continue,
            };

            let titles_json = serde_json::json!({
                "romaji": media.title.as_ref().and_then(|t| t.romaji.as_deref()).unwrap_or(""),
                "english": media.title.as_ref().and_then(|t| t.english.as_deref()),
                "japanese": media.title.as_ref().and_then(|t| t.native.as_deref()),
                "synonyms": [],
            })
            .to_string();

            let episode_count = media.episodes.unwrap_or(0);
            let image_url = media.cover_image.as_ref().and_then(|c| c.large.clone());
            let updated_at = entry.updated_at.unwrap_or(0);

            let synopsis = media.description.as_deref();
            let anime_type = media.media_type.as_deref();
            let anime_status = media.status.as_deref();
            storage
                .upsert_anime_full(media.id, &titles_json, episode_count, image_url.as_deref(), synopsis, anime_type, anime_status, updated_at)
                .await?;
            storage
                .set_anime_season(media.id, media.season.as_deref(), media.season_year)
                .await?;

            let status = entry.status.unwrap_or_else(|| "PLANNING".to_string());
            let mapped_status = match status.as_str() {
                "CURRENT" => "watching",
                "COMPLETED" => "completed",
                "PAUSED" => "on_hold",
                "DROPPED" => "dropped",
                _ => "plan_to_watch",
            };

            let merged = merge_entry(
                storage,
                media.id,
                mapped_status,
                entry.progress,
                entry.score.map(|s| s as i32),
                entry.notes.as_deref().unwrap_or(""),
                updated_at,
            )
            .await?;

            if merged {
                report.merged += 1;
            } else {
                report.skipped += 1;
            }
            report.imported += 1;

            storage
                .upsert_tracker_mapping(media.id, "anilist", &media.id.to_string())
                .await?;
        }
    }

    Ok(report)
}
