use crate::engine::events::{EngineEvent, MatchCandidate};
use crate::engine::parser::{parse_filename, ParsedFilename};
use crate::engine::runtime::EngineState;
use crate::engine::storage::{MappingSource, Storage};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecognitionResult {
    pub known_file: bool,
    pub parsed: Option<ParsedFilename>,
    pub candidates: Vec<MatchCandidate>,
}

/// Score a query against every title variant stored in an anime row's `titles_json`
/// (romaji, english, japanese, and synonyms). Returns the best match score 0..=100.
/// Shared by the real-time recognizer and the library scanner so both rank identically.
pub fn score_titles_json(query: &str, titles_json: &str) -> u8 {
    let titles: serde_json::Value = serde_json::from_str(titles_json).unwrap_or_default();
    let mut best = 0u8;
    for key in ["romaji", "english", "japanese"] {
        if let Some(t) = titles[key].as_str() {
            best = best.max(score_title_match(query, t));
        }
    }
    if let Some(arr) = titles["synonyms"].as_array() {
        for v in arr {
            if let Some(s) = v.as_str() {
                best = best.max(score_title_match(query, s));
            }
        }
    }
    best
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
    // Containment match — but guard against a very short title matching inside an
    // unrelated longer one (e.g. "K" or "Air" is a substring of countless titles).
    // Only award the containment score when the shorter (contained) string is
    // long enough to be a meaningful signal.
    if q.len().min(c.len()) >= 4 && (q.contains(&c) || c.contains(&q)) {
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

/// Does this string look like a real filesystem path (vs. a player's window
/// title)? Real paths contain a directory separator; mpv/VLC titles don't.
/// Used to avoid indexing/looking-up bogus window-title "paths".
pub fn looks_like_path(s: &str) -> bool {
    s.contains('\\') || s.contains('/')
}

/// Extract just the filename from a path for parsing.
/// Preserves the full path for file index lookups.
fn strip_path(file_path: &str) -> &str {
    std::path::Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file_path)
}

/// Pull the video filename (up to and including its extension) out of a window
/// title / path. mpv/VLC put "<filename>.mkv - mpv" in the title, so the
/// substring ending at the last video extension is the filename we indexed.
fn extract_video_filename(text: &str) -> Option<String> {
    const EXTS: &[&str] = &["mkv", "mp4", "avi", "mov", "wmv", "flv", "webm", "m4v"];
    let lower = text.to_lowercase();
    let mut best_end: Option<usize> = None;
    for ext in EXTS {
        let pat = format!(".{ext}");
        if let Some(pos) = lower.rfind(&pat) {
            let end = pos + pat.len();
            best_end = Some(best_end.map_or(end, |b| b.max(end)));
        }
    }
    best_end.map(|end| text[..end].trim().to_string())
}

/// Build a "known file" recognition result for an already-mapped anime id.
async fn known_result(
    storage: &Storage,
    anime_id: i64,
    confidence: i32,
) -> Option<RecognitionResult> {
    let anime = storage.fetch_anime(anime_id).await.ok().flatten()?;
    let titles: serde_json::Value = serde_json::from_str(&anime.titles_json).unwrap_or_default();
    let title = titles["romaji"].as_str().unwrap_or("Unknown").to_string();
    Some(RecognitionResult {
        known_file: true,
        parsed: None,
        candidates: vec![MatchCandidate {
            anime_id,
            title,
            confidence: confidence as u8,
            match_source: "file_index".to_string(),
        }],
    })
}

pub async fn recognize_file(
    file_path: &str,
    window_title: Option<&str>,
    storage: &Storage,
) -> anyhow::Result<RecognitionResult> {
    // 1. Exact full-path index lookup — only for real paths. Players like mpv put
    //    the window title in `file_path`, which must never be used as a file key.
    if looks_like_path(file_path) {
        if let Some(existing) = storage.get_file_index(file_path).await? {
            if let Some(anime_id) = existing.anime_id {
                if let Some(res) = known_result(storage, anime_id, existing.confidence).await {
                    return Ok(res);
                }
            }
        }
    }

    // 2. Filename fallback — mpv/VLC only expose the filename in the window title,
    //    not the absolute path, so match the mapped file by its basename. This is
    //    what lets a played S04E13 file resolve to the *Season 4* entry rather than
    //    falling through to a title match on the base-season entry.
    let fname_source = window_title.unwrap_or(file_path);
    if let Some(fname) = extract_video_filename(fname_source) {
        if let Some(existing) = storage.get_file_index_by_filename(&fname).await? {
            if let Some(anime_id) = existing.anime_id {
                if let Some(res) = known_result(storage, anime_id, existing.confidence).await {
                    return Ok(res);
                }
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

        let confidence = score_titles_json(&parsed.cleaned_title, &anime.titles_json);

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

    // mpv and VLC expose only a window title, which the scanner reports as the
    // file path and the Now Playing Confirm button passes straight through here.
    // Indexing it would key a mapping to a string that is not a file: no path
    // lookup can ever hit it, and Up Next would offer it as an episode to play.
    // The automatic path guards the same way before writing (see session.rs).
    if looks_like_path(file_path) {
        state
            .storage
            .upsert_file_index(
                file_path,
                Some(anime_id),
                episode,
                100,
                MappingSource::Manual,
                now,
            )
            .await?;
    }

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
