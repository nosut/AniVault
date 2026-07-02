use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::parser::parse_filename;
use crate::engine::storage::{SonarrSeriesDb, Storage};
use crate::engine::sonarr::client::{SonarrClient, SonarrSeriesRaw};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ImportReport {
    pub imported: i64,
    pub auto_mapped: i64,
    pub unmapped: i64,
}

/// Simple Levenshtein distance between two strings.
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let len_a = a_chars.len();
    let len_b = b_chars.len();
    if len_a == 0 {
        return len_b;
    }
    if len_b == 0 {
        return len_a;
    }
    let mut prev: Vec<usize> = (0..=len_b).collect();
    let mut curr = vec![0usize; len_b + 1];
    for i in 1..=len_a {
        curr[0] = i;
        for j in 1..=len_b {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            curr[j] = (prev[j] + 1)
                .min(curr[j - 1] + 1)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[len_b]
}

/// Score a Sonarr series title against an anime library title.
/// `anime_titles_json` is a JSON object string with fields: romaji, english, japanese, synonyms[].
/// Returns 0-100+ (bonus points can push over 100).
pub fn score_match_series(
    sonarr_title: &str,
    anime_titles_json: &str,
    sonarr_ep_count: i32,
    anime_ep_count: Option<i32>,
) -> i32 {
    let titles: serde_json::Value = serde_json::from_str(anime_titles_json).unwrap_or_default();
    let romaji = titles.get("romaji").and_then(|v| v.as_str()).unwrap_or("");
    let english = titles.get("english").and_then(|v| v.as_str()).unwrap_or("");
    let japanese = titles.get("japanese").and_then(|v| v.as_str()).unwrap_or("");
    let synonyms: Vec<&str> = titles
        .get("synonyms")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|s| s.as_str()).collect())
        .unwrap_or_default();

    let sonarr_lower = sonarr_title.to_lowercase();
    let candidates: Vec<&str> = std::iter::once(romaji)
        .chain(std::iter::once(english))
        .chain(std::iter::once(japanese))
        .chain(synonyms.into_iter())
        .filter(|s| !s.is_empty())
        .collect();

    let mut best = 0;

    for candidate in &candidates {
        let cand_lower = candidate.to_lowercase();

        if cand_lower == sonarr_lower {
            best = best.max(100);
        } else if cand_lower.contains(&sonarr_lower) || sonarr_lower.contains(&cand_lower) {
            best = best.max(60);
        } else {
            // Simple word overlap: count shared words
            let sonarr_words: Vec<&str> = sonarr_lower.split_whitespace().collect();
            let cand_words: Vec<&str> = cand_lower.split_whitespace().collect();
            let shared = sonarr_words
                .iter()
                .filter(|w| w.len() > 2 && cand_words.contains(w))
                .count();

            if shared > 0 {
                let ratio = (shared as f64 / sonarr_words.len().max(1) as f64 * 40.0) as i32;
                best = best.max(ratio);
            }
        }

        // Bonus: Levenshtein distance < 3 adds +40
        if levenshtein(&sonarr_lower, &cand_lower) < 3 {
            best = best.max(40);
        }
    }

    // Bonus: episode count within ±3 (+20) or ±10 (+5)
    if let Some(anime_ep) = anime_ep_count {
        if sonarr_ep_count > 0 && anime_ep > 0 {
            let diff = (sonarr_ep_count - anime_ep).abs();
            if diff <= 3 {
                best += 20;
            } else if diff <= 10 {
                best += 5;
            }
        }
    }

    best
}

fn parse_sonarr_date(s: &str) -> Option<i64> {
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d"))
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(s).map(|d| d.naive_utc()))
        .ok()
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

pub async fn import_sonarr_series(
    client: &SonarrClient,
    storage: &Storage,
) -> anyhow::Result<ImportReport> {
    let raw_series = client.fetch_series().await?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut imported: i64 = 0;
    let mut auto_mapped: i64 = 0;
    let mut unmapped: i64 = 0;

    for raw in &raw_series {
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

        // Try auto-match
        let parsed = parse_filename(&raw.title, None);
        let search_title = parsed
            .as_ref()
            .map(|p| p.cleaned_title.as_str())
            .unwrap_or(&raw.title);

        let candidates = storage.search_anime_by_title(search_title, 5).await?;

        let best = candidates
            .iter()
            .map(|anime| {
                let score = score_match_series(
                    &raw.title,
                    &anime.titles_json,
                    ep_count,
                    anime.episode_count,
                );
                (anime.id, score)
            })
            .max_by_key(|(_, score)| *score);

        if let Some((anime_id, score)) = best {
            let mapping = crate::engine::storage::SonarrMappingDb {
                id: None,
                sonarr_id: raw.id,
                anime_id: if score >= 80 { Some(anime_id) } else { None },
                title_match: search_title.to_string(),
                confidence: score,
                mapped_at: now,
                user_confirmed: false,
            };
            storage.sonarr_mapping_upsert(&mapping).await?;

            if score >= 80 {
                auto_mapped += 1;
            } else {
                unmapped += 1;
            }
        } else {
            // No candidates at all — store as unmapped
            let mapping = crate::engine::storage::SonarrMappingDb {
                id: None,
                sonarr_id: raw.id,
                anime_id: None,
                title_match: search_title.to_string(),
                confidence: 0,
                mapped_at: now,
                user_confirmed: false,
            };
            storage.sonarr_mapping_upsert(&mapping).await?;
            unmapped += 1;
        }
    }

    Ok(ImportReport {
        imported,
        auto_mapped,
        unmapped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_close_match_adds_bonus() {
        // "Attack on Titan" vs "Attack on Titans" — one char diff, dist = 1
        let score = score_match_series("Attack on Titan", r#"{"romaji":"Attack on Titans","english":"","japanese":"","synonyms":[]}"#, 25, Some(25));
        assert!(score >= 40, "expected >= 40, got {}", score);
    }

    #[test]
    fn levenshtein_distance_works() {
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("abc", ""), 3);
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("attack on titan", "attack on titans"), 1);
        assert_eq!(levenshtein("hello", "hello"), 0);
    }
}
