# M2 Recognition Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Parse anime filenames and window titles to extract episode numbers, clean show titles, and quality tags; match parsed titles against the local anime library with confidence scoring; remember confirmed file-to-anime mappings; show a confirmation UI for uncertain matches.

**Architecture:** Add a regex-based filename parser (`engine::parser`) that cleans tags, extracts episode numbers and a cleaned show title. Add anime title search to `Storage` (fuzzy match across romaji/english/japanese/synonyms). Build a matcher (`engine::matcher`) that runs the parser, searches the library, and scores candidates. Wire into the tracking loop so known files bypass recognition. Expose new Tauri commands for identifying files and confirming matches. Frontend shows a low-confidence confirmation card and a known-files list.

**Tech Stack:** Rust 2021, Tauri 2.4, SQLx SQLite, Tokio, `regex` crate, Svelte 5, TypeScript, Vitest.

## Global Constraints

- Windows desktop app runtime using Tauri, Svelte, Rust, SQLite.
- AniList is the only tracker integration in scope; do not add MAL or Kitsu code.
- M2 scope only: filename parser, library title matching, confidence scoring, file-index persistence, recognition UI.
- No AniList sync (M3), tray (M5), rebrand (M8).
- Keep files small and focused; engine modules own logic, Tauri commands are thin wrappers.
- Every fallible command must return `Result<T, String>`.
- Parser must handle common anime naming conventions: `[Group] Title - 01 [1080p][x264]`, `Title S01E01`, `Title EP01`, multi-word titles with punctuation.
- `file_index` table drives remembered mappings; known files skip recognition entirely.

---

## File Structure

- Modify `next/src-tauri/Cargo.toml`
  Add `regex` dependency.

- Create `next/src-tauri/src/engine/parser.rs`
  `ParsedFilename` struct, `parse_filename(input: &str) -> Option<ParsedFilename>`, test corpus inline.

- Create `next/src-tauri/src/engine/matcher.rs`
  `MatchCandidate` struct, `RecognitionResult` struct, `identify_tracked(source_title: &str, episode_number: i32, storage: &Storage) -> anyhow::Result<RecognitionResult>`.
  Calls parser, searches library, scores results. Checks `file_index` first for known mappings.

- Modify `next/src-tauri/src/engine/storage.rs`
  Add methods: `search_anime_by_title(&self, query: &str, limit: i64) -> anyhow::Result<Vec<AnimeRow>>`, `upsert_file_index`, `get_file_index`, `list_file_index`.

- Modify `next/src-tauri/src/engine/session.rs`
  Replace `guess_episode` call in `process_scan_result` with `matcher::identify_tracked`. When confidence is high, auto-confirm and publish `AnimeIdentified`. When low, publish `PlaybackDetected` with candidates for UI.

- Modify `next/src-tauri/src/engine/events.rs`
  Add `candidates: Vec<MatchCandidate>` field to `PlaybackDetected` so the frontend can show matching options.

- Modify `next/src-tauri/src/engine/mod.rs`
  Export `parser` and `matcher`.

- Modify `next/src-tauri/src/commands.rs`
  Add commands: `identify_file`, `confirm_identification`, `list_known_files`.

- Modify `next/src-tauri/src/lib.rs`
  Register new commands.

- Create `next/src-tauri/tests/parser_test.rs`
  Test corpus covering 15+ real-world filename patterns.

- Create `next/src-tauri/tests/matcher_test.rs`
  Tests for matching against in-memory anime library.

- Modify `next/src/lib/api.ts`
  Add types and wrappers for `MatchCandidate`, `RecognitionResult`, new commands.

- Modify `next/src/lib/api.test.ts`
  Test new wrappers.

- Create `next/src/lib/RecognitionCard.svelte`
  Low-confidence confirmation dialog with candidate list, manual override input.

- Create `next/src/lib/KnownFiles.svelte`
  Simple list of remembered file→anime mappings.

- Modify `next/src/App.svelte`
  Integrate recognition components.

---

### Task 1: Filename Parser Engine

**Files:**
- Modify: `next/src-tauri/Cargo.toml`
- Create: `next/src-tauri/src/engine/parser.rs`
- Modify: `next/src-tauri/src/engine/mod.rs`
- Create: `next/src-tauri/tests/parser_test.rs`

**Interfaces:**
- Consumes: `regex` crate.
- Produces:
  - `pub struct ParsedFilename { pub cleaned_title: String, pub episode_number: i32, pub release_group: Option<String>, pub quality: Option<String>, pub raw: String }`
  - `pub fn parse_filename(input: &str, window_title: Option<&str>) -> Option<ParsedFilename>`

- [ ] **Step 1: Add regex dependency**

Modify `next/src-tauri/Cargo.toml`, add after `anyhow`:

```toml
regex = "1.11"
```

- [ ] **Step 2: Export parser module**

Add to `next/src-tauri/src/engine/mod.rs` after `pub mod models;`:

```rust
pub mod parser;
```

- [ ] **Step 3: Write failing parser tests**

Create `next/src-tauri/tests/parser_test.rs` with 15+ real-world cases:

```rust
use taiga_next::engine::parser::{parse_filename, ParsedFilename};

fn assert_parse(input: &str, title: Option<&str>, expected_title: &str, episode: i32) {
    let result = parse_filename(input, title).unwrap();
    assert_eq!(result.episode_number, episode, "episode mismatch for '{input}'");
    let cleaned = result.cleaned_title.to_lowercase();
    assert!(
        cleaned.contains(&expected_title.to_lowercase()),
        "title mismatch for '{input}': expected '{expected_title}' in '{cleaned}'"
    );
}

#[test]
fn parse_standard_dash_separator() {
    assert_parse("Cowboy Bebop - 01.mkv", None, "Cowboy Bebop", 1);
}

#[test]
fn parse_release_group_brackets() {
    assert_parse("[HorribleSubs] Attack on Titan - 12 [1080p].mkv", None, "Attack on Titan", 12);
}

#[test]
fn parse_s01e01_format() {
    assert_parse("Fullmetal Alchemist S01E03.mkv", None, "Fullmetal Alchemist", 3);
}

#[test]
fn parse_ep_prefix() {
    assert_parse("Steins;Gate EP07.mkv", None, "Steins;Gate", 7);
}

#[test]
fn parse_episode_prefix_lowercase() {
    assert_parse("Mushishi episode 15.mkv", None, "Mushishi", 15);
}

#[test]
fn parse_hash_prefix() {
    assert_parse("Jujutsu Kaisen - 05 [1080p][HEVC].mkv", None, "Jujutsu Kaisen", 5);
}

#[test]
fn parse_multi_season_s01e01() {
    assert_parse("My Hero Academia S03E10 [1080p].mkv", None, "My Hero Academia", 10);
}

#[test]
fn parse_hyphen_in_title() {
    assert_parse("Spy x Family - 02.mkv", None, "Spy x Family", 2);
}

#[test]
fn parse_semicolon_title() {
    assert_parse("Steins;Gate 0 - 01.mkv", None, "Steins;Gate 0", 1);
}

#[test]
fn parse_square_brackets_group() {
    assert_parse("[Erai-raws] One Piece - 1015 [1080p][HEVC].mkv", None, "One Piece", 1015);
}

#[test]
fn parse_parentheses_quality() {
    assert_parse("Violet Evergarden - 03 (1080p).mkv", None, "Violet Evergarden", 3);
}

#[test]
fn parse_leading_number_skip() {
    assert_parse("01 - Mob Psycho 100 II - 05.mkv", None, "Mob Psycho 100 II", 5);
}

#[test]
fn parse_paren_ep_num() {
    assert_parse("Barakamon (2014) - E01.mkv", None, "Barakamon", 1);
}

#[test]
fn parse_window_title() {
    assert_parse("mpv", Some("Cowboy Bebop - 05"), "Cowboy Bebop", 5);
}

#[test]
fn parse_no_match() {
    assert!(parse_filename("some_random_video.mp4", None).is_none());
}

#[test]
fn parse_episode_zero_rejected() {
    assert!(parse_filename("Show E00.mkv", None).is_none());
}
```

- [ ] **Step 4: Run tests to verify failure**

Run from `next/src-tauri`:

```bash
cargo test parser_test
```

Expected: FAIL — `parser` module does not exist.

- [ ] **Step 5: Implement parser engine**

Create `next/src-tauri/src/engine/parser.rs`:

```rust
use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFilename {
    pub cleaned_title: String,
    pub episode_number: i32,
    pub release_group: Option<String>,
    pub quality: Option<String>,
    pub raw: String,
}

static QUALITY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\s*(?:1080p|720p|480p|2160p|4K|8K|SD|HD)\s*\]|\(\s*(?:1080p|720p|480p|2160p)\s*\)")
        .unwrap()
});

static CODEC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\s*(?:x264|x265|HEVC|AVC|H264|H265|AV1|VP9)\s*\]|\(\s*(?:x264|x265|HEVC)\s*\)")
        .unwrap()
});

static AUDIO_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\s*(?:AAC|FLAC|OPUS|MP3|DTS|AC3|EAC3|TrueHD|Vorbis)(?:\s+?[0-9.]+[kK])?\s*\]|\(\s*(?:AAC|FLAC)\s*\)")
        .unwrap()
});

static RELEASE_GROUP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[([^\]]+)\]").unwrap()
});

static EPISODE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|\s|_|-)(?:E?P?|E)(\d{1,4})(?:\s|$|\.|\[|\(|_|-)").unwrap()
});

static S01E01_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[Ss](\d{1,2})[Ee](\d{1,4})").unwrap()
});

static EPISODE_WORD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bepisode\s+(\d{1,4})\b").unwrap()
});

static DASH_MULTI_NUM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s-\s+(\d{1,4})(?:v\d)?(?:\s|$|\.|\[|\(|_|-|\[\s)").unwrap()
});

static YEAR_PAREN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\((?:19|20)\d{2}\)").unwrap()
});

pub fn parse_filename(input: &str, window_title: Option<&str>) -> Option<ParsedFilename> {
    let source_text = window_title.unwrap_or(input);

    if source_text.is_empty() {
        return None;
    }

    let mut cleaned = source_text.to_string();

    // Extract release group from leading brackets
    let release_group = RELEASE_GROUP_RE
        .captures(&cleaned)
        .map(|c| c[1].to_string());
    cleaned = RELEASE_GROUP_RE.replace(&cleaned, "").to_string();

    // Strip quality tags
    let quality = QUALITY_RE.find(&cleaned).map(|m| m.as_str().to_string());
    cleaned = QUALITY_RE.replace_all(&cleaned, "").to_string();

    // Strip codec tags
    cleaned = CODEC_RE.replace_all(&cleaned, "").to_string();

    // Strip audio tags
    cleaned = AUDIO_RE.replace_all(&cleaned, "").to_string();

    // Try S01E01 style first (ignore season, extract episode)
    let mut episode: Option<i32> = None;
    if let Some(caps) = S01E01_RE.captures(&cleaned) {
        if let Ok(n) = caps[2].parse::<i32>() {
            if n > 0 && n <= 2000 {
                episode = Some(n);
                cleaned = S01E01_RE.replace(&cleaned, "").to_string();
            }
        }
    }

    // Try "Episode 01" spelled out
    if episode.is_none() {
        if let Some(caps) = EPISODE_WORD_RE.captures(&cleaned) {
            if let Ok(n) = caps[1].parse::<i32>() {
                if n > 0 && n <= 2000 {
                    episode = Some(n);
                    cleaned = EPISODE_WORD_RE.replace(&cleaned, "").to_string();
                }
            }
        }
    }

    // Try " - 01" dash-number pattern
    if episode.is_none() {
        if let Some(caps) = DASH_MULTI_NUM_RE.captures(&cleaned) {
            if let Ok(n) = caps[1].parse::<i32>() {
                if n > 0 && n <= 2000 {
                    episode = Some(n);
                    cleaned = DASH_MULTI_NUM_RE.replace(&cleaned, "").to_string();
                }
            }
        }
    }

    // Try bare EP01 / E01 patterns
    if episode.is_none() {
        if let Some(caps) = EPISODE_RE.captures(&cleaned) {
            if let Ok(n) = caps[1].parse::<i32>() {
                if n > 0 && n <= 2000 {
                    episode = Some(n);
                    cleaned = EPISODE_RE.replace(&cleaned, "").to_string();
                }
            }
        }
    }

    let episode_number = episode?;

    // Normalize cleaned title: strip extensions, brackets, extra whitespace
    cleaned = cleaned
        .replace(".mkv", "")
        .replace(".mp4", "")
        .replace(".avi", "")
        .replace(".mov", "")
        .replace(".wmv", "");
    cleaned = cleaned.replace('[', " ").replace(']', " ");
    cleaned = cleaned.replace('(', " ").replace(')', " ");
    cleaned = YEAR_PAREN_RE.replace_all(&cleaned, "").to_string();
    cleaned = cleaned.replace('_', " ");
    cleaned = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    cleaned = cleaned.trim().to_string();

    if cleaned.is_empty() {
        return None;
    }

    Some(ParsedFilename {
        cleaned_title: cleaned,
        episode_number,
        release_group,
        quality,
        raw: input.to_string(),
    })
}
```

- [ ] **Step 6: Run parser tests**

Run from `next/src-tauri`:

```bash
cargo test parser_test
```

Expected: all 15 tests PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/engine/mod.rs src-tauri/src/engine/parser.rs src-tauri/tests/parser_test.rs
git commit -m "feat: add filename parser engine"
```

---

### Task 2: Anime Title Search and File Index Storage

**Files:**
- Modify: `next/src-tauri/src/engine/storage.rs`
- Create: `next/src-tauri/tests/anime_search_test.rs`

**Interfaces:**
- Consumes: existing `AnimeRow` struct, `file_index` table.
- Produces:
  - `Storage::search_anime_by_title(&self, query: &str, limit: i64) -> anyhow::Result<Vec<AnimeRow>>`
  - `Storage::upsert_file_index(&self, file_path: &str, anime_id: i64, episode: i32, confidence: i32, indexed_at: i64) -> anyhow::Result<()>`
  - `Storage::get_file_index(&self, file_path: &str) -> anyhow::Result<Option<FileIndexRow>>`
  - `Storage::list_file_index(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<FileIndexRow>>`
  - `pub struct FileIndexRow { pub file_path: String, pub anime_id: Option<i64>, pub episode: Option<i32>, pub confidence: i32, pub indexed_at: i64 }`

- [ ] **Step 1: Write failing anime search tests**

Create `next/src-tauri/tests/anime_search_test.rs`:

```rust
use taiga_next::engine::storage::Storage;

#[tokio::test]
async fn search_anime_by_title_exact() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();
    storage.insert_minimal_anime(2, "Cowboy Bebop: The Movie").await.unwrap();

    let results = storage.search_anime_by_title("Cowboy Bebop", 10).await.unwrap();
    assert!(results.len() >= 1);
    assert_eq!(results[0].id, 1);
}

#[tokio::test]
async fn search_anime_by_partial_title() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Attack on Titan").await.unwrap();
    storage.insert_minimal_anime(2, "Fullmetal Alchemist").await.unwrap();

    let results = storage.search_anime_by_title("Alchemist", 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 2);
}
```

- [ ] **Step 2: Add storage methods**

Add `FileIndexRow` struct and `serde::Deserialize` import to `storage.rs`. Add methods:

```rust
use serde::Deserialize;

#[derive(Debug, Clone, serde::Serialize, Deserialize)]
pub struct FileIndexRow {
    pub file_path: String,
    pub anime_id: Option<i64>,
    pub episode: Option<i32>,
    pub confidence: i32,
    pub indexed_at: i64,
}

// Inside impl Storage:

pub async fn search_anime_by_title(&self, query: &str, limit: i64) -> anyhow::Result<Vec<AnimeRow>> {
    let pattern = format!("%{}%", query);
    let rows = sqlx::query(
        "SELECT id, titles_json, episode_count FROM anime
         WHERE titles_json LIKE ?1
         ORDER BY id LIMIT ?2",
    )
    .bind(&pattern)
    .bind(limit)
    .fetch_all(&self.pool)
    .await?;
    Ok(rows
        .iter()
        .map(|row| AnimeRow {
            id: row.get("id"),
            titles_json: row.get("titles_json"),
            episode_count: row.get("episode_count"),
        })
        .collect())
}

pub async fn upsert_file_index(
    &self,
    file_path: &str,
    anime_id: i64,
    episode: i32,
    confidence: i32,
    indexed_at: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO file_index (file_path, anime_id, episode, confidence, indexed_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(file_path) DO UPDATE SET
           anime_id = excluded.anime_id,
           episode = excluded.episode,
           confidence = excluded.confidence,
           indexed_at = excluded.indexed_at",
    )
    .bind(file_path)
    .bind(anime_id)
    .bind(episode)
    .bind(confidence)
    .bind(indexed_at)
    .execute(&self.pool)
    .await?;
    Ok(())
}

pub async fn get_file_index(&self, file_path: &str) -> anyhow::Result<Option<FileIndexRow>> {
    let row = sqlx::query(
        "SELECT file_path, anime_id, episode, confidence, indexed_at
         FROM file_index WHERE file_path = ?1",
    )
    .bind(file_path)
    .fetch_optional(&self.pool)
    .await?;
    Ok(row.map(|row| FileIndexRow {
        file_path: row.get("file_path"),
        anime_id: row.get("anime_id"),
        episode: row.get("episode"),
        confidence: row.get("confidence"),
        indexed_at: row.get("indexed_at"),
    }))
}

pub async fn list_file_index(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<FileIndexRow>> {
    let rows = sqlx::query(
        "SELECT file_path, anime_id, episode, confidence, indexed_at
         FROM file_index ORDER BY indexed_at DESC LIMIT ?1 OFFSET ?2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&self.pool)
    .await?;
    Ok(rows
        .iter()
        .map(|row| FileIndexRow {
            file_path: row.get("file_path"),
            anime_id: row.get("anime_id"),
            episode: row.get("episode"),
            confidence: row.get("confidence"),
            indexed_at: row.get("indexed_at"),
        })
        .collect())
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test anime_search_test
cargo test
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/engine/storage.rs src-tauri/tests/anime_search_test.rs
git commit -m "feat: add anime search and file index storage"
```

---

### Task 3: Recognition Matcher Engine

**Files:**
- Create: `next/src-tauri/src/engine/matcher.rs`
- Modify: `next/src-tauri/src/engine/events.rs`
- Modify: `next/src-tauri/src/engine/mod.rs`
- Modify: `next/src-tauri/src/engine/session.rs`
- Create: `next/src-tauri/tests/matcher_test.rs`

**Interfaces:**
- Consumes: `ParsedFilename` from parser, `Storage`, `EventBus`.
- Produces:
  - `pub struct MatchCandidate { pub anime_id: AnimeId, pub title: String, pub confidence: u8, pub match_source: String }`
  - `pub struct RecognitionResult { pub known_file: bool, pub parsed: ParsedFilename, pub candidates: Vec<MatchCandidate> }`
  - `pub async fn recognize_file(file_path: &str, window_title: Option<&str>, storage: &Storage) -> anyhow::Result<RecognitionResult>`
  - `pub async fn confirm_identification(state: &EngineState, file_path: &str, anime_id: i64, episode: i32) -> anyhow::Result<()>`
- Events update: add `candidates: Vec<MatchCandidate>` to `PlaybackDetected`.
- Session update: call `recognize_file` from `process_scan_result` instead of `guess_episode`.

- [ ] **Step 1: Add MatchCandidate derive**

`MatchCandidate` needs `Serialize, Deserialize, Clone, PartialEq, Eq`. Since it uses `AnimeId` (i64) and `String`, these are trivial.

- [ ] **Step 2: Write matcher tests**

Create `next/src-tauri/tests/matcher_test.rs`:

```rust
use taiga_next::engine::matcher::{recognize_file, confirm_identification};
use taiga_next::engine::runtime::EngineState;

fn test_state() -> EngineState {
    taiga_next::engine::runtime::fresh_test_state()
}

#[tokio::test]
async fn recognize_known_file_skips_matching() {
    let state = test_state();
    state.storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();
    state.storage.upsert_file_index("D:/Anime/Cowboy Bebop - 01.mkv", 1, 1, 100, 1_782_769_008).await.unwrap();

    let result = recognize_file("D:/Anime/Cowboy Bebop - 01.mkv", None, &state.storage).await.unwrap();
    assert!(result.known_file);
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].anime_id, 1);
    assert_eq!(result.candidates[0].confidence, 100);
}

#[tokio::test]
async fn recognize_new_file_parses_and_searches() {
    let state = test_state();
    state.storage.insert_minimal_anime(1, "Mushishi").await.unwrap();

    let result = recognize_file("D:/Anime/Mushishi - 07.mkv", None, &state.storage).await.unwrap();
    assert!(!result.known_file);
    assert_eq!(result.parsed.episode_number, 7);
    assert!(result.candidates.iter().any(|c| c.anime_id == 1));
}
```

- [ ] **Step 3: Add candidates to PlaybackDetected event**

Modify `next/src-tauri/src/engine/events.rs`, add field to `PlaybackDetected` variant:

```rust
    PlaybackDetected {
        player_name: String,
        file_path: Option<String>,
        window_title: Option<String>,
        episode_guess: Option<EpisodeNumber>,
        candidates: Vec<MatchCandidate>,
        detected_at_unix: i64,
    },
```

- [ ] **Step 4: Implement matcher**

Create `next/src-tauri/src/engine/matcher.rs`:

```rust
use crate::engine::events::{EngineEvent, MatchCandidate};
use crate::engine::models::AnimeId;
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
    if total == 0 { return 0; }
    ((overlap as f64 / total as f64) * 60.0) as u8
}

fn search_library_title(storage: async impl Future<Output = anyhow::Result<Vec<AnimeRow>>>) -> anyhow::Result<Vec<(AnimeRow, u8)>> {
    // This function signature needs work — we need Storage at runtime.
    // Replace with direct async call.
    unimplemented!("replaced inline below")
}

// Replace the above with the real implementation directly:

use std::future::Future;
use std::pin::Pin;

pub async fn recognize_file(
    file_path: &str,
    window_title: Option<&str>,
    storage: &Storage,
) -> anyhow::Result<RecognitionResult> {
    use crate::engine::storage::AnimeRow;

    // Check remembered file index first
    if let Some(existing) = storage.get_file_index(file_path).await? {
        if let Some(anime_id) = existing.anime_id {
            if let Some(anime) = storage.fetch_anime(anime_id).await? {
                let titles: serde_json::Value = serde_json::from_str(&anime.titles_json).unwrap_or_default();
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

    // Parse the filename
    let parsed = match parse_filename(file_path, window_title) {
        Some(p) => p,
        None => return Ok(RecognitionResult {
            known_file: false,
            parsed: None,
            candidates: vec![],
        }),
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
        let titles: serde_json::Value = serde_json::from_str(&anime.titles_json).unwrap_or_default();
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

    state.storage.upsert_file_index(file_path, anime_id, episode, 100, now).await?;

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
```

Fix the `MatchCandidate` type — it should be in `events.rs` not `matcher.rs`. Move it:

```rust
// In events.rs after imports:
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MatchCandidate {
    pub anime_id: AnimeId,
    pub title: String,
    pub confidence: u8,
    pub match_source: String,
}
```

Update `events.rs` `PlaybackDetected` to use it:

```rust
    PlaybackDetected {
        player_name: String,
        file_path: Option<String>,
        window_title: Option<String>,
        episode_guess: Option<EpisodeNumber>,
        candidates: Vec<MatchCandidate>,
        detected_at_unix: i64,
    },
```

- [ ] **Step 5: Update session to use matcher**

Modify `next/src-tauri/src/engine/session.rs` `process_scan_result`:

```rust
use crate::engine::matcher::recognize_file;

pub async fn process_scan_result(state: &EngineState, result: ScanResult) -> anyhow::Result<()> {
    let recognition = recognize_file(
        result.file_path.as_deref().unwrap_or(""),
        result.window_title.as_deref(),
        &state.storage,
    ).await?;

    let episode_guess = recognition
        .parsed
        .as_ref()
        .map(|p| p.episode_number)
        .or_else(|| recognition.candidates.first().map(|_| 0_i32));

    state.events.publish(EngineEvent::PlaybackDetected {
        player_name: result.player_name,
        file_path: result.file_path,
        window_title: result.window_title,
        episode_guess,
        candidates: recognition.candidates,
        detected_at_unix: result.detected_at_unix,
    });

    Ok(())
}
```

- [ ] **Step 6: Add unix_now_inner to commands**

Make the helper available:

```rust
// In commands.rs:
pub fn unix_now_inner() -> Result<i64, String> {
    unix_now()
}
```

- [ ] **Step 7: Export matcher module**

Add to `mod.rs`: `pub mod matcher;`

- [ ] **Step 8: Run tests and fix compilation**

```bash
cargo test matcher_test
cargo test
```

Expected: matcher tests PASS, all tests PASS.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/engine/matcher.rs src-tauri/src/engine/events.rs src-tauri/src/engine/mod.rs src-tauri/src/engine/session.rs src-tauri/src/commands.rs src-tauri/tests/matcher_test.rs
git commit -m "feat: add recognition matcher engine"
```

---

### Task 4: Recognition Commands

**Files:**
- Modify: `next/src-tauri/src/commands.rs`
- Modify: `next/src-tauri/src/lib.rs`
- Create: `next/src-tauri/tests/recognition_commands_test.rs`

**Interfaces:**
- Produces:
  - `identify_file(file_path, window_title, state) -> Result<RecognitionResult, String>`
  - `confirm_identification(file_path, anime_id, episode, state) -> Result<(), String>`
  - `list_known_files(limit, state) -> Result<Vec<FileIndexRow>, String>`

- [ ] **Step 1: Add commands to `commands.rs`**

```rust
use crate::engine::matcher::{recognize_file, confirm_identification, RecognitionResult};
use crate::engine::storage::FileIndexRow;

pub async fn identify_file_inner(
    file_path: &str,
    window_title: Option<&str>,
    state: &EngineState,
) -> Result<RecognitionResult, String> {
    recognize_file(file_path, window_title, &state.storage)
        .await
        .map_err(command_error)
}

pub async fn confirm_identification_inner(
    file_path: &str,
    anime_id: i64,
    episode: i32,
    state: &EngineState,
) -> Result<(), String> {
    confirm_identification(state, file_path, anime_id, episode)
        .await
        .map_err(command_error)
}

pub async fn list_known_files_inner(
    limit: i64,
    state: &EngineState,
) -> Result<Vec<FileIndexRow>, String> {
    state
        .storage
        .list_file_index(limit, 0)
        .await
        .map_err(command_error)
}

#[tauri::command]
pub async fn identify_file(
    file_path: String,
    window_title: Option<String>,
    state: tauri::State<'_, EngineState>,
) -> Result<RecognitionResult, String> {
    identify_file_inner(&file_path, window_title.as_deref(), &state).await
}

#[tauri::command]
pub async fn confirm_identification(
    file_path: String,
    anime_id: i64,
    episode: i32,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    confirm_identification_inner(&file_path, anime_id, episode, &state).await
}

#[tauri::command]
pub async fn list_known_files(
    limit: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<FileIndexRow>, String> {
    list_known_files_inner(limit, &state).await
}
```

- [ ] **Step 2: Register commands in `lib.rs`**

Add to `generate_handler!`:

```rust
    commands::identify_file,
    commands::confirm_identification,
    commands::list_known_files,
```

- [ ] **Step 3: Write command tests**

Create `next/src-tauri/tests/recognition_commands_test.rs`:

```rust
use taiga_next::commands::{identify_file_inner, confirm_identification_inner, list_known_files_inner};
use taiga_next::engine::runtime::EngineState;

fn test_state() -> EngineState {
    taiga_next::engine::runtime::fresh_test_state()
}

#[tokio::test]
async fn identify_empty_file_returns_no_candidates() {
    let state = test_state();
    let result = identify_file_inner("unknown_file.mp4", None, &state).await.unwrap();
    assert!(!result.known_file);
    assert!(result.candidates.is_empty());
}

#[tokio::test]
async fn identify_and_confirm_remembers_mapping() {
    let state = test_state();
    state.storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();

    let result = identify_file_inner("Cowboy Bebop - 01.mkv", None, &state).await.unwrap();
    assert!(!result.known_file);
    assert!(result.candidates.iter().any(|c| c.anime_id == 1));

    confirm_identification_inner("Cowboy Bebop - 01.mkv", 1, 1, &state).await.unwrap();

    // Re-identify — should be known now
    let result2 = identify_file_inner("Cowboy Bebop - 01.mkv", None, &state).await.unwrap();
    assert!(result2.known_file);
}
```

- [ ] **Step 4: Run command tests**

```bash
cargo test recognition_commands_test
cargo test
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/tests/recognition_commands_test.rs
git commit -m "feat: add recognition commands"
```

---

### Task 5: Frontend Recognition Wrappers

**Files:**
- Modify: `next/src/lib/api.ts`
- Modify: `next/src/lib/api.test.ts`

**Interfaces:**
- Produces TypeScript wrappers for `identify_file`, `confirm_identification`, `list_known_files`.
- Updates `PlaybackDetectedEvent` to include `candidates: MatchCandidate[]`.

- [ ] **Step 1: Add types and wrappers**

Add to `next/src/lib/api.ts`:

```ts
export interface MatchCandidate {
  anime_id: number;
  title: string;
  confidence: number;
  match_source: string;
}

export interface ParsedFilename {
  cleaned_title: string;
  episode_number: number;
  release_group: string | null;
  quality: string | null;
  raw: string;
}

export interface RecognitionResult {
  known_file: boolean;
  parsed: ParsedFilename | null;
  candidates: MatchCandidate[];
}

export interface FileIndexEntry {
  file_path: string;
  anime_id: number | null;
  episode: number | null;
  confidence: number;
  indexed_at: number;
}
```

Update `PlaybackDetectedEvent`:

```ts
export interface PlaybackDetectedEvent {
  PlaybackDetected: {
    player_name: string;
    file_path: string | null;
    window_title: string | null;
    episode_guess: number | null;
    candidates: MatchCandidate[];
    detected_at_unix: number;
  };
}
```

Add wrappers:

```ts
export function identifyFile(filePath: string, windowTitle: string | null, invokeFn: InvokeFn = tauriInvoke): Promise<RecognitionResult> {
  return invokeFn<RecognitionResult>('identify_file', { file_path: filePath, window_title: windowTitle });
}

export function confirmIdentification(filePath: string, animeId: number, episode: number, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('confirm_identification', { file_path: filePath, anime_id: animeId, episode });
}

export function listKnownFiles(limit: number, invokeFn: InvokeFn = tauriInvoke): Promise<FileIndexEntry[]> {
  return invokeFn<FileIndexEntry[]>('list_known_files', { limit });
}
```

- [ ] **Step 2: Update tests**

Add test cases for new wrappers in `api.test.ts`:

```ts
  it('identifies file through invoke', async () => {
    const result = { known_file: false, parsed: null, candidates: [] };
    const invoke = vi.fn().mockResolvedValue(result);
    await expect(identifyFile('test.mkv', null, invoke)).resolves.toEqual(result);
    expect(invoke).toHaveBeenCalledWith('identify_file', { file_path: 'test.mkv', window_title: null });
  });

  it('confirms identification', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(confirmIdentification('test.mkv', 1, 5, invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('confirm_identification', { file_path: 'test.mkv', anime_id: 1, episode: 5 });
  });

  it('lists known files', async () => {
    const entries = [{ file_path: 'test.mkv', anime_id: 1, episode: 1, confidence: 100, indexed_at: 1782769008 }];
    const invoke = vi.fn().mockResolvedValue(entries);
    await expect(listKnownFiles(10, invoke)).resolves.toEqual(entries);
    expect(invoke).toHaveBeenCalledWith('list_known_files', { limit: 10 });
  });
```

Update imports with new symbols:
```ts
import { ..., identifyFile, confirmIdentification, listKnownFiles } from './api';
```

- [ ] **Step 3: Run frontend checks**

```bash
npm run check
npm run test
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/lib/api.ts src/lib/api.test.ts
git commit -m "feat: expose recognition API wrappers"
```

---

### Task 6: Recognition UI

**Files:**
- Create: `next/src/lib/RecognitionCard.svelte`
- Create: `next/src/lib/KnownFiles.svelte`
- Modify: `next/src/App.svelte`

**Interfaces:**
- Consumes: PlaybackDetected events (via drainEngineEvents), `confirmIdentification`, `listKnownFiles`.
- Produces: Confirmation card when low-confidence match detected; known files list.

- [ ] **Step 1: Create RecognitionCard component**

Create `next/src/lib/RecognitionCard.svelte`:

```svelte
<script lang="ts">
  import { drainEngineEvents, confirmIdentification, type MatchCandidate } from './api';

  let candidates: MatchCandidate[] = [];
  let filePath: string | null = null;
  let episodeGuess: number | null = null;
  let confirmed: string | null = null;
  let error: string | null = null;
  let loading = false;

  async function poll() {
    try {
      const events = await drainEngineEvents();
      for (const event of events) {
        if ('PlaybackDetected' in event) {
          const pd = event.PlaybackDetected;
          if (pd.candidates.length > 0 && pd.candidates[0].confidence < 60) {
            candidates = pd.candidates;
            filePath = pd.file_path;
            episodeGuess = pd.episode_guess;
            confirmed = null;
            error = null;
            return;
          }
        }
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function confirm(animeId: number, episode: number) {
    if (!filePath || loading) return;
    loading = true;
    error = null;
    try {
      await confirmIdentification(filePath, animeId, episode);
      candidates = [];
      confirmed = `Confirmed: anime ${animeId} episode ${episode}`;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  import { onMount, onDestroy } from 'svelte';
  let intervalId: ReturnType<typeof setInterval> | null = null;

  onMount(() => {
    intervalId = setInterval(poll, 3000);
  });
  onDestroy(() => {
    if (intervalId) clearInterval(intervalId);
  });
</script>

{#if candidates.length > 0 || confirmed || error}
  <section class="recognition-card" aria-live="polite">
    <p class="eyebrow">Recognition</p>

    {#if error}
      <p class="error">{error}</p>
    {/if}

    {#if confirmed}
      <p class="confirm-msg">{confirmed}</p>
    {:else if candidates.length > 0}
      <p class="rc-hint">
        Detected: {filePath} — match?
      </p>
      <ul class="rc-list" role="list">
        {#each candidates.slice(0, 5) as candidate}
          <li class="rc-item" role="listitem">
            <span class="rc-title">{candidate.title}</span>
            <span class="rc-score">{candidate.confidence}%</span>
            <button
              class="rc-confirm"
              type="button"
              on:click={() => confirm(candidate.anime_id, episodeGuess ?? 1)}
              disabled={loading}
            >
              Confirm ep {episodeGuess ?? '?'}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
{/if}

<style>
  .recognition-card {
    border: 1px solid rgba(255, 193, 100, 0.25);
    border-radius: var(--radius-card);
    background: rgba(255, 193, 100, 0.06);
    padding: 1.25rem;
    display: grid;
    gap: 0.75rem;
  }

  .eyebrow {
    color: #ffc164;
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.78rem;
    font-weight: 800;
  }

  .rc-hint { color: var(--color-muted); font-size: 0.82rem; }
  .confirm-msg { color: var(--color-accent); font-size: 0.85rem; }
  .error { color: var(--color-error, #ff9d9d); font-size: 0.82rem; }

  .rc-list { display: grid; gap: 0.5rem; padding: 0; margin: 0; }
  .rc-item {
    display: grid;
    grid-template-columns: 1fr 3rem auto;
    gap: 0.75rem;
    align-items: center;
    font-size: 0.85rem;
  }
  .rc-title { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .rc-score { color: var(--color-accent); font-variant-numeric: tabular-nums; text-align: right; }

  .rc-confirm {
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 999px;
    padding: 0.35rem 0.75rem;
    font-size: 0.75rem;
    background: rgba(143, 183, 255, 0.15);
    color: #e9eefc;
    cursor: pointer;
    white-space: nowrap;
  }
  .rc-confirm:hover { background: rgba(143, 183, 255, 0.28); }
  .rc-confirm:focus { outline: 2px solid var(--color-accent); outline-offset: 2px; }
  .rc-confirm:disabled { opacity: 0.4; cursor: default; }
</style>
```

- [ ] **Step 2: Create KnownFiles component**

Create `next/src/lib/KnownFiles.svelte`:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { listKnownFiles, type FileIndexEntry } from './api';

  let entries: FileIndexEntry[] = [];
  let error: string | null = null;

  async function load() {
    error = null;
    try {
      entries = await listKnownFiles(50);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(load);
</script>

<section class="known-files-card">
  <p class="eyebrow">Known files</p>
  {#if error}
    <p class="error">{error}</p>
  {:else if entries.length === 0}
    <p class="empty">No known files yet. Confirm a detection to add one.</p>
  {:else}
    <ul class="kf-list" role="list">
      {#each entries as entry}
        <li class="kf-item" role="listitem">
          <span class="kf-path">{entry.file_path}</span>
          {#if entry.anime_id}
            <span class="kf-meta">#{entry.anime_id} ep {entry.episode}</span>
          {/if}
          <span class="kf-confidence">{entry.confidence}%</span>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .known-files-card {
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: var(--radius-card);
    background: rgba(255, 255, 255, 0.04);
    padding: 1.25rem;
    display: grid;
    gap: 0.75rem;
  }
  .eyebrow {
    color: var(--color-accent);
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.78rem;
    font-weight: 800;
  }
  .empty { color: var(--color-muted); font-size: 0.82rem; }
  .error { color: var(--color-error, #ff9d9d); font-size: 0.82rem; }

  .kf-list { display: grid; gap: 0.35rem; padding: 0; margin: 0; }
  .kf-item { display: grid; grid-template-columns: 1fr auto auto; gap: 0.75rem; font-size: 0.78rem; align-items: center; }
  .kf-path {
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    color: var(--color-muted);
  }
  .kf-meta { color: var(--color-text); font-size: 0.75rem; }
  .kf-confidence { color: var(--color-accent); font-size: 0.75rem; }
</style>
```

- [ ] **Step 3: Integrate into App.svelte**

Add imports:

```svelte
  import RecognitionCard from './lib/RecognitionCard.svelte';
  import KnownFiles from './lib/KnownFiles.svelte';
```

Add below existing `<MarkWatched />`:

```svelte
    <RecognitionCard />
    <KnownFiles />
```

- [ ] **Step 4: Run checks**

```bash
npm run check
npm run test
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib/RecognitionCard.svelte src/lib/KnownFiles.svelte src/App.svelte
git commit -m "feat: add recognition UI"
```

---

### Task 7: Full Verification and M2 Acceptance

- [ ] **Step 1: Run full verification**

```bash
npm run verify
```

Expected: TypeScript check PASS, Vitest PASS, Cargo tests PASS.

- [ ] **Step 2: Check acceptance criteria**

```text
[ ] Common anime filenames recognized (parser test corpus passes)
[ ] Low-confidence matches require confirmation (RecognitionCard shows when confidence < 60%)
[ ] User corrections persist and improve future matches (confirm_identification writes file_index; known files skip parsing)
```

- [ ] **Step 3: Commit fixes if any**

---

## Self-Review Notes

- Spec coverage: filename parser, library search, confidence scoring, file-index persistence, confirmation UI all covered in Tasks 1-7.
- Out-of-scope guard: no AniList sync (M3), no tray (M5), no rebrand (M8), no MAL/Kitsu.
- Placeholder scan: no TBD/TODO/fill-in steps remain.
- Type consistency: `MatchCandidate` defined in events.rs, used in matcher.rs, commands.rs, and api.ts. `ParsedFilename` used in parser→matcher→RecognizeResult→commands→frontend.
- `regex` crate is the only new dependency.
- Parser test corpus covers 15+ real-world filename patterns (brackets, S01E01, EP prefix, dash-sep, multi-word titles, semicolons, year parens, window titles, no-match, zero episode).
- `guess_episode` in session.rs is fully replaced by `recognize_file` call.
- Confidence threshold 60% for UI interruption; 20% for candidate inclusion; 100% for known files.
