use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::storage::{SonarrSeriesDb, Storage};
use crate::engine::sonarr::client::{SonarrClient, SonarrSeriesRaw};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportReport {
    pub imported: i64,
    pub auto_mapped: i64,
    pub unmapped: i64,
}

/// Parse the date strings Sonarr returns (RFC 3339 with or without fractional
/// seconds, or a bare `YYYY-MM-DD`). Shared with the calendar command.
pub(crate) fn parse_sonarr_date(s: &str) -> Option<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp());
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ") {
        return Some(dt.and_utc().timestamp());
    }
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().timestamp())
}

async fn optional_parse_sonarr_date(s: &Option<String>) -> Option<i64> {
    s.as_ref().and_then(|s| parse_sonarr_date(s))
}

fn total_episode_count(raw: &SonarrSeriesRaw) -> i32 {
    raw.statistics
        .as_ref()
        .map(|s| s.total_episode_count)
        .unwrap_or(0)
}

fn total_file_count(raw: &SonarrSeriesRaw) -> i32 {
    raw.statistics
        .as_ref()
        .map(|s| s.episode_file_count)
        .unwrap_or(0)
}

fn season_count(raw: &SonarrSeriesRaw) -> i32 {
    if raw.season_count.unwrap_or(0) > 0 {
        raw.season_count.unwrap_or(0)
    } else {
        raw.seasons.len() as i32
    }
}

fn pick_poster_url(raw: &SonarrSeriesRaw) -> Option<String> {
    raw.images
        .iter()
        .find(|img| img.cover_type.as_deref() == Some("poster"))
        .and_then(|img| img.remote_url.clone())
}

/// Default tag labels that mark a series as "one I care about". Overridable via
/// the `sonarr.wanted_tags` setting (a JSON array of tag labels).
const WANTED_TAGS: &[&str] = &["1 - nosut", "mine"];

/// Tag labels used to filter the Sonarr import: the `sonarr.wanted_tags`
/// setting when present, else the built-in default.
async fn wanted_tag_labels(storage: &Storage) -> Vec<String> {
    if let Ok(Some(raw)) = storage.get_setting("sonarr.wanted_tags").await {
        if let Ok(labels) = serde_json::from_str::<Vec<String>>(&raw) {
            return labels;
        }
    }
    WANTED_TAGS.iter().map(|s| s.to_string()).collect()
}

pub async fn import_sonarr_series(
    client: &SonarrClient,
    storage: &Storage,
) -> anyhow::Result<ImportReport> {
    let all_series = client.fetch_series().await?;

    // Resolve which tag ids correspond to the wanted labels, then keep only series
    // carrying at least one of them. When none of the wanted labels exist in this
    // Sonarr instance (e.g. a fresh install without the tags set up), import
    // everything instead of silently importing nothing.
    let wanted_labels = wanted_tag_labels(storage).await;
    let tags = client.fetch_tags().await.unwrap_or_default();
    let wanted_ids: std::collections::HashSet<i64> = tags
        .iter()
        .filter(|t| {
            wanted_labels
                .iter()
                .any(|w| w.eq_ignore_ascii_case(t.label.trim()))
        })
        .map(|t| t.id)
        .collect();
    let raw_series: Vec<&SonarrSeriesRaw> = if wanted_labels.is_empty() {
        tracing::info!("No Sonarr tag filter set — importing all series");
        all_series.iter().collect()
    } else if wanted_ids.is_empty() {
        tracing::info!(
            "No Sonarr tags match {:?} — importing all series (set the \
             sonarr.wanted_tags setting to filter)",
            wanted_labels
        );
        all_series.iter().collect()
    } else {
        all_series
            .iter()
            .filter(|s| s.tags.iter().any(|id| wanted_ids.contains(id)))
            .collect()
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut imported: i64 = 0;
    let mut auto_mapped: i64 = 0;
    let mut unmapped: i64 = 0;

    for raw in raw_series.iter().copied() {
        let ep_count = total_episode_count(raw);
        let file_count = total_file_count(raw);
        let se_count = season_count(raw);

        let series_db = SonarrSeriesDb {
            sonarr_id: raw.id,
            title: raw.title.clone(),
            season_count: se_count,
            episode_count: ep_count,
            episode_file_count: file_count,
            monitored: raw.monitored,
            next_airing: optional_parse_sonarr_date(&raw.next_airing).await,
            path: raw.path.clone(),
            poster_url: pick_poster_url(raw),
            overview: raw.overview.clone(),
            network: raw.network.clone(),
            status: raw.status.clone(),
            added: raw
                .added
                .as_deref()
                .and_then(parse_sonarr_date)
                .unwrap_or(now),
            last_synced: now,
        };

        storage.sonarr_series_upsert(&series_db).await?;
        imported += 1;

        // Never clobber a mapping the user set by hand.
        if let Ok(Some(existing)) = storage.sonarr_mapping_by_sonarr_id(raw.id).await {
            if existing.user_confirmed {
                if existing.anime_id.is_some() {
                    auto_mapped += 1;
                } else {
                    unmapped += 1;
                }
                continue;
            }
        }

        // Auto-match using the SAME normalized title scorer as file/episode
        // matching (punctuation-insensitive, checks romaji/english/native/synonyms),
        // rather than the weaker raw-string series scorer.
        let title = raw.title.as_str();
        let cleaned: String = title
            .chars()
            .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
            .collect();

        // Gather candidates from a broad OR-based search AND an AND word-subsequence
        // search. The AND search is essential: it surfaces the exact title even when
        // the OR search's id-ordered candidate pool overflows on common words
        // (e.g. "The ... and the ..."), which is what left identical titles unmatched.
        let mut candidates: Vec<crate::engine::storage::AnimeRow> = Vec::new();
        for q in [title, cleaned.as_str()] {
            if q.trim().is_empty() {
                continue;
            }
            if let Ok(mut c) = storage.search_anime_by_title(q, 25).await {
                candidates.append(&mut c);
            }
        }
        let words: Vec<&str> = cleaned.split_whitespace().filter(|w| w.len() >= 3).collect();
        if !words.is_empty() {
            if let Ok(mut c) = storage.search_anime_by_words(&words, 20).await {
                candidates.append(&mut c);
            }
        }

        let mut best_id: Option<i64> = None;
        let mut best_score: u8 = 0;
        for c in &candidates {
            let s = crate::engine::matcher::score_titles_json(title, &c.titles_json);
            if s > best_score {
                best_score = s;
                best_id = Some(c.id);
            }
        }

        // Exact/containment matches score >= 80 after normalization; require that
        // for a confident auto-map (word-overlap-only maxes at 60 and stays manual).
        const SONARR_MATCH_THRESHOLD: u8 = 80;
        let mapped = best_score >= SONARR_MATCH_THRESHOLD;
        let mapping = crate::engine::storage::SonarrMappingDb {
            id: None,
            sonarr_id: raw.id,
            anime_id: if mapped { best_id } else { None },
            title_match: title.to_string(),
            confidence: best_score as i32,
            mapped_at: now,
            user_confirmed: false,
        };
        storage.sonarr_mapping_upsert(&mapping).await?;
        if mapped {
            auto_mapped += 1;
        } else {
            unmapped += 1;
        }
    }

    // Drop any previously-imported series that are no longer tag-eligible.
    let keep_ids: Vec<i64> = raw_series.iter().map(|s| s.id).collect();
    storage.prune_sonarr_series_except(&keep_ids).await?;

    Ok(ImportReport {
        imported,
        auto_mapped,
        unmapped,
    })
}
