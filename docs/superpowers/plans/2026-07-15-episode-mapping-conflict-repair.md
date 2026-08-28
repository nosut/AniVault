# Episode Mapping Conflict Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect already-confident episode files mapped to the wrong anime, require confirmation before repairing them, preserve explicit manual mappings, and make File Management show the real mapping.

**Architecture:** Add persisted mapping provenance to `file_index`, carry it through every write path, and have targeted rescans report plausible direct-sibling conflicts without mutating them. A separate Tauri command recomputes those conflicts server-side and transactionally repairs only automatic, inherited, or legacy rows after user confirmation; the frontend presents the candidates and keeps manual conflicts protected.

**Tech Stack:** Rust 2021, Tokio, SQLx/SQLite migrations, Tauri 2 commands, Svelte 5, TypeScript, Vitest, npm/Vite, NSIS.

## Global Constraints

- Preserve ordinary full, watcher, and targeted rescan as non-destructive for existing mappings.
- Mapping sources are exactly `automatic`, `inherited`, `manual`, and `legacy`.
- Existing rows migrate to `legacy`; unknown database source strings decode as `legacy`.
- Repair only non-ignored direct siblings with a target-title score at or above `MATCH_THRESHOLD`.
- Never repair a row whose current source is `manual`; Map Folder remains the explicit override.
- The repair request sends only `animeId`; the backend must recompute candidates before writing.
- Do not add frontend dependencies; test extracted pure UI helpers with the existing Vitest setup.
- Preserve unrelated worktree changes and stage only files named by each task.
- Release version is exactly `1.0.2` in npm, Cargo, and Tauri metadata.
- Build an x64 NSIS installer and report its byte size and SHA-256 checksum.

---

## File Map

- `next/src-tauri/migrations/0007_file_index_mapping_source.sql`: add and backfill mapping provenance.
- `next/src-tauri/src/engine/storage.rs`: source enum, source-aware reads/writes, known-file title projection, and guarded transactional repair updates.
- `next/src-tauri/src/engine/library_scanner.rs`: structured match result, shared query scoring, conflict detection, and rescan conflict reports.
- `next/src-tauri/src/engine/matcher.rs`: mark user confirmation as manual.
- `next/src-tauri/src/engine/session.rs`: mark playback recognition as automatic.
- `next/src-tauri/src/commands.rs`: propagate sources, expose enriched known files, and implement repair command internals/wrapper.
- `next/src-tauri/src/lib.rs`: register the new Tauri command.
- `next/src-tauri/tests/storage_test.rs`: migration and storage provenance tests.
- `next/src-tauri/tests/mapping_sweep_test.rs`: structured match/source expectations.
- `next/src-tauri/tests/mapping_repair_test.rs`: exact rescan and confirmed-repair regression suite.
- `next/src-tauri/tests/library_scan_prune_test.rs`, `folder_inheritance_test.rs`, `matcher_test.rs`: update fixtures for explicit source arguments.
- `next/src/lib/api.ts`: frontend source, conflict, known-file, and repair report types plus wrapper.
- `next/src/lib/api.test.ts`: Tauri payload/response contract tests.
- `next/src/lib/fileMappingUi.ts`: pure display labels and conflict partitioning.
- `next/src/lib/fileMappingUi.test.ts`: frontend behavior tests without a DOM dependency.
- `next/src/lib/FileManager.svelte`: show actual mapped title and source.
- `next/src/lib/DetailView.svelte`: rescan warning, protected state, confirmation, repair, and visible errors.
- `next/package.json`, `next/package-lock.json`, `next/src-tauri/Cargo.toml`, `next/src-tauri/Cargo.lock`, `next/src-tauri/tauri.conf.json`: version `1.0.2`.

---

### Task 1: Persist and Propagate Mapping Provenance

**Files:**
- Create: `next/src-tauri/migrations/0007_file_index_mapping_source.sql`
- Modify: `next/src-tauri/src/engine/storage.rs`
- Modify: `next/src-tauri/src/engine/library_scanner.rs`
- Modify: `next/src-tauri/src/engine/matcher.rs`
- Modify: `next/src-tauri/src/engine/session.rs`
- Modify: `next/src-tauri/src/commands.rs`
- Modify: Rust tests containing `upsert_file_index` or `upsert_file_mappings` calls
- Test: `next/src-tauri/tests/storage_test.rs`
- Test: `next/src-tauri/tests/mapping_sweep_test.rs`

**Interfaces:**
- Produces: `MappingSource::{Automatic, Inherited, Manual, Legacy}` with `as_db()`, `from_db()`, and `is_repairable()`.
- Produces: `FileMatch { anime_id, confidence, episode, mapping_source }`.
- Produces: `Storage::upsert_file_index(file_path, anime_id, episode, confidence, mapping_source, indexed_at)`.
- Produces: `Storage::upsert_file_mappings(mappings, mapping_source, indexed_at)`.
- Consumes: existing SQLx migrator and scanner title matching.

- [ ] **Step 1: Write failing migration and enum tests**

Add to `next/src-tauri/tests/storage_test.rs`:

```rust
use anivault_core::engine::storage::{MappingSource, Storage};

#[tokio::test]
async fn mapping_source_migration_backfills_legacy_rows() {
    use sqlx::sqlite::SqlitePoolOptions;

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE file_index (
            file_path TEXT PRIMARY KEY,
            anime_id INTEGER,
            episode INTEGER,
            confidence INTEGER NOT NULL DEFAULT 0,
            indexed_at INTEGER NOT NULL,
            ignored INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO file_index
         (file_path, anime_id, episode, confidence, indexed_at, ignored)
         VALUES ('D:/Anime/Show - 01.mkv', 7, 1, 100, 1, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(include_str!("../migrations/0007_file_index_mapping_source.sql"))
        .execute(&pool)
        .await
        .unwrap();

    let source: String = sqlx::query_scalar(
        "SELECT mapping_source FROM file_index WHERE file_path = 'D:/Anime/Show - 01.mkv'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(source, "legacy");
}

#[test]
fn mapping_source_unknown_values_fail_closed_as_legacy() {
    assert_eq!(MappingSource::from_db("manual"), MappingSource::Manual);
    assert_eq!(MappingSource::from_db("unexpected"), MappingSource::Legacy);
    assert!(!MappingSource::Manual.is_repairable());
    assert!(MappingSource::Automatic.is_repairable());
    assert!(MappingSource::Inherited.is_repairable());
    assert!(MappingSource::Legacy.is_repairable());
}
```

Update the existing Skeleton Knight assertion in `mapping_sweep_test.rs` to use fields:

```rust
let matched = anivault_core::engine::library_scanner::match_file(&state.storage, &ep2)
    .await
    .unwrap();
assert_eq!(matched.anime_id, Some(185542));
assert_eq!(matched.confidence, 85);
assert_eq!(matched.episode, Some(2));
assert_eq!(matched.mapping_source, MappingSource::Inherited);
```

Add this policy regression to `mapping_sweep_test.rs`:

```rust
#[tokio::test]
async fn rematch_unmapped_does_not_rewrite_an_existing_automatic_mapping() {
    let state = fresh_test_state().await;
    state.storage.insert_minimal_anime(1, "Wrong Existing Anime").await.unwrap();
    state.storage.insert_minimal_anime(2, "Target Show").await.unwrap();
    let path = "D:/Anime/Target Show - 01.mkv";
    state.storage.upsert_file_index(path, Some(1), 1, 50, MappingSource::Automatic, now()).await.unwrap();

    let changed = anivault_core::commands::rematch_unmapped_files_inner(&state).await.unwrap();

    assert_eq!(changed, 0);
    assert_eq!(state.storage.get_file_index(path).await.unwrap().unwrap().anime_id, Some(1));
}
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run from `next/src-tauri`:

```powershell
cargo test --test storage_test mapping_source -- --nocapture
cargo test --test mapping_sweep_test scanner_prefers_unanimous_folder_mapping_over_base_title_match -- --nocapture
```

Expected: compilation fails because migration 0007, `MappingSource`, and structured `FileMatch` do not exist.

- [ ] **Step 3: Add migration and source enum**

Create `next/src-tauri/migrations/0007_file_index_mapping_source.sql`:

```sql
-- Existing mappings predate provenance tracking. Treat them as legacy so they
-- require explicit repair confirmation and are never mistaken for new manual rows.
ALTER TABLE file_index
ADD COLUMN mapping_source TEXT NOT NULL DEFAULT 'legacy';
```

Add near `FileIndexRow` in `storage.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingSource {
    Automatic,
    Inherited,
    Manual,
    Legacy,
}

impl MappingSource {
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Inherited => "inherited",
            Self::Manual => "manual",
            Self::Legacy => "legacy",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "automatic" => Self::Automatic,
            "inherited" => Self::Inherited,
            "manual" => Self::Manual,
            "legacy" => Self::Legacy,
            _ => Self::Legacy,
        }
    }

    pub fn is_repairable(self) -> bool {
        !matches!(self, Self::Manual)
    }
}
```

Add `pub mapping_source: MappingSource` to `FileIndexRow`. Update every SELECT that builds
`FileIndexRow` to select `mapping_source` and decode it with:

```rust
mapping_source: MappingSource::from_db(&row.get::<String, _>("mapping_source")),
```

- [ ] **Step 4: Make storage writes require an explicit source**

Change the storage signatures and SQL:

```rust
pub async fn upsert_file_index(
    &self,
    file_path: &str,
    anime_id: Option<i64>,
    episode: i32,
    confidence: i32,
    mapping_source: MappingSource,
    indexed_at: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO file_index
         (file_path, anime_id, episode, confidence, mapping_source, indexed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(file_path) DO UPDATE SET
           anime_id = excluded.anime_id,
           episode = excluded.episode,
           confidence = excluded.confidence,
           mapping_source = excluded.mapping_source,
           indexed_at = excluded.indexed_at",
    )
    .bind(file_path)
    .bind(anime_id)
    .bind(episode)
    .bind(confidence)
    .bind(mapping_source.as_db())
    .bind(indexed_at)
    .execute(&self.pool)
    .await?;
    Ok(())
}
```

Add `mapping_source: MappingSource` to `upsert_file_mappings`, bind it in each insert, and
update `mapping_source = excluded.mapping_source` on conflict. Keep confidence 100 for
that batch helper.

- [ ] **Step 5: Replace the match tuple with a source-bearing struct**

In `library_scanner.rs`:

```rust
use crate::engine::storage::{MappingSource, Storage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMatch {
    pub anime_id: Option<i64>,
    pub confidence: i32,
    pub episode: Option<i32>,
    pub mapping_source: MappingSource,
}
```

Return `MappingSource::Inherited` for folder consensus and `MappingSource::Automatic`
for title matches or unmatched results:

```rust
let (anime_id, confidence, mapping_source) = if let Some(id) =
    folder_anime.filter(|_| best_score < MATCH_THRESHOLD || folder_score >= MATCH_THRESHOLD)
{
    (Some(id), INHERITED_CONFIDENCE, MappingSource::Inherited)
} else if best_score >= MATCH_THRESHOLD {
    (best_id, best_score as i32, MappingSource::Automatic)
} else {
    (None, 0, MappingSource::Automatic)
};

Ok(FileMatch { anime_id, confidence, episode, mapping_source })
```

- [ ] **Step 6: Update every production write path with the correct source**

Use this exact mapping while replacing tuple destructuring with `FileMatch` fields:

| Write path | Source |
|---|---|
| Library scanner and watcher title match | `matched.mapping_source` |
| Folder-inheritance sweep | `matched.mapping_source` |
| `rematch_unmapped_files_inner` | `matched.mapping_source` |
| Playback session automatic recognition | `MappingSource::Automatic` |
| `matcher::confirm_identification` | `MappingSource::Manual` |
| File Management single/bulk mapping | `MappingSource::Manual` |
| Detail View Map Folder | `MappingSource::Manual` |
| Deep AniList match | `MappingSource::Automatic` |

For example, scanner writes become:

```rust
let matched = match_file(storage, file_path).await?;
storage
    .upsert_file_index(
        &file_path_str,
        matched.anime_id,
        matched.episode.unwrap_or(0),
        matched.confidence,
        matched.mapping_source,
        now,
    )
    .await?;
```

Manual batch calls become:

```rust
state
    .storage
    .upsert_file_mappings(&mappings, MappingSource::Manual, now)
    .await?;
```

Deep AniList batch calls use `MappingSource::Automatic` instead.

Change `rematch_unmapped_files_inner` so the command cannot silently rewrite an existing
mapping, including a manually confirmed repair whose title score is below 100:

```rust
if file.ignored || file.anime_id.is_some() {
    continue;
}
```

Existing mapped rows must use the confirmed conflict-repair flow.

- [ ] **Step 7: Update test fixtures to state their intended provenance**

For every test call found by:

```powershell
rg -n "upsert_file_index\(|upsert_file_mappings\(" src tests
```

insert a source before the timestamp. Use `Manual` for explicit anchors, `Automatic` for
scanner/title fixtures, `Inherited` only for inherited fixtures, and `Legacy` for tests
modeling upgraded data. Do not choose a source based only on confidence.

- [ ] **Step 8: Run provenance tests and compile all Rust tests**

```powershell
cargo test --test storage_test mapping_source -- --nocapture
cargo test --test mapping_sweep_test -- --nocapture
cargo check --tests
```

Expected: all commands exit 0; the Skeleton Knight match reports source `Inherited`.

- [ ] **Step 9: Commit provenance support**

```powershell
git add next/src-tauri/migrations/0007_file_index_mapping_source.sql next/src-tauri/src/engine/storage.rs next/src-tauri/src/engine/library_scanner.rs next/src-tauri/src/engine/matcher.rs next/src-tauri/src/engine/session.rs next/src-tauri/src/commands.rs next/src-tauri/tests
git commit -m "fix: track episode mapping provenance"
```

---

### Task 2: Detect Mapping Conflicts During Targeted Rescan

**Files:**
- Modify: `next/src-tauri/src/engine/library_scanner.rs`
- Create: `next/src-tauri/tests/mapping_repair_test.rs`

**Interfaces:**
- Consumes: `MappingSource`, `FileMatch`, `MATCH_THRESHOLD`, `Storage::get_file_index`, and `Storage::fetch_anime`.
- Produces: `FileMappingConflict` and `LibraryScanReport.mapping_conflicts`.
- Produces: `detect_mapping_conflicts(storage, anime_id, dirs) -> Vec<FileMappingConflict>` for Task 3.

- [ ] **Step 1: Write the exact failing rescan regression**

Create `next/src-tauri/tests/mapping_repair_test.rs` with a temp-directory helper and:

```rust
use anivault_core::engine::library_scanner::rescan_anime_dirs;
use anivault_core::engine::runtime::fresh_test_state;
use anivault_core::engine::storage::MappingSource;
use std::fs;

#[tokio::test]
async fn targeted_rescan_reports_confident_wrong_series_without_changing_it() {
    let state = fresh_test_state().await;
    let root = unique_temp_dir("confident_wrong_series");
    let season = root.join("Skeleton Knight in Another World").join("Season 2");
    fs::create_dir_all(&season).unwrap();
    let ep1 = season.join("Skeleton Knight in Another World - S02E01.mkv");
    let ep2 = season.join("Skeleton Knight in Another World - S02E02.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&ep2, b"x").unwrap();

    state.storage.insert_minimal_anime(132474, "Skeleton Knight in Another World").await.unwrap();
    state.storage.insert_minimal_anime(185542, "Skeleton Knight in Another World Season 2").await.unwrap();
    state.storage.upsert_file_index(&ep1.to_string_lossy(), Some(185542), 1, 100, MappingSource::Manual, now()).await.unwrap();
    state.storage.upsert_file_index(&ep2.to_string_lossy(), Some(132474), 2, 100, MappingSource::Legacy, now()).await.unwrap();

    let report = rescan_anime_dirs(&state.storage, 185542).await.unwrap();

    assert_eq!(report.mapping_conflicts.len(), 1);
    let conflict = &report.mapping_conflicts[0];
    assert_eq!(conflict.file_path, ep2.to_string_lossy());
    assert_eq!(conflict.current_anime_id, 132474);
    assert_eq!(conflict.current_anime_title, "Skeleton Knight in Another World");
    assert_eq!(conflict.mapping_source, MappingSource::Legacy);
    assert!(conflict.repairable);
    assert_eq!(state.storage.get_file_index(&ep2.to_string_lossy()).await.unwrap().unwrap().anime_id, Some(132474));

    fs::remove_dir_all(root).ok();
}
```

Add these complete tests in the same file, reusing the real `unique_temp_dir` and `now`
helpers from the first test:

```rust
#[tokio::test]
async fn targeted_rescan_reports_manual_conflict_as_protected() {
    let state = fresh_test_state().await;
    let root = unique_temp_dir("manual_conflict");
    let season = root.join("Skeleton Knight in Another World").join("Season 2");
    fs::create_dir_all(&season).unwrap();
    let ep1 = season.join("Skeleton Knight in Another World - S02E01.mkv");
    let ep2 = season.join("Skeleton Knight in Another World - S02E02.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&ep2, b"x").unwrap();
    state.storage.insert_minimal_anime(132474, "Skeleton Knight in Another World").await.unwrap();
    state.storage.insert_minimal_anime(185542, "Skeleton Knight in Another World Season 2").await.unwrap();
    state.storage.upsert_file_index(&ep1.to_string_lossy(), Some(185542), 1, 100, MappingSource::Manual, now()).await.unwrap();
    state.storage.upsert_file_index(&ep2.to_string_lossy(), Some(132474), 2, 100, MappingSource::Manual, now()).await.unwrap();

    let report = rescan_anime_dirs(&state.storage, 185542).await.unwrap();
    assert_eq!(report.mapping_conflicts.len(), 1);
    assert!(!report.mapping_conflicts[0].repairable);
    assert_eq!(state.storage.get_file_index(&ep2.to_string_lossy()).await.unwrap().unwrap().anime_id, Some(132474));
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn targeted_rescan_ignores_nested_and_weak_filename_conflicts() {
    let state = fresh_test_state().await;
    let root = unique_temp_dir("unrelated_conflicts");
    let season = root.join("Skeleton Knight in Another World").join("Season 2");
    let specials = season.join("Specials");
    fs::create_dir_all(&specials).unwrap();
    let anchor = season.join("Skeleton Knight in Another World - S02E01.mkv");
    let unrelated = season.join("Unrelated Movie - 01.mkv");
    let nested = specials.join("Skeleton Knight in Another World - S02E99.mkv");
    for path in [&anchor, &unrelated, &nested] {
        fs::write(path, b"x").unwrap();
    }
    state.storage.insert_minimal_anime(7, "Unrelated Movie").await.unwrap();
    state.storage.insert_minimal_anime(185542, "Skeleton Knight in Another World Season 2").await.unwrap();
    state.storage.upsert_file_index(&anchor.to_string_lossy(), Some(185542), 1, 100, MappingSource::Manual, now()).await.unwrap();
    state.storage.upsert_file_index(&unrelated.to_string_lossy(), Some(7), 1, 100, MappingSource::Automatic, now()).await.unwrap();
    state.storage.upsert_file_index(&nested.to_string_lossy(), Some(7), 99, 100, MappingSource::Automatic, now()).await.unwrap();

    let report = rescan_anime_dirs(&state.storage, 185542).await.unwrap();
    assert!(report.mapping_conflicts.is_empty());
    fs::remove_dir_all(root).ok();
}
```

- [ ] **Step 2: Run the regression and verify it fails**

```powershell
cargo test --test mapping_repair_test -- --nocapture
```

Expected: compilation fails because `mapping_conflicts` and `FileMappingConflict` do not exist.

- [ ] **Step 3: Extract reusable file queries and selected-anime scoring**

In `library_scanner.rs`, move current filename/parent/grandparent query construction into:

```rust
struct FileQueries {
    episode: Option<i32>,
    filename_title: Option<String>,
    all: Vec<String>,
}

fn file_queries(file_path: &Path) -> FileQueries {
    let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    let parsed = parse_filename(file_name, None);
    let episode = parsed
        .as_ref()
        .map(|p| p.episode_number)
        .filter(|episode| *episode > 0);
    let filename_title = parsed
        .as_ref()
        .map(|parsed| parsed.cleaned_title.trim().to_string())
        .filter(|title| !title.is_empty());
    let mut queries = Vec::new();
    if let Some(title) = &filename_title {
        queries.push(title.clone());
    }
    let parent = file_path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str());
    let grandparent = file_path.parent().and_then(|p| p.parent()).and_then(|p| p.file_name()).and_then(|n| n.to_str());
    for dir_name in [grandparent, parent].into_iter().flatten() {
        let cleaned = dir_name
            .replace(['[', ']', '(', ')', '_'], " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !cleaned.is_empty() {
            queries.push(cleaned);
        }
    }
    FileQueries { episode, filename_title, all: queries }
}

fn score_queries(queries: &[String], titles_json: &str) -> u8 {
    queries
        .iter()
        .map(|query| crate::engine::matcher::score_titles_json(query, titles_json))
        .max()
        .unwrap_or(0)
}
```

Use these helpers from `match_file` so detection and normal matching cannot drift.

- [ ] **Step 4: Implement read-only direct-sibling conflict detection**

Add:

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileMappingConflict {
    pub file_path: String,
    pub episode: Option<i32>,
    pub current_anime_id: i64,
    pub current_anime_title: String,
    pub mapping_source: MappingSource,
    pub target_confidence: i32,
    pub repairable: bool,
}
```

Implement `detect_mapping_conflicts` to fetch the target anime once, iterate each directory
with `std::fs::read_dir` (non-recursive), skip non-files/non-video/duplicate paths, load the
existing row, and require all of:

```rust
if row.ignored || row.anime_id.is_none() || row.anime_id == Some(anime_id) {
    continue;
}
let queries = file_queries(&path);
let target_score = match &queries.filename_title {
    Some(filename_title) => crate::engine::matcher::score_titles_json(filename_title, &target.titles_json),
    None => score_queries(&queries.all, &target.titles_json),
};
if target_score < MATCH_THRESHOLD {
    continue;
}
```

Resolve `current_anime_title` from the current anime's `titles_json`, preferring non-empty
English, then Romaji, then `#<id>`. Sort results by episode then file path for stable UI and tests.

- [ ] **Step 5: Attach conflicts only to targeted rescan reports**

Extend the report:

```rust
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct LibraryScanReport {
    pub found: i64,
    pub indexed: i64,
    pub skipped: i64,
    pub removed: i64,
    pub errors: Vec<String>,
    pub mapping_conflicts: Vec<FileMappingConflict>,
}
```

After targeted indexing/pruning, assign:

```rust
report.mapping_conflicts = detect_mapping_conflicts(storage, anime_id, &seen_dirs).await?;
```

Do not call conflict detection from full scans or watcher scans; their default report field
remains an empty vector.

- [ ] **Step 6: Run focused and neighboring scanner tests**

```powershell
cargo test --test mapping_repair_test -- --nocapture
cargo test --test folder_inheritance_test --test mapping_sweep_test --test library_scan_prune_test
```

Expected: all tests pass; rescan reports the wrong episode but leaves its anime ID unchanged.

- [ ] **Step 7: Commit conflict detection**

```powershell
git add next/src-tauri/src/engine/library_scanner.rs next/src-tauri/tests/mapping_repair_test.rs
git commit -m "fix: report conflicting episode mappings on rescan"
```

---

### Task 3: Add Confirmed Transactional Repair

**Files:**
- Modify: `next/src-tauri/src/engine/storage.rs`
- Modify: `next/src-tauri/src/engine/library_scanner.rs`
- Modify: `next/src-tauri/src/commands.rs`
- Modify: `next/src-tauri/src/lib.rs`
- Test: `next/src-tauri/tests/mapping_repair_test.rs`

**Interfaces:**
- Consumes: `detect_mapping_conflicts`, `MappingSource::is_repairable`, and target rescan directories.
- Produces: `FileMappingRepairReport { repaired, skipped, protected }`.
- Produces: `repair_anime_file_mappings_inner(state, anime_id)` and Tauri command `repair_anime_file_mappings`.
- Produces: guarded `Storage::repair_file_mappings` transaction.

- [ ] **Step 1: Extend the regression through confirmed repair**

Add to `mapping_repair_test.rs`:

```rust
use anivault_core::commands::repair_anime_file_mappings_inner;
use anivault_core::engine::runtime::EngineState;
use std::path::PathBuf;

async fn skeleton_knight_mixed_fixture(
    wrong_source: MappingSource,
) -> (EngineState, PathBuf, PathBuf, PathBuf) {
    let state = fresh_test_state().await;
    let root = unique_temp_dir("repair_fixture");
    let season = root.join("Skeleton Knight in Another World").join("Season 2");
    fs::create_dir_all(&season).unwrap();
    let ep1 = season.join("Skeleton Knight in Another World - S02E01.mkv");
    let ep2 = season.join("Skeleton Knight in Another World - S02E02.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&ep2, b"x").unwrap();
    state.storage.insert_minimal_anime(132474, "Skeleton Knight in Another World").await.unwrap();
    state.storage.insert_minimal_anime(185542, "Skeleton Knight in Another World Season 2").await.unwrap();
    state.storage.upsert_file_index(&ep1.to_string_lossy(), Some(185542), 1, 100, MappingSource::Manual, now()).await.unwrap();
    state.storage.upsert_file_index(&ep2.to_string_lossy(), Some(132474), 2, 100, wrong_source, now()).await.unwrap();
    (state, root, ep1, ep2)
}

#[tokio::test]
async fn confirmed_repair_moves_legacy_episode_and_detail_query_returns_both() {
    let (state, root, _ep1, ep2) = skeleton_knight_mixed_fixture(MappingSource::Legacy).await;

    let before = rescan_anime_dirs(&state.storage, 185542).await.unwrap();
    assert_eq!(before.mapping_conflicts.len(), 1);
    assert_eq!(state.storage.file_index_by_anime(185542).await.unwrap().len(), 1);

    let repaired = repair_anime_file_mappings_inner(&state, 185542).await.unwrap();
    assert_eq!(repaired.repaired, 1);
    assert_eq!(repaired.protected, 0);

    let files = state.storage.file_index_by_anime(185542).await.unwrap();
    assert_eq!(files.iter().map(|row| row.episode).collect::<Vec<_>>(), vec![Some(1), Some(2)]);
    let ep2_row = state.storage.get_file_index(&ep2.to_string_lossy()).await.unwrap().unwrap();
    assert_eq!(ep2_row.mapping_source, MappingSource::Manual);

    let second = repair_anime_file_mappings_inner(&state, 185542).await.unwrap();
    assert_eq!(second.repaired, 0);
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn repair_revalidates_and_preserves_manual_mapping_changed_after_rescan() {
    let (state, root, _ep1, ep2) = skeleton_knight_mixed_fixture(MappingSource::Legacy).await;
    assert_eq!(rescan_anime_dirs(&state.storage, 185542).await.unwrap().mapping_conflicts.len(), 1);

    state.storage.upsert_file_index(&ep2.to_string_lossy(), Some(132474), 2, 100, MappingSource::Manual, now()).await.unwrap();
    let result = repair_anime_file_mappings_inner(&state, 185542).await.unwrap();

    assert_eq!(result.repaired, 0);
    assert_eq!(result.protected, 1);
    assert_eq!(state.storage.get_file_index(&ep2.to_string_lossy()).await.unwrap().unwrap().anime_id, Some(132474));
    fs::remove_dir_all(root).ok();
}
```

- [ ] **Step 2: Run repair tests and verify they fail**

```powershell
cargo test --test mapping_repair_test -- --nocapture
```

Expected: compilation fails because the repair report, command, and storage transaction do not exist.

- [ ] **Step 3: Add guarded transactional updates in storage**

Add an input owned by the storage boundary:

```rust
pub struct FileMappingRepair {
    pub file_path: String,
    pub expected_anime_id: i64,
    pub target_anime_id: i64,
    pub episode: i32,
    pub confidence: i32,
}
```

Implement:

```rust
pub async fn repair_file_mappings(
    &self,
    repairs: &[FileMappingRepair],
    indexed_at: i64,
) -> anyhow::Result<u64> {
    let mut tx = self.pool.begin().await?;
    let mut repaired = 0;
    for repair in repairs {
        let result = sqlx::query(
            "UPDATE file_index
             SET anime_id = ?1,
                 episode = ?2,
                 confidence = ?3,
                 mapping_source = 'manual',
                 indexed_at = ?4
             WHERE file_path = ?5
               AND anime_id = ?6
               AND ignored = 0
               AND mapping_source IN ('automatic', 'inherited', 'legacy')",
        )
        .bind(repair.target_anime_id)
        .bind(repair.episode)
        .bind(repair.confidence)
        .bind(indexed_at)
        .bind(&repair.file_path)
        .bind(repair.expected_anime_id)
        .execute(&mut *tx)
        .await?;
        repaired += result.rows_affected();
    }
    tx.commit().await?;
    Ok(repaired)
}
```

The `WHERE` clause is mandatory defense in depth; do not rely only on the preceding scan.

- [ ] **Step 4: Implement server-side recomputation and report**

Add in `library_scanner.rs` a shared helper that derives real parent directories from the
selected anime's rows. Reuse it from both rescan and repair so directory scope cannot drift.

In `commands.rs` add:

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileMappingRepairReport {
    pub repaired: i64,
    pub skipped: i64,
    pub protected: i64,
}

pub async fn repair_anime_file_mappings_inner(
    state: &EngineState,
    anime_id: i64,
) -> anyhow::Result<FileMappingRepairReport> {
    let dirs = library_scanner::anime_file_dirs(&state.storage, anime_id).await?;
    let conflicts = library_scanner::detect_mapping_conflicts(&state.storage, anime_id, &dirs).await?;
    let protected = conflicts.iter().filter(|conflict| !conflict.repairable).count() as i64;
    let repairable: Vec<_> = conflicts.iter().filter(|conflict| conflict.repairable).collect();
    let repairs = repairable
        .iter()
        .map(|conflict| FileMappingRepair {
            file_path: conflict.file_path.clone(),
            expected_anime_id: conflict.current_anime_id,
            target_anime_id: anime_id,
            episode: conflict.episode.unwrap_or(0),
            confidence: conflict.target_confidence,
        })
        .collect::<Vec<_>>();
    let repaired = state.storage.repair_file_mappings(&repairs, unix_now_inner()?).await? as i64;
    Ok(FileMappingRepairReport {
        repaired,
        skipped: repairs.len() as i64 - repaired,
        protected,
    })
}
```

Before building each repair, preserve the existing positive episode when conflict parsing
returns no positive value. Load the current row again for that fallback.

- [ ] **Step 5: Expose and register the Tauri command**

Add:

```rust
#[tauri::command]
pub async fn repair_anime_file_mappings(
    anime_id: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<FileMappingRepairReport, String> {
    repair_anime_file_mappings_inner(&state, anime_id)
        .await
        .map_err(command_error)
}
```

Register `commands::repair_anime_file_mappings` in `tauri::generate_handler!` beside
`rescan_anime_files`.

- [ ] **Step 6: Run repair and command suites**

```powershell
cargo test --test mapping_repair_test -- --nocapture
cargo test --test library_commands_test --test commands_test
cargo check --tests
```

Expected: exact regression passes, stale manual mapping is protected, second repair is idempotent.

- [ ] **Step 7: Commit repair command**

```powershell
git add next/src-tauri/src/engine/storage.rs next/src-tauri/src/engine/library_scanner.rs next/src-tauri/src/commands.rs next/src-tauri/src/lib.rs next/src-tauri/tests/mapping_repair_test.rs
git commit -m "fix: repair confirmed episode mapping conflicts"
```

---

### Task 4: Enrich the Frontend Contract and Known-File Data

**Files:**
- Modify: `next/src-tauri/src/engine/storage.rs`
- Modify: `next/src-tauri/src/commands.rs`
- Modify: `next/src/lib/api.ts`
- Modify: `next/src/lib/api.test.ts`
- Create: `next/src/lib/fileMappingUi.ts`
- Create: `next/src/lib/fileMappingUi.test.ts`

**Interfaces:**
- Produces: backend `KnownFileRow` with flattened file fields plus `anime_title`.
- Produces: TypeScript `MappingSource`, `KnownFileEntry`, `FileMappingConflict`, and `FileMappingRepairReport`.
- Produces: `repairAnimeFileMappings(animeId)` wrapper.
- Produces: pure labels and `partitionMappingConflicts()` for Svelte components.

- [ ] **Step 1: Write failing API wrapper and label tests**

Extend the import list in `api.test.ts` with `repairAnimeFileMappings` and add:

```typescript
it('repairs anime file mappings through invoke', async () => {
  const report = { repaired: 1, skipped: 0, protected: 0 };
  const invoke = vi.fn().mockResolvedValue(report);
  await expect(repairAnimeFileMappings(185542, invoke)).resolves.toEqual(report);
  expect(invoke).toHaveBeenCalledWith('repair_anime_file_mappings', { animeId: 185542 });
});
```

Create `fileMappingUi.test.ts`:

```typescript
import { describe, expect, it } from 'vitest';
import { knownFileMappingLabel, mappingSourceLabel, partitionMappingConflicts } from './fileMappingUi';

describe('file mapping UI helpers', () => {
  it('shows the actual mapped title rather than the filename group', () => {
    expect(knownFileMappingLabel({
      file_path: 'D:/Skeleton Knight/Season 2/Skeleton Knight - S02E02.mkv',
      anime_id: 132474,
      anime_title: 'Skeleton Knight in Another World',
      episode: 2,
      confidence: 100,
      indexed_at: 1,
      ignored: false,
      mapping_source: 'legacy',
    })).toBe('Skeleton Knight in Another World (#132474) - Ep 2 - 100% - Legacy');
  });

  it('partitions repairable and protected conflicts', () => {
    const conflicts = [
      { file_path: 'ep2.mkv', episode: 2, current_anime_id: 1, current_anime_title: 'Base', mapping_source: 'legacy' as const, target_confidence: 80, repairable: true },
      { file_path: 'special.mkv', episode: 1, current_anime_id: 2, current_anime_title: 'Special', mapping_source: 'manual' as const, target_confidence: 80, repairable: false },
    ];
    expect(partitionMappingConflicts(conflicts)).toEqual({
      repairable: [conflicts[0]],
      protected: [conflicts[1]],
    });
    expect(mappingSourceLabel('inherited')).toBe('Inherited');
  });
});
```

- [ ] **Step 2: Run frontend tests and verify they fail**

From `next`:

```powershell
npm.cmd test -- --run src/lib/api.test.ts src/lib/fileMappingUi.test.ts
```

Expected: imports fail because the wrapper and helper module do not exist.

- [ ] **Step 3: Add an enriched known-file projection**

In `storage.rs` add this serializable projection and `list_known_files(limit, offset)`:

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct KnownFileRow {
    pub file_path: String,
    pub anime_id: Option<i64>,
    pub anime_title: Option<String>,
    pub episode: Option<i32>,
    pub confidence: i32,
    pub indexed_at: i64,
    pub ignored: bool,
    pub mapping_source: MappingSource,
}
```

Use this query:

```sql
SELECT fi.file_path,
       fi.anime_id,
       fi.episode,
       fi.confidence,
       fi.indexed_at,
       fi.ignored,
       fi.mapping_source,
       COALESCE(
         NULLIF(json_extract(a.titles_json, '$.english'), ''),
         NULLIF(json_extract(a.titles_json, '$.romaji'), '')
       ) AS anime_title
FROM file_index fi
LEFT JOIN anime a ON a.id = fi.anime_id
ORDER BY fi.indexed_at DESC
LIMIT ?1 OFFSET ?2
```

Keep `list_file_index` for internal matching. Change only `list_known_files_inner` and the
Tauri wrapper to return `Vec<KnownFileRow>`.

- [ ] **Step 4: Add exact TypeScript contracts and wrapper**

In `api.ts`:

```typescript
export type MappingSource = 'automatic' | 'inherited' | 'manual' | 'legacy';

export interface FileIndexEntry {
  file_path: string;
  anime_id: number | null;
  episode: number | null;
  confidence: number;
  indexed_at: number;
  ignored: boolean;
  mapping_source: MappingSource;
}

export interface KnownFileEntry extends FileIndexEntry {
  anime_title: string | null;
}

export interface FileMappingConflict {
  file_path: string;
  episode: number | null;
  current_anime_id: number;
  current_anime_title: string;
  mapping_source: MappingSource;
  target_confidence: number;
  repairable: boolean;
}

export interface LibraryScanReport {
  found: number;
  indexed: number;
  skipped: number;
  removed: number;
  errors: string[];
  mapping_conflicts: FileMappingConflict[];
}

export interface FileMappingRepairReport {
  repaired: number;
  skipped: number;
  protected: number;
}

export function repairAnimeFileMappings(
  animeId: number,
  invokeFn: InvokeFn = tauriInvoke,
): Promise<FileMappingRepairReport> {
  return invokeFn<FileMappingRepairReport>('repair_anime_file_mappings', { animeId });
}
```

Change `listKnownFiles` to return `Promise<KnownFileEntry[]>`. Update existing API test
fixtures to include `ignored`, `mapping_source`, and `anime_title` where required.

- [ ] **Step 5: Implement pure UI helpers**

Create `fileMappingUi.ts`:

```typescript
import type { FileMappingConflict, KnownFileEntry, MappingSource } from './api';

export function mappingSourceLabel(source: MappingSource): string {
  return {
    automatic: 'Automatic',
    inherited: 'Inherited',
    manual: 'Manual',
    legacy: 'Legacy',
  }[source];
}

export function knownFileMappingLabel(entry: KnownFileEntry): string {
  if (entry.ignored) return 'Ignored';
  if (entry.anime_id == null) return 'Unmapped';
  const title = entry.anime_title?.trim() || `Anime #${entry.anime_id}`;
  return `${title} (#${entry.anime_id}) - Ep ${entry.episode ?? '?'} - ${entry.confidence}% - ${mappingSourceLabel(entry.mapping_source)}`;
}

export function partitionMappingConflicts(conflicts: FileMappingConflict[]): {
  repairable: FileMappingConflict[];
  protected: FileMappingConflict[];
} {
  return {
    repairable: conflicts.filter((conflict) => conflict.repairable),
    protected: conflicts.filter((conflict) => !conflict.repairable),
  };
}
```

- [ ] **Step 6: Run backend compile and frontend contract tests**

```powershell
npm.cmd test -- --run src/lib/api.test.ts src/lib/fileMappingUi.test.ts
npm.cmd run check
```

From `next/src-tauri`:

```powershell
cargo check --tests
```

Expected: all commands exit 0 and known-file JSON has actual titles and source strings.

- [ ] **Step 7: Commit API and helper support**

```powershell
git add next/src-tauri/src/engine/storage.rs next/src-tauri/src/commands.rs next/src/lib/api.ts next/src/lib/api.test.ts next/src/lib/fileMappingUi.ts next/src/lib/fileMappingUi.test.ts
git commit -m "feat: expose episode mapping conflicts"
```

---

### Task 5: Present Mapping Truth and Confirmed Repair in the UI

**Files:**
- Modify: `next/src/lib/FileManager.svelte`
- Modify: `next/src/lib/DetailView.svelte`
- Test: `next/src/lib/fileMappingUi.test.ts`

**Interfaces:**
- Consumes: `KnownFileEntry`, `LibraryScanReport`, `repairAnimeFileMappings`, and pure UI helpers from Task 4.
- Produces: visible mapping identity/source in File Management and an inline rescan/repair flow in Episode Files.

- [ ] **Step 1: Add one failing helper assertion for protected-only state**

Extend `fileMappingUi.test.ts`:

```typescript
it('returns no repairable conflicts for protected-only input', () => {
  const manual = {
    file_path: 'ep2.mkv', episode: 2, current_anime_id: 1,
    current_anime_title: 'Base', mapping_source: 'manual' as const,
    target_confidence: 80, repairable: false,
  };
  expect(partitionMappingConflicts([manual])).toEqual({ repairable: [], protected: [manual] });
});
```

Run:

```powershell
npm.cmd test -- --run src/lib/fileMappingUi.test.ts
```

Expected: pass after Task 4; this locks the branch the Svelte markup will consume.

- [ ] **Step 2: Make File Management display the real mapping**

Change File Manager state and group types from `FileIndexEntry` to `KnownFileEntry`. Import
`knownFileMappingLabel`. Keep `seriesKey()` and `Mixed` grouping behavior unchanged.

Replace the mapped row text with:

```svelte
{:else if e.anime_id != null}
  {knownFileMappingLabel(e)}
```

Keep the filename-derived group title visually separate from the status badge. Add title
attributes to the mapping badge so long anime titles remain inspectable without changing
row height.

- [ ] **Step 3: Add persistent Episode Files action state**

In `DetailView.svelte`, import `repairAnimeFileMappings`, `partitionMappingConflicts`,
`mappingSourceLabel`, and the report types. Add:

```typescript
let fileScanReport: LibraryScanReport | null = null;
let fileActionError: string | null = null;
let fileActionMessage: string | null = null;
let repairConfirming = false;
let repairing = false;

$: conflictGroups = partitionMappingConflicts(fileScanReport?.mapping_conflicts ?? []);
```

When `animeId` changes, clear scan report, error, message, and confirmation state while
leaving the normal episode-loading lifecycle intact.

- [ ] **Step 4: Replace silent rescan failure with conflict-aware handling**

Use:

```typescript
async function handleRescanFiles() {
  rescanning = true;
  fileActionError = null;
  fileActionMessage = null;
  repairConfirming = false;
  try {
    fileScanReport = await rescanAnimeFiles(animeId);
    await loadEpisodeFiles();
    if (fileScanReport.mapping_conflicts.length === 0) {
      fileActionMessage = 'Rescan complete. No mapping conflicts found.';
    }
  } catch (e) {
    fileActionError = e instanceof Error ? e.message : String(e);
  } finally {
    rescanning = false;
  }
}
```

Do not clear `episodeFiles` in the catch branch.

- [ ] **Step 5: Add confirmation and repair handlers**

```typescript
function beginRepairMappings() {
  if (conflictGroups.repairable.length > 0) repairConfirming = true;
}

function cancelRepairMappings() {
  repairConfirming = false;
}

async function confirmRepairMappings() {
  if (repairing) return;
  repairing = true;
  fileActionError = null;
  try {
    const result = await repairAnimeFileMappings(animeId);
    await loadEpisodeFiles();
    fileScanReport = await rescanAnimeFiles(animeId);
    fileActionMessage = `Repaired ${result.repaired} file mapping${result.repaired === 1 ? '' : 's'}.`;
    repairConfirming = false;
  } catch (e) {
    fileActionError = e instanceof Error ? e.message : String(e);
  } finally {
    repairing = false;
  }
}
```

- [ ] **Step 6: Render compact conflict, protected, confirmation, and error states**

Below the Episode Files actions, render a single un-nested warning section when conflicts exist:

```svelte
{#if fileScanReport && fileScanReport.mapping_conflicts.length > 0}
  <div class="mapping-conflict-warning">
    <p>{fileScanReport.mapping_conflicts.length} conflicting file mapping{fileScanReport.mapping_conflicts.length === 1 ? '' : 's'} found.</p>
    <ul>
      {#each fileScanReport.mapping_conflicts as conflict (conflict.file_path)}
        <li>
          <span>Ep {conflict.episode ?? '?'}</span>
          <span>{conflict.current_anime_title} (#{conflict.current_anime_id})</span>
          <span>{mappingSourceLabel(conflict.mapping_source)}</span>
          <span class="ep-path">{conflict.file_path}</span>
          {#if !conflict.repairable}<span class="protected-label">Manual mapping protected</span>{/if}
        </li>
      {/each}
    </ul>
    {#if conflictGroups.repairable.length > 0 && !repairConfirming}
      <button class="action-btn small" on:click={beginRepairMappings}>Repair mappings</button>
    {:else if repairConfirming}
      <div class="repair-confirm-row">
        <span>Move {conflictGroups.repairable.length} eligible file{conflictGroups.repairable.length === 1 ? '' : 's'} to this anime?</span>
        <button class="action-btn small" on:click={confirmRepairMappings} disabled={repairing}>{repairing ? 'Repairing...' : 'Confirm repair'}</button>
        <button class="action-btn small" on:click={cancelRepairMappings} disabled={repairing}>Cancel</button>
      </div>
    {/if}
    {#if conflictGroups.protected.length > 0}
      <p class="muted">Protected manual mappings were not changed. Use Map folder to override them intentionally.</p>
    {/if}
  </div>
{/if}
{#if fileActionMessage}<p class="success-msg">{fileActionMessage}</p>{/if}
{#if fileActionError}<p class="error">{fileActionError}</p>{/if}
```

Add restrained styles using existing colors, `border-radius: 6px`, stable flex/grid tracks,
wrapping paths, and no nested card styling. Use ASCII three-dot copy in new strings.

- [ ] **Step 7: Run frontend tests, type-check, and production build**

```powershell
npm.cmd test
npm.cmd run check
npm.cmd run build
```

Expected: Vitest passes, TypeScript exits 0, Vite build exits 0. Existing Svelte warnings may
remain, but there must be no new warning tied to the conflict markup.

- [ ] **Step 8: Manually verify the two UI states**

Use a test database or the exact Skeleton Knight data:

1. Open Season 2 and press Rescan.
2. Verify episode 2 appears as a repairable conflict mapped to base series `#132474`.
3. Cancel and verify no database change.
4. Confirm repair and verify the Episode Files list shows episodes 1 and 2.
5. Change a conflict fixture to source `manual`; verify it is protected and no repair button
   appears when all conflicts are protected.
6. Disconnect the mapped drive or force a command error; verify existing episode rows stay visible.

- [ ] **Step 9: Commit the UI flow**

```powershell
git add next/src/lib/FileManager.svelte next/src/lib/DetailView.svelte next/src/lib/fileMappingUi.test.ts
git commit -m "fix: confirm episode mapping repairs in library view"
```

---

### Task 6: Release Version, Full Verification, and Installer

**Files:**
- Modify: `next/package.json`
- Modify: `next/package-lock.json`
- Modify: `next/src-tauri/Cargo.toml`
- Modify: `next/src-tauri/Cargo.lock`
- Modify: `next/src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: all prior tasks.
- Produces: AniVault `1.0.2` NSIS installer and checksum.

- [ ] **Step 1: Confirm the release is a SemVer patch**

The public contract remains backward compatible and fixes incorrect behavior, so update
`1.0.1` to `1.0.2`. Do not change dependency versions.

- [ ] **Step 2: Update all five version sources**

Set exactly:

```text
next/package.json                         1.0.2
next/package-lock.json (root + package)   1.0.2
next/src-tauri/Cargo.toml                 1.0.2
next/src-tauri/Cargo.lock (anivault)      1.0.2
next/src-tauri/tauri.conf.json            1.0.2
```

Verify:

```powershell
rg -n 'version.*1\.0\.[0-9]+|name = "anivault"' next/package.json next/package-lock.json next/src-tauri/Cargo.toml next/src-tauri/Cargo.lock next/src-tauri/tauri.conf.json
```

Expected: every application version is `1.0.2`; dependency versions are unchanged.

- [ ] **Step 3: Run focused regression suites**

From `next/src-tauri`:

```powershell
cargo test --test storage_test mapping_source -- --nocapture
cargo test --test mapping_repair_test -- --nocapture
cargo test --test folder_inheritance_test --test mapping_sweep_test --test library_scan_prune_test
```

From `next`:

```powershell
npm.cmd test -- --run src/lib/api.test.ts src/lib/fileMappingUi.test.ts
```

Expected: all focused tests pass.

- [ ] **Step 4: Run complete verification suites**

From `next/src-tauri`:

```powershell
cargo test
```

From `next`:

```powershell
npm.cmd run check
npm.cmd test
npm.cmd run build
```

Expected: all commands exit 0. Record any pre-existing Svelte warnings separately; do not
claim a clean build if a command fails.

- [ ] **Step 5: Build the NSIS installer**

From `next`:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\bundle.ps1
```

Expected artifact:

```text
next/src-tauri/target/release/bundle/nsis/AniVault_1.0.2_x64-setup.exe
```

- [ ] **Step 6: Verify and checksum the artifact**

```powershell
$installer = Resolve-Path '.\src-tauri\target\release\bundle\nsis\AniVault_1.0.2_x64-setup.exe'
Get-Item -LiteralPath $installer | Select-Object FullName,Length,LastWriteTime
Get-FileHash -Algorithm SHA256 -LiteralPath $installer | Select-Object Algorithm,Hash,Path
```

Expected: installer exists, has non-zero length, and produces a 64-character SHA-256 hash.

- [ ] **Step 7: Inspect final scope and commit release metadata**

```powershell
git status --short
git diff --check
git diff --stat
git add next/package.json next/package-lock.json next/src-tauri/Cargo.toml next/src-tauri/Cargo.lock next/src-tauri/tauri.conf.json
git commit -m "chore: release 1.0.2"
```

Expected: only intended source, test, design/plan, and version changes are present; generated
`target` and `dist` files remain ignored unless the repository already tracks them.

- [ ] **Step 8: Report completion evidence**

Provide:

- Exact installer path.
- Installer byte size.
- SHA-256 checksum.
- Focused and full test command results.
- Any remaining warnings or manual-mapping conflicts that intentionally require Map Folder.
