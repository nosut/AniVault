use crate::engine::events::{EngineEvent, MatchCandidate};
use crate::engine::parser::{parse_filename, ParsedFilename};
use crate::engine::runtime::EngineState;
use crate::engine::storage::Storage;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecognitionResult {
    pub known_file: bool,
    pub parsed: Option<ParsedFilename>,
    pub candidates: Vec<MatchCandidate>,
}

fn normalize_title(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn score_title_match(query: &str, candidate: &str) -> u8 {
    let q = normalize_title(query);
    let c = normalize_title(candidate);
    if q == c {
        return 100;
    }
    if q.contains(&c) || c.contains(&q) {
        return 80;
    }
    // Simple word-overlap score
    let q_words: std::collections::HashSet<&str> = q.split_whitespace().collect();
    let c_words: std::collections::HashSet<&str> = c.split_whitespace().collect();
    let overlap = q_words.intersection(&c_words).count();
    let total = q_words.len().max(c_words.len());
    if total == 0 {
        return 0;
    }
    ((overlap as f64 / total as f64) * 60.0) as u8
}

/// Extract just the filename from a path for parsing.
/// Preserves the full path for file index lookups.
fn strip_path(file_path: &str) -> &str {
    std::path::Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file_path)
}

pub async fn recognize_file(
    file_path: &str,
    window_title: Option<&str>,
    storage: &Storage,
) -> anyhow::Result<RecognitionResult> {
    // Check remembered file index first
    if let Some(existing) = storage.get_file_index(file_path).await? {
        if let Some(anime_id) = existing.anime_id {
            if let Some(anime) = storage.fetch_anime(anime_id).await? {
                let titles: serde_json::Value =
                    serde_json::from_str(&anime.titles_json).unwrap_or_default();
                let title = titles["romaji"].as_str().unwrap_or("Unknown").to_string();
                return Ok(RecognitionResult {
                    known_file: true,
                    parsed: None,
                    candidates: vec![MatchCandidate {
                        anime_id,
                        title,
                        confidence: existing.confidence as u8,
                        match_source: "file_index".to_string(),
                    }],
                });
            }
        }
    }

    // Parse the filename (use window title as-is, or strip directory from file path)
    let parse_source = window_title.unwrap_or(strip_path(file_path));
    let parsed = match parse_filename(parse_source, None) {
        Some(p) => p,
        None => {
            return Ok(RecognitionResult {
                known_file: false,
                parsed: None,
                candidates: vec![],
            })
        }
    };

    // Search local library with normalized title
    let matches = storage
        .search_anime_by_title(&parsed.cleaned_title, 10)
        .await?;

    let mut candidates: Vec<MatchCandidate> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    for anime in &matches {
        if !seen_ids.insert(anime.id) {
            continue;
        }
        let titles: serde_json::Value =
            serde_json::from_str(&anime.titles_json).unwrap_or_default();
        let romaji = titles["romaji"].as_str().unwrap_or("");
        let english = titles["english"].as_str().unwrap_or("");
        let japanese = titles["japanese"].as_str().unwrap_or("");

        let score = [
            score_title_match(&parsed.cleaned_title, romaji),
            score_title_match(&parsed.cleaned_title, english),
            score_title_match(&parsed.cleaned_title, japanese),
        ]
        .into_iter()
        .max()
        .unwrap_or(0);

        let synonyms: Vec<String> = titles["synonyms"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let syn_score = synonyms
            .iter()
            .map(|s| score_title_match(&parsed.cleaned_title, s))
            .max()
            .unwrap_or(0);

        let confidence = score.max(syn_score);

        if confidence >= 20 {
            candidates.push(MatchCandidate {
                anime_id: anime.id,
                title: romaji.to_string(),
                confidence,
                match_source: "title_match".to_string(),
            });
        }
    }

    candidates.sort_by(|a, b| b.confidence.cmp(&a.confidence));

    Ok(RecognitionResult {
        known_file: false,
        parsed: Some(parsed),
        candidates,
    })
}

pub async fn confirm_identification(
    state: &EngineState,
    file_path: &str,
    anime_id: i64,
    episode: i32,
) -> anyhow::Result<()> {
    let now = crate::commands::unix_now_inner()?;

    state
        .storage
        .upsert_file_index(file_path, anime_id, episode, 100, now)
        .await?;

    state.events.publish(EngineEvent::AnimeIdentified(
        crate::engine::events::AnimeIdentified {
            anime_id,
            episode,
            confidence: 100,
            evidence: format!("user confirmed: {file_path}"),
        },
    ));

    Ok(())
}
