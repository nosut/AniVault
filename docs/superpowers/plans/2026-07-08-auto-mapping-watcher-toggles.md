# Automatic Mapping, Library Watcher & Startup Switches Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** New episode files self-map and appear automatically (folder inheritance + watcher + startup/hourly scan), and the Startup settings use slider switches.

**Architecture:** Backend (Rust/Tauri): extend `library_scanner::match_file` with folder inheritance, add a `library_watcher` module hosting a startup+hourly scan worker and a `notify`-based filesystem watcher, and re-match siblings after manual mappings. A new `EngineEvent::LibraryUpdated` flows through the existing 3-second event poll to refresh the Svelte UI. Frontend: two Startup toggle buttons become `role="switch"` sliders.

**Tech Stack:** Rust (Tauri 2, sqlx/SQLite, tokio, notify 6 — already a dependency), Svelte 4 + TypeScript + vitest.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-08-auto-mapping-watcher-toggles-design.md`.
- Backend tests: run `cargo test` from `next/src-tauri`. Frontend tests: run `npm test` from `next`.
- Integration tests live in `next/src-tauri/tests/*.rs` and use `anivault_core::engine::storage::Tests::new_in_memory()` / `anivault_core::engine::runtime::fresh_test_state()`; the test-only helper `storage.insert_minimal_anime(id, title)` inserts an anime row.
- Inherited-mapping confidence is exactly **85**; manual mappings stay **100**; the title auto-attach threshold stays **40**.
- Watcher debounce quiet period: **5 seconds**. Auto-scan cadence: **~20 s after launch, then every 3600 s**.
- Windows is the only target platform (paths use `\`), but path logic must handle both separators as existing code does (`['\\', '/']`).
- All file paths below are relative to the repo root `C:\Users\nosut\Downloads\Tools\AI projects\AniVault`.

---

### Task 1: Startup toggles → switch style

**Files:**
- Modify: `next/src/lib/SettingsView.svelte:429-452` (markup), `next/src/lib/SettingsView.svelte:1099-1123` (CSS)

**Interfaces:**
- Consumes: existing `.switch` / `.switch-thumb` CSS (SettingsView.svelte:1061-1097) and existing handlers `handleStartupToggle`, `handleStartInTrayToggle`.
- Produces: nothing used by later tasks.

- [ ] **Step 1: Replace the two toggle-btn buttons with switches**

In `next/src/lib/SettingsView.svelte`, replace lines 429-452 (the two `toggle-row` divs in the Startup panel) with:

```svelte
            <div class="toggle-row">
              <span class="label">Launch AniVault when Windows starts</span>
              <button
                type="button"
                role="switch"
                aria-checked={startupEnabled}
                class="switch"
                on:click={handleStartupToggle}
              >
                <span class="switch-thumb" />
              </button>
            </div>
            <div class="toggle-row">
              <span class="label">Start minimized to the system tray</span>
              <button
                type="button"
                role="switch"
                aria-checked={startInTray}
                class="switch"
                on:click={handleStartInTrayToggle}
              >
                <span class="switch-thumb" />
              </button>
            </div>
```

- [ ] **Step 2: Remove the now-unused `.toggle-btn` styles**

Delete these four CSS rules (SettingsView.svelte:1099-1123) — first confirm with a search that `toggle-btn` no longer appears anywhere else in this file (Svelte errors on unused scoped selectors only as warnings, but dead CSS should go):

```css
  .toggle-btn { ... }
  .toggle-btn:hover { ... }
  .toggle-btn.active { ... }
  .toggle-btn:focus-visible { ... }
```

- [ ] **Step 3: Run the frontend test suite and build**

Run (from `next/`): `npm test`
Expected: all vitest suites PASS (no component tests exist for SettingsView; this guards against import/syntax breakage).

Run (from `next/`): `npx svelte-check --threshold error` — expected: no errors. (If svelte-check isn't installed, `npm run build` also compiles all components.)

- [ ] **Step 4: Commit**

```bash
git add next/src/lib/SettingsView.svelte
git commit -m "feat: startup settings use slider switches like Enable tracking"
```

---

### Task 2: Storage queries for inheritance and sibling sweep

**Files:**
- Modify: `next/src-tauri/src/engine/storage.rs` (add two methods after `file_paths_under`, which ends at line 771)
- Test: `next/src-tauri/tests/folder_inheritance_test.rs` (create)

**Interfaces:**
- Consumes: existing `file_index` table (columns `file_path, anime_id, episode, confidence, indexed_at, ignored`).
- Produces:
  - `Storage::mapped_files_under(&self, dir: &str) -> anyhow::Result<Vec<(String, i64)>>` — `(file_path, anime_id)` of non-ignored rows with `anime_id NOT NULL AND confidence > 0` whose path starts with `dir` (caller passes a prefix WITH trailing separator).
  - `Storage::unmatched_files_under(&self, dir: &str) -> anyhow::Result<Vec<String>>` — paths of non-ignored rows with `anime_id IS NULL`, same prefix semantics.

- [ ] **Step 1: Write the failing tests**

Create `next/src-tauri/tests/folder_inheritance_test.rs`:

```rust
use anivault_core::engine::storage::Tests;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Create a unique empty temp directory for a test and return its path.
fn unique_temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("anivault_inherit_{tag}_{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[tokio::test]
async fn mapped_files_under_returns_only_real_matches() {
    let storage = Tests::new_in_memory().await;
    storage.insert_minimal_anime(7, "Some Show").await.unwrap();
    storage.insert_minimal_anime(8, "Other Show").await.unwrap();

    // Windows-style paths; the queries are pure string-prefix matches so no
    // real files are needed here.
    let base = "C:\\Lib\\ShowA\\";
    storage.upsert_file_index("C:\\Lib\\ShowA\\ep1.mkv", Some(7), 1, 100, now()).await.unwrap();
    storage.upsert_file_index("C:\\Lib\\ShowA\\ep2.mkv", None, 0, 0, now()).await.unwrap(); // unmatched
    storage.upsert_file_index("C:\\Lib\\ShowA2\\ep1.mkv", Some(8), 1, 100, now()).await.unwrap(); // sibling folder
    // Ignored row must never count.
    storage.upsert_file_index("C:\\Lib\\ShowA\\junk.mkv", Some(8), 1, 100, now()).await.unwrap();
    storage.set_file_index_ignored("C:\\Lib\\ShowA\\junk.mkv", true).await.unwrap();

    let mapped = storage.mapped_files_under(base).await.unwrap();
    assert_eq!(mapped, vec![("C:\\Lib\\ShowA\\ep1.mkv".to_string(), 7)]);

    let unmatched = storage.unmatched_files_under(base).await.unwrap();
    assert_eq!(unmatched, vec!["C:\\Lib\\ShowA\\ep2.mkv".to_string()]);
}
```

Note: `set_file_index_ignored` clears `anime_id` when ignoring (storage.rs:702-721), so the ignored row is excluded by both the `ignored = 0` filter and the NULL `anime_id` — the test documents that either way it never appears.

- [ ] **Step 2: Run the test to verify it fails**

Run (from `next/src-tauri`): `cargo test --test folder_inheritance_test`
Expected: COMPILE ERROR — `mapped_files_under` / `unmatched_files_under` not found.

- [ ] **Step 3: Implement the two queries**

In `next/src-tauri/src/engine/storage.rs`, directly after `file_paths_under` (after line 771), add:

```rust
    /// Mapped, non-ignored rows under `dir`: `(file_path, anime_id)` for rows
    /// with a real match (`confidence > 0`). `dir` must include a trailing path
    /// separator (see `file_paths_under`). Used for folder-inheritance matching.
    pub async fn mapped_files_under(&self, dir: &str) -> anyhow::Result<Vec<(String, i64)>> {
        // Escape LIKE metacharacters so paths with % or _ match literally.
        let escaped = dir
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("{escaped}%");
        let rows = sqlx::query(
            "SELECT file_path, anime_id FROM file_index
             WHERE file_path LIKE ?1 ESCAPE '\\'
               AND anime_id IS NOT NULL AND ignored = 0 AND confidence > 0
             ORDER BY file_path",
        )
        .bind(pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| (r.get::<String, _>("file_path"), r.get::<i64, _>("anime_id")))
            .collect())
    }

    /// Non-ignored rows under `dir` that have no anime match at all. `dir` must
    /// include a trailing path separator. Used to re-match siblings after a
    /// manual mapping.
    pub async fn unmatched_files_under(&self, dir: &str) -> anyhow::Result<Vec<String>> {
        let escaped = dir
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("{escaped}%");
        let rows = sqlx::query(
            "SELECT file_path FROM file_index
             WHERE file_path LIKE ?1 ESCAPE '\\'
               AND anime_id IS NULL AND ignored = 0
             ORDER BY file_path",
        )
        .bind(pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| r.get::<String, _>("file_path")).collect())
    }
```

- [ ] **Step 4: Run the test to verify it passes**

Run (from `next/src-tauri`): `cargo test --test folder_inheritance_test`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
git add next/src-tauri/src/engine/storage.rs next/src-tauri/tests/folder_inheritance_test.rs
git commit -m "feat: storage queries for mapped/unmatched files under a directory"
```

---

### Task 3: Folder inheritance in `match_file`

**Files:**
- Modify: `next/src-tauri/src/engine/library_scanner.rs` (constant near line 33; `match_file` body lines 112-118; new pure function)
- Test: `next/src-tauri/tests/folder_inheritance_test.rs` (extend)

**Interfaces:**
- Consumes: `Storage::mapped_files_under` (Task 2), existing `dir_prefix(dir: &str) -> String` (library_scanner.rs:341-344), `MATCH_THRESHOLD: u8 = 40`.
- Produces:
  - `pub fn unanimous_dir_anime(rows: &[(String, i64)], prefix: &str) -> Option<i64>` in `library_scanner`.
  - `const INHERITED_CONFIDENCE: i32 = 85;` (private).
  - Changed behavior of `match_file`: when title matching scores below threshold AND an episode number parsed AND all mapped direct siblings agree on one anime, returns `(Some(anime_id), 85, Some(episode))`.

- [ ] **Step 1: Write the failing tests**

Append to `next/src-tauri/tests/folder_inheritance_test.rs`:

```rust
use anivault_core::engine::library_scanner::{match_file, unanimous_dir_anime};

#[test]
fn unanimous_dir_anime_direct_children_only() {
    let prefix = "C:\\Lib\\ShowA\\";
    // Direct children agreeing → Some
    let rows = vec![
        ("C:\\Lib\\ShowA\\ep1.mkv".to_string(), 7),
        ("C:\\Lib\\ShowA\\ep2.mkv".to_string(), 7),
    ];
    assert_eq!(unanimous_dir_anime(&rows, prefix), Some(7));

    // Disagreement → None
    let mixed = vec![
        ("C:\\Lib\\ShowA\\ep1.mkv".to_string(), 7),
        ("C:\\Lib\\ShowA\\other.mkv".to_string(), 8),
    ];
    assert_eq!(unanimous_dir_anime(&mixed, prefix), None);

    // A row in a subdirectory is not a direct sibling — ignored.
    let sub = vec![("C:\\Lib\\ShowA\\Season 1\\ep1.mkv".to_string(), 7)];
    assert_eq!(unanimous_dir_anime(&sub, prefix), None);

    // No rows → None
    assert_eq!(unanimous_dir_anime(&[], prefix), None);
}

#[tokio::test]
async fn file_inherits_unanimous_folder_anime() {
    let storage = Tests::new_in_memory().await;
    let dir = unique_temp_dir("inherit");
    let ep1 = dir.join("Zzqx Qwpv - 01.mkv");
    let ep2 = dir.join("Zzqx Qwpv - 02.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&ep2, b"x").unwrap();

    // The anime title shares no words with the filename, so title matching
    // fails — exactly the situation that forced manual mapping.
    storage.insert_minimal_anime(7, "Completely Unrelated Title").await.unwrap();
    // Episode 1 was mapped manually (confidence 100).
    storage
        .upsert_file_index(&ep1.to_string_lossy(), Some(7), 1, 100, now())
        .await
        .unwrap();

    let (anime_id, confidence, episode) = match_file(&storage, &ep2).await.unwrap();
    assert_eq!(anime_id, Some(7), "episode 2 must inherit the folder's anime");
    assert_eq!(confidence, 85);
    assert_eq!(episode, Some(2));

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn mixed_folder_does_not_inherit() {
    let storage = Tests::new_in_memory().await;
    let dir = unique_temp_dir("mixed");
    let a = dir.join("Zzqx Qwpv - 01.mkv");
    let b = dir.join("Vvbnm Rrtyu - 01.mkv");
    let c = dir.join("Zzqx Qwpv - 02.mkv");
    for f in [&a, &b, &c] {
        fs::write(f, b"x").unwrap();
    }
    storage.insert_minimal_anime(7, "Completely Unrelated Title").await.unwrap();
    storage.insert_minimal_anime(8, "Another Unrelated Title").await.unwrap();
    storage.upsert_file_index(&a.to_string_lossy(), Some(7), 1, 100, now()).await.unwrap();
    storage.upsert_file_index(&b.to_string_lossy(), Some(8), 1, 100, now()).await.unwrap();

    let (anime_id, confidence, _) = match_file(&storage, &c).await.unwrap();
    assert_eq!(anime_id, None, "disagreeing siblings must not inherit");
    assert_eq!(confidence, 0);

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn no_episode_number_does_not_inherit() {
    let storage = Tests::new_in_memory().await;
    let dir = unique_temp_dir("noep");
    let ep1 = dir.join("Zzqx Qwpv - 01.mkv");
    let extra = dir.join("Zzqx Qwpv.mkv"); // no parsable episode number
    fs::write(&ep1, b"x").unwrap();
    fs::write(&extra, b"x").unwrap();
    storage.insert_minimal_anime(7, "Completely Unrelated Title").await.unwrap();
    storage.upsert_file_index(&ep1.to_string_lossy(), Some(7), 1, 100, now()).await.unwrap();

    let (anime_id, confidence, _) = match_file(&storage, &extra).await.unwrap();
    assert_eq!(anime_id, None, "no episode number → leave unmatched");
    assert_eq!(confidence, 0);

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn confident_title_match_beats_inheritance() {
    let storage = Tests::new_in_memory().await;
    let dir = unique_temp_dir("titlewins");
    let sibling = dir.join("Zzqx Qwpv - 01.mkv");
    let movie = dir.join("Great Vault Movie - 01.mkv");
    fs::write(&sibling, b"x").unwrap();
    fs::write(&movie, b"x").unwrap();
    storage.insert_minimal_anime(7, "Completely Unrelated Title").await.unwrap();
    storage.insert_minimal_anime(9, "Great Vault Movie").await.unwrap();
    storage.upsert_file_index(&sibling.to_string_lossy(), Some(7), 1, 100, now()).await.unwrap();

    let (anime_id, confidence, _) = match_file(&storage, &movie).await.unwrap();
    assert_eq!(anime_id, Some(9), "a confident title match must win over inheritance");
    assert!(confidence >= 40);

    fs::remove_dir_all(&dir).ok();
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run (from `next/src-tauri`): `cargo test --test folder_inheritance_test`
Expected: COMPILE ERROR — `unanimous_dir_anime` not found (and after stubbing, the behavior tests fail with `anime_id == None`).

- [ ] **Step 3: Implement inheritance**

In `next/src-tauri/src/engine/library_scanner.rs`:

(a) After the `MATCH_THRESHOLD` constant (line 33), add:

```rust
/// Confidence recorded when a file inherits its anime from unanimous mapped
/// siblings in the same folder — below manual (100) so manual wins on display,
/// well above the title threshold so the scanner treats it as a real match.
const INHERITED_CONFIDENCE: i32 = 85;
```

(b) Add the pure function (near `dir_prefix`, after line 344):

```rust
/// If every mapped file directly inside the directory `prefix` (which must end
/// with a path separator) agrees on one anime, return it. Rows in
/// subdirectories are ignored; disagreement or no direct siblings → None.
pub fn unanimous_dir_anime(rows: &[(String, i64)], prefix: &str) -> Option<i64> {
    let mut found: Option<i64> = None;
    for (path, anime_id) in rows {
        let Some(rest) = path.strip_prefix(prefix) else { continue };
        if rest.contains(['\\', '/']) {
            continue; // lives in a subdirectory, not a direct sibling
        }
        match found {
            None => found = Some(*anime_id),
            Some(a) if a == *anime_id => {}
            Some(_) => return None,
        }
    }
    found
}
```

(c) In `match_file`, replace the threshold block (lines 112-118):

```rust
    // Require a minimum confidence to auto-attach; below that, leave unmatched (0)
    // so the file resurfaces on the next re-scan and can be corrected manually.
    let (anime_id, confidence) = if best_score >= MATCH_THRESHOLD {
        (best_id, best_score as i32)
    } else {
        (None, 0)
    };
```

with:

```rust
    // Require a minimum confidence to auto-attach. Below the threshold, fall
    // back to folder inheritance: if every mapped file in this directory agrees
    // on one anime and we parsed an episode number, adopt that anime — one
    // manual mapping then fixes the whole series. Otherwise leave unmatched (0)
    // so the file resurfaces on the next re-scan.
    let (anime_id, confidence) = if best_score >= MATCH_THRESHOLD {
        (best_id, best_score as i32)
    } else if let (Some(_), Some(dir)) = (
        episode,
        file_path.parent().and_then(|p| p.to_str()).filter(|d| !d.is_empty()),
    ) {
        let prefix = dir_prefix(dir);
        let mut rows = storage.mapped_files_under(&prefix).await?;
        // Never inherit from this file's own (stale) row.
        rows.retain(|(p, _)| p != &file_path_str);
        match unanimous_dir_anime(&rows, &prefix) {
            Some(id) => (Some(id), INHERITED_CONFIDENCE),
            None => (None, 0),
        }
    } else {
        (None, 0)
    };
```

- [ ] **Step 4: Run the tests to verify they pass**

Run (from `next/src-tauri`): `cargo test --test folder_inheritance_test`
Expected: PASS (6 tests).

- [ ] **Step 5: Run the full backend suite (guards existing scanner behavior)**

Run (from `next/src-tauri`): `cargo test`
Expected: all tests PASS.

- [ ] **Step 6: Commit**

```bash
git add next/src-tauri/src/engine/library_scanner.rs next/src-tauri/tests/folder_inheritance_test.rs
git commit -m "feat: files inherit anime from unanimous mapped folder siblings"
```

---

### Task 4: Manual mapping immediately re-matches siblings

**Files:**
- Modify: `next/src-tauri/src/engine/library_scanner.rs` (two new helpers)
- Modify: `next/src-tauri/src/commands.rs:1519-1564` (`set_known_file_mapping`, `set_known_file_mappings` → add `_inner` variants that also sweep)
- Test: `next/src-tauri/tests/mapping_sweep_test.rs` (create)

**Interfaces:**
- Consumes: `Storage::unmatched_files_under` (Task 2), `match_file` with inheritance (Task 3), `unix_now_inner() -> anyhow::Result<i64>` (commands.rs:97).
- Produces:
  - `library_scanner::parent_dirs(paths: &[String]) -> Vec<String>` (pub) — distinct parent directories.
  - `library_scanner::rematch_unmatched_in_dirs(storage: &Storage, dirs: &[String]) -> anyhow::Result<usize>` (pub, async) — re-runs `match_file` on unmatched rows under each dir, upserts only rows that gained a match, returns the count.
  - `commands::set_known_file_mapping_inner(state: &EngineState, file_path: &str, anime_id: i64, episode: i32) -> anyhow::Result<()>`
  - `commands::set_known_file_mappings_inner(state: &EngineState, mappings: Vec<FileMappingInput>) -> anyhow::Result<usize>`

- [ ] **Step 1: Write the failing tests**

Create `next/src-tauri/tests/mapping_sweep_test.rs`:

```rust
use anivault_core::commands::{set_known_file_mapping_inner, FileMappingInput};
use anivault_core::engine::runtime::fresh_test_state;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("anivault_sweep_{tag}_{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[tokio::test]
async fn manual_mapping_sweeps_unmatched_siblings() {
    let state = fresh_test_state().await;
    let dir = unique_temp_dir("basic");
    let ep1 = dir.join("Zzqx Qwpv - 01.mkv");
    let ep2 = dir.join("Zzqx Qwpv - 02.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&ep2, b"x").unwrap();

    state.storage.insert_minimal_anime(7, "Completely Unrelated Title").await.unwrap();
    // Both start unmatched, as a failed scan would leave them.
    state.storage.upsert_file_index(&ep1.to_string_lossy(), None, 0, 0, now()).await.unwrap();
    state.storage.upsert_file_index(&ep2.to_string_lossy(), None, 0, 0, now()).await.unwrap();

    // Manually map episode 1 — episode 2 must self-map via inheritance.
    set_known_file_mapping_inner(&state, &ep1.to_string_lossy(), 7, 1)
        .await
        .unwrap();

    let ep1_row = state.storage.get_file_index(&ep1.to_string_lossy()).await.unwrap().unwrap();
    assert_eq!(ep1_row.anime_id, Some(7));
    assert_eq!(ep1_row.confidence, 100);

    let ep2_row = state.storage.get_file_index(&ep2.to_string_lossy()).await.unwrap().unwrap();
    assert_eq!(ep2_row.anime_id, Some(7), "sibling must be swept into the mapping");
    assert_eq!(ep2_row.confidence, 85);
    assert_eq!(ep2_row.episode, Some(2));

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn sweep_leaves_ignored_files_alone() {
    let state = fresh_test_state().await;
    let dir = unique_temp_dir("ignored");
    let ep1 = dir.join("Zzqx Qwpv - 01.mkv");
    let junk = dir.join("Zzqx Qwpv - 02.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&junk, b"x").unwrap();

    state.storage.insert_minimal_anime(7, "Completely Unrelated Title").await.unwrap();
    state.storage.upsert_file_index(&ep1.to_string_lossy(), None, 0, 0, now()).await.unwrap();
    state.storage.upsert_file_index(&junk.to_string_lossy(), None, 0, 0, now()).await.unwrap();
    state.storage.set_file_index_ignored(&junk.to_string_lossy(), true).await.unwrap();

    set_known_file_mapping_inner(&state, &ep1.to_string_lossy(), 7, 1)
        .await
        .unwrap();

    let junk_row = state.storage.get_file_index(&junk.to_string_lossy()).await.unwrap().unwrap();
    assert_eq!(junk_row.anime_id, None, "ignored files must never be swept");
    assert!(junk_row.ignored);

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn bulk_mapping_sweeps_siblings_too() {
    let state = fresh_test_state().await;
    let dir = unique_temp_dir("bulk");
    let ep1 = dir.join("Zzqx Qwpv - 01.mkv");
    let ep2 = dir.join("Zzqx Qwpv - 02.mkv");
    let ep3 = dir.join("Zzqx Qwpv - 03.mkv");
    for f in [&ep1, &ep2, &ep3] {
        fs::write(f, b"x").unwrap();
    }
    state.storage.insert_minimal_anime(7, "Completely Unrelated Title").await.unwrap();
    for f in [&ep1, &ep2, &ep3] {
        state.storage.upsert_file_index(&f.to_string_lossy(), None, 0, 0, now()).await.unwrap();
    }

    // Bulk-map episodes 1 and 2; episode 3 must be swept.
    let n = anivault_core::commands::set_known_file_mappings_inner(
        &state,
        vec![
            FileMappingInput { file_path: ep1.to_string_lossy().to_string(), anime_id: 7, episode: 1 },
            FileMappingInput { file_path: ep2.to_string_lossy().to_string(), anime_id: 7, episode: 2 },
        ],
    )
    .await
    .unwrap();
    assert_eq!(n, 2);

    let ep3_row = state.storage.get_file_index(&ep3.to_string_lossy()).await.unwrap().unwrap();
    assert_eq!(ep3_row.anime_id, Some(7));
    assert_eq!(ep3_row.confidence, 85);
    assert_eq!(ep3_row.episode, Some(3));

    fs::remove_dir_all(&dir).ok();
}
```

Note: `FileMappingInput` (commands.rs:1538-1543) derives only `Deserialize`; the test constructs it directly, so its fields are already `pub`. If the compiler complains about construction, add `#[derive(serde::Deserialize)]`-adjacent field visibility is already `pub` — no change expected.

- [ ] **Step 2: Run the tests to verify they fail**

Run (from `next/src-tauri`): `cargo test --test mapping_sweep_test`
Expected: COMPILE ERROR — `set_known_file_mapping_inner` / `set_known_file_mappings_inner` not found.

- [ ] **Step 3: Implement the sweep helpers in `library_scanner.rs`**

Add after `rematch`-related code (e.g. after `match_file`, before `unix_now`):

```rust
/// Distinct parent directories of a set of file paths, in first-seen order.
pub fn parent_dirs(paths: &[String]) -> Vec<String> {
    let mut dirs: Vec<String> = Vec::new();
    for p in paths {
        if let Some(d) = Path::new(p).parent().and_then(|d| d.to_str()) {
            if !d.is_empty() && !dirs.iter().any(|x| x == d) {
                dirs.push(d.to_string());
            }
        }
    }
    dirs
}

/// Re-run matching for the unmatched, non-ignored files under the given
/// directories. Called after a manual mapping so siblings of the newly mapped
/// file inherit it immediately (see `unanimous_dir_anime`) instead of waiting
/// for the next scan. Only rows that gain a match are written; returns how many.
pub async fn rematch_unmatched_in_dirs(
    storage: &Storage,
    dirs: &[String],
) -> anyhow::Result<usize> {
    let now = unix_now();
    let mut updated = 0usize;
    for dir in dirs {
        let prefix = dir_prefix(dir);
        for path in storage.unmatched_files_under(&prefix).await? {
            let (anime_id, confidence, episode) = match_file(storage, Path::new(&path)).await?;
            if anime_id.is_some() {
                storage
                    .upsert_file_index(&path, anime_id, episode.unwrap_or(0), confidence, now)
                    .await?;
                updated += 1;
            }
        }
    }
    Ok(updated)
}
```

- [ ] **Step 4: Refactor the two mapping commands to `_inner` + sweep**

In `next/src-tauri/src/commands.rs`, replace the bodies at lines 1519-1564:

```rust
/// Manually map a known file to an anime + episode at full confidence, then
/// re-match the unmatched files in the same folder so siblings inherit the
/// mapping immediately. Unlike `confirm_identification`, this is a management
/// write and does not emit a playback identification event.
pub async fn set_known_file_mapping_inner(
    state: &EngineState,
    file_path: &str,
    anime_id: i64,
    episode: i32,
) -> anyhow::Result<()> {
    let now = unix_now_inner()?;
    state
        .storage
        .upsert_file_index(file_path, Some(anime_id), episode, 100, now)
        .await?;
    let dirs = library_scanner::parent_dirs(&[file_path.to_string()]);
    library_scanner::rematch_unmatched_in_dirs(&state.storage, &dirs).await?;
    Ok(())
}

#[tauri::command]
pub async fn set_known_file_mapping(
    file_path: String,
    anime_id: i64,
    episode: i32,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    set_known_file_mapping_inner(&state, &file_path, anime_id, episode)
        .await
        .map_err(command_error)
}

/// One entry of a bulk mapping request.
#[derive(serde::Deserialize)]
pub struct FileMappingInput {
    pub file_path: String,
    pub anime_id: i64,
    pub episode: i32,
}

/// Bulk manual mapping — map many files to anime + episode at once (one
/// transaction), then sweep unmatched siblings in the affected folders.
pub async fn set_known_file_mappings_inner(
    state: &EngineState,
    mappings: Vec<FileMappingInput>,
) -> anyhow::Result<usize> {
    let now = unix_now_inner()?;
    let tuples: Vec<(String, i64, i32)> = mappings
        .into_iter()
        .map(|m| (m.file_path, m.anime_id, m.episode))
        .collect();
    let count = tuples.len();
    state.storage.upsert_file_mappings(&tuples, now).await?;
    let paths: Vec<String> = tuples.into_iter().map(|(p, _, _)| p).collect();
    let dirs = library_scanner::parent_dirs(&paths);
    library_scanner::rematch_unmatched_in_dirs(&state.storage, &dirs).await?;
    Ok(count)
}

#[tauri::command]
pub async fn set_known_file_mappings(
    mappings: Vec<FileMappingInput>,
    state: tauri::State<'_, EngineState>,
) -> Result<usize, String> {
    set_known_file_mappings_inner(&state, mappings)
        .await
        .map_err(command_error)
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run (from `next/src-tauri`): `cargo test --test mapping_sweep_test`
Expected: PASS (3 tests).

- [ ] **Step 6: Run the full backend suite**

Run (from `next/src-tauri`): `cargo test`
Expected: all tests PASS (existing `commands_test.rs` and file-manager flows unaffected — command names and signatures are unchanged).

- [ ] **Step 7: Commit**

```bash
git add next/src-tauri/src/engine/library_scanner.rs next/src-tauri/src/commands.rs next/src-tauri/tests/mapping_sweep_test.rs
git commit -m "feat: manual mapping re-matches unmatched siblings in the same folder"
```

---

### Task 5: `LibraryUpdated` event + startup/hourly auto-scan worker

**Files:**
- Modify: `next/src-tauri/src/engine/events.rs` (new enum variant)
- Modify: `next/src-tauri/src/engine/library_scanner.rs:263-283` (`index_new_files_in_dir`: don't rewrite/count unchanged unmatched files)
- Create: `next/src-tauri/src/engine/library_watcher.rs`
- Modify: `next/src-tauri/src/engine/mod.rs` (register module)
- Modify: `next/src-tauri/src/lib.rs:70` (spawn the worker)
- Test: `next/src-tauri/tests/library_watcher_test.rs` (create)

**Interfaces:**
- Consumes: `EngineState` (`.storage`, `.events: EventBus` with `publish`/`drain`), `library_scanner::{get_library_folders, scan_library_folders, LibraryScanReport}`.
- Produces:
  - `EngineEvent::LibraryUpdated { indexed: i64, removed: i64 }`.
  - `library_watcher::run_auto_scan(state: &EngineState)` (pub, async, infallible — logs errors).
  - `library_watcher::spawn_library_scan_worker(state: &EngineState) -> tauri::async_runtime::JoinHandle<()>`.
  - Changed `LibraryScanReport.indexed` semantics: counts only **new or changed** rows (an unmatched file that stays unmatched is `skipped`).

- [ ] **Step 1: Write the failing tests**

Create `next/src-tauri/tests/library_watcher_test.rs`:

```rust
use anivault_core::engine::events::EngineEvent;
use anivault_core::engine::library_scanner::set_library_folders;
use anivault_core::engine::library_watcher::run_auto_scan;
use anivault_core::engine::runtime::fresh_test_state;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("anivault_watch_{tag}_{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn auto_scan_publishes_event_only_on_change() {
    let state = fresh_test_state().await;
    let dir = unique_temp_dir("autoscan");
    fs::write(dir.join("Show - 01.mkv"), b"x").unwrap();
    set_library_folders(&state.storage, vec![dir.to_string_lossy().to_string()])
        .await
        .unwrap();

    // First pass indexes the file → event.
    run_auto_scan(&state).await;
    let events = state.events.drain();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, EngineEvent::LibraryUpdated { indexed: 1, removed: 0 })),
        "first scan must publish LibraryUpdated, got {events:?}"
    );

    // Second pass: nothing changed on disk → no event (and no index churn).
    run_auto_scan(&state).await;
    assert!(
        state.events.drain().is_empty(),
        "unchanged rescan must stay silent"
    );

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn auto_scan_without_folders_is_a_no_op() {
    let state = fresh_test_state().await;
    run_auto_scan(&state).await;
    assert!(state.events.drain().is_empty());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run (from `next/src-tauri`): `cargo test --test library_watcher_test`
Expected: COMPILE ERROR — module `library_watcher` / variant `LibraryUpdated` not found.

- [ ] **Step 3: Add the event variant**

In `next/src-tauri/src/engine/events.rs`, add to the `EngineEvent` enum (after `SyncFailed`):

```rust
    /// An automatic scan (watcher or timer) changed the file index — the UI
    /// should refresh library/file views. Not emitted for manual scans, which
    /// already return their report to the caller.
    LibraryUpdated {
        indexed: i64,
        removed: i64,
    },
```

- [ ] **Step 4: Make unchanged unmatched files silent in `index_new_files_in_dir`**

In `next/src-tauri/src/engine/library_scanner.rs`, replace the loop body of `index_new_files_in_dir` (lines 263-281):

```rust
    for file_path in &video_files {
        let file_path_str = file_path.to_string_lossy().to_string();
        report.found += 1;

        // Skip if already indexed with a valid match; re-evaluate if unmatched
        // (confidence 0). Ignored files are tombstoned — never re-index them.
        let existing = storage.get_file_index(&file_path_str).await?;
        if let Some(ref ex) = existing {
            if ex.ignored || ex.confidence > 0 {
                report.skipped += 1;
                continue;
            }
        }

        let (anime_id, confidence, episode) = match_file(storage, file_path).await?;

        // An already-indexed unmatched file that stays unmatched isn't a change —
        // don't rewrite it, so `indexed` reports only real changes and periodic
        // auto-scans stay silent when nothing happened.
        if existing.is_some() && anime_id.is_none() {
            report.skipped += 1;
            continue;
        }

        storage
            .upsert_file_index(&file_path_str, anime_id, episode.unwrap_or(0), confidence, now)
            .await?;
        report.indexed += 1;
    }
    Ok(())
```

- [ ] **Step 5: Create `library_watcher.rs` with the auto-scan worker**

Create `next/src-tauri/src/engine/library_watcher.rs`:

```rust
//! Automatic library maintenance: a startup + hourly scan worker (this file)
//! and, added alongside it, a filesystem watcher for near-real-time pickup.

use std::time::Duration;

use crate::engine::events::EngineEvent;
use crate::engine::library_scanner;
use crate::engine::runtime::EngineState;

/// Run one automatic library scan pass. Publishes `LibraryUpdated` only when
/// the index actually changed; silent no-op when no folders are configured.
/// Errors are logged, never propagated — automatic passes must not kill loops.
pub async fn run_auto_scan(state: &EngineState) {
    let folders = match library_scanner::get_library_folders(&state.storage).await {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!("auto-scan: cannot read library folders: {e}");
            return;
        }
    };
    if folders.is_empty() {
        return;
    }
    match library_scanner::scan_library_folders(&state.storage).await {
        Ok(r) if r.indexed > 0 || r.removed > 0 => {
            tracing::info!(indexed = r.indexed, removed = r.removed, "auto-scan changed the index");
            state.events.publish(EngineEvent::LibraryUpdated {
                indexed: r.indexed,
                removed: r.removed,
            });
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("auto-scan failed: {e}"),
    }
}

/// Spawn the startup + hourly automatic scan: one pass shortly after launch
/// (delayed so tracking/sync startup settles first), then every hour.
pub fn spawn_library_scan_worker(state: &EngineState) -> tauri::async_runtime::JoinHandle<()> {
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(20)).await;
        loop {
            run_auto_scan(&state).await;
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    })
}
```

In `next/src-tauri/src/engine/mod.rs`, add (after `pub mod library_scanner;`):

```rust
pub mod library_watcher;
```

- [ ] **Step 6: Spawn it at startup**

In `next/src-tauri/src/lib.rs`, after `sync_worker::spawn_sync_worker(&state);` (line 70), add:

```rust
            engine::library_watcher::spawn_library_scan_worker(&state);
```

- [ ] **Step 7: Run the tests to verify they pass**

Run (from `next/src-tauri`): `cargo test --test library_watcher_test`
Expected: PASS (2 tests).

- [ ] **Step 8: Run the full backend suite**

Run (from `next/src-tauri`): `cargo test`
Expected: all tests PASS. Watch for scanner tests that assert `indexed` on re-scans of unmatched files — the new skip changes that count; if one fails, verify the test's intent against the new semantics (indexed = new or changed) and update the assertion accordingly.

- [ ] **Step 9: Commit**

```bash
git add next/src-tauri/src/engine/events.rs next/src-tauri/src/engine/library_scanner.rs next/src-tauri/src/engine/library_watcher.rs next/src-tauri/src/engine/mod.rs next/src-tauri/src/lib.rs next/src-tauri/tests/library_watcher_test.rs
git commit -m "feat: LibraryUpdated event + startup/hourly automatic library scan"
```

---

### Task 6: Filesystem watcher with per-directory debounce

**Files:**
- Modify: `next/src-tauri/src/engine/runtime.rs` (new `EngineState` field)
- Modify: `next/src-tauri/src/engine/library_scanner.rs` (`is_video_file`, `scan_specific_dirs` helpers)
- Modify: `next/src-tauri/src/engine/library_watcher.rs` (watcher task + pure helpers)
- Modify: `next/src-tauri/src/commands.rs:769-774` (`set_library_folders_inner` signals the watcher)
- Modify: `next/src-tauri/src/lib.rs` (spawn the watcher)
- Test: `next/src-tauri/tests/library_watcher_test.rs` (extend — pure functions only; the OS watcher itself is not integration-tested, the hourly scan is its fallback)

**Interfaces:**
- Consumes: `notify` crate v6 (already in Cargo.toml), `EngineState`, `scan_dirs` (private in library_scanner — exposed via new wrapper).
- Produces:
  - `EngineState.library_folders_changed: Arc<tokio::sync::Notify>` — notified after `set_library_folders`.
  - `library_scanner::is_video_file(path: &Path) -> bool` (pub).
  - `library_scanner::scan_specific_dirs(storage: &Storage, dirs: &[String]) -> anyhow::Result<LibraryScanReport>` (pub, async).
  - `library_watcher::affected_dirs(paths: &[PathBuf]) -> Vec<PathBuf>` (pub).
  - `library_watcher::take_quiet_dirs(pending: &mut HashMap<PathBuf, Instant>, now: Instant, quiet: Duration) -> Vec<PathBuf>` (pub).
  - `library_watcher::spawn_library_watcher(state: &EngineState) -> tauri::async_runtime::JoinHandle<()>`.

- [ ] **Step 1: Write the failing tests for the pure helpers**

Append to `next/src-tauri/tests/library_watcher_test.rs`:

```rust
use anivault_core::engine::library_watcher::{affected_dirs, take_quiet_dirs};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[test]
fn affected_dirs_filters_to_video_parents() {
    let paths = vec![
        PathBuf::from("C:\\Lib\\ShowA\\ep1.mkv"),
        PathBuf::from("C:\\Lib\\ShowA\\ep1.mkv"), // duplicate event
        PathBuf::from("C:\\Lib\\ShowA\\ep2.MP4"), // extension is case-insensitive
        PathBuf::from("C:\\Lib\\ShowB\\notes.txt"), // not a video
        PathBuf::from("C:\\Lib\\ShowC\\ep1.mkv.part"), // in-progress download
    ];
    let dirs = affected_dirs(&paths);
    assert_eq!(dirs, vec![PathBuf::from("C:\\Lib\\ShowA")]);
}

#[test]
fn take_quiet_dirs_respects_debounce() {
    let base = Instant::now();
    let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
    pending.insert(PathBuf::from("C:\\Lib\\Quiet"), base);
    pending.insert(PathBuf::from("C:\\Lib\\Busy"), base + Duration::from_secs(4));

    // 5 seconds after `base`: Quiet has been silent 5s (ready), Busy only 1s.
    let ready = take_quiet_dirs(&mut pending, base + Duration::from_secs(5), Duration::from_secs(5));
    assert_eq!(ready, vec![PathBuf::from("C:\\Lib\\Quiet")]);
    assert!(pending.contains_key(&PathBuf::from("C:\\Lib\\Busy")));
    assert!(!pending.contains_key(&PathBuf::from("C:\\Lib\\Quiet")));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run (from `next/src-tauri`): `cargo test --test library_watcher_test`
Expected: COMPILE ERROR — `affected_dirs` / `take_quiet_dirs` not found.

- [ ] **Step 3: Add the `Notify` signal to `EngineState`**

In `next/src-tauri/src/engine/runtime.rs`:

(a) Add the field to the struct (after `app_handle`):

```rust
    /// Notified whenever the configured library folders change, so the
    /// filesystem watcher can rebuild its watch list.
    pub library_folders_changed: Arc<tokio::sync::Notify>,
```

(b) Initialize it in BOTH constructors (`fresh_test_state` and `initialize_engine_at`):

```rust
        library_folders_changed: Arc::new(tokio::sync::Notify::new()),
```

(c) In `next/src-tauri/src/commands.rs`, change `set_library_folders_inner` (lines 769-774) to:

```rust
pub async fn set_library_folders_inner(
    state: &EngineState,
    folders: Vec<String>,
) -> anyhow::Result<()> {
    library_scanner::set_library_folders(&state.storage, folders).await?;
    // Wake the filesystem watcher so it re-watches the new folder set.
    state.library_folders_changed.notify_waiters();
    Ok(())
}
```

- [ ] **Step 4: Add the scanner helpers**

In `next/src-tauri/src/engine/library_scanner.rs`:

(a) Add near `find_video_files` and refactor it to share the check:

```rust
/// Does this path have a recognized video-file extension?
pub fn is_video_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    VIDEO_EXTENSIONS.contains(&ext.as_str())
}
```

and inside `find_video_files`, replace the extension check (lines 364-371) with:

```rust
                } else if path.is_file() && is_video_file(&path) {
                    files.push(path);
                }
```

(b) Add a public wrapper for targeted directory scans (after `scan_library_folders`):

```rust
/// Scan a specific set of directories: index new/changed files and prune
/// deleted ones under each. Used by the filesystem watcher for targeted scans;
/// a directory that no longer exists is silently skipped (never pruned under).
pub async fn scan_specific_dirs(
    storage: &Storage,
    dirs: &[String],
) -> anyhow::Result<LibraryScanReport> {
    scan_dirs(storage, dirs, false).await
}
```

- [ ] **Step 5: Implement the watcher in `library_watcher.rs`**

Extend `next/src-tauri/src/engine/library_watcher.rs` — update the imports at the top to:

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use notify::Watcher;

use crate::engine::events::EngineEvent;
use crate::engine::library_scanner;
use crate::engine::runtime::EngineState;
```

and append:

```rust
/// How long a directory must stay quiet after its last filesystem event before
/// we scan it — rides out multi-file moves and in-progress downloads.
const DEBOUNCE_QUIET: Duration = Duration::from_secs(5);

/// Directories worth rescanning for a batch of event paths: the parent of
/// every touched video file, deduplicated, in first-seen order.
pub fn affected_dirs(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for p in paths {
        if !library_scanner::is_video_file(p) {
            continue;
        }
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() && !dirs.contains(&parent.to_path_buf()) {
                dirs.push(parent.to_path_buf());
            }
        }
    }
    dirs
}

/// Remove and return the pending directories whose last event is at least
/// `quiet` ago — they're ready to scan. Recently-busy directories stay pending.
pub fn take_quiet_dirs(
    pending: &mut HashMap<PathBuf, Instant>,
    now: Instant,
    quiet: Duration,
) -> Vec<PathBuf> {
    let mut ready: Vec<PathBuf> = pending
        .iter()
        .filter(|(_, t)| now.saturating_duration_since(**t) >= quiet)
        .map(|(d, _)| d.clone())
        .collect();
    ready.sort();
    for d in &ready {
        pending.remove(d);
    }
    ready
}

/// Watch the configured library folders and run targeted scans when video
/// files change. Rebuilds its watch list when `library_folders_changed` fires.
/// Folders that are offline or fail to watch are logged and left to the hourly
/// scan as fallback.
pub fn spawn_library_watcher(state: &EngineState) -> tauri::async_runtime::JoinHandle<()> {
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let folders = library_scanner::get_library_folders(&state.storage)
                .await
                .unwrap_or_default();

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<notify::Event>();
            let mut watcher = match notify::recommended_watcher(
                move |res: Result<notify::Event, notify::Error>| {
                    if let Ok(ev) = res {
                        let _ = tx.send(ev);
                    }
                },
            ) {
                Ok(w) => w,
                Err(e) => {
                    tracing::warn!("library watcher unavailable: {e}");
                    // Still honor folder-change signals so a later config change retries.
                    state.library_folders_changed.notified().await;
                    continue;
                }
            };

            for f in &folders {
                let path = std::path::Path::new(f);
                if !path.exists() {
                    tracing::debug!("not watching offline library folder {f}");
                    continue;
                }
                if let Err(e) = watcher.watch(path, notify::RecursiveMode::Recursive) {
                    tracing::warn!("cannot watch library folder {f}: {e}");
                }
            }

            let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
            loop {
                tokio::select! {
                    ev = rx.recv() => {
                        match ev {
                            Some(event) => {
                                for d in affected_dirs(&event.paths) {
                                    pending.insert(d, Instant::now());
                                }
                            }
                            None => break, // watcher gone; rebuild
                        }
                    }
                    _ = state.library_folders_changed.notified() => break, // rebuild with new folders
                    _ = tokio::time::sleep(Duration::from_secs(1)), if !pending.is_empty() => {
                        let ready = take_quiet_dirs(&mut pending, Instant::now(), DEBOUNCE_QUIET);
                        if ready.is_empty() {
                            continue;
                        }
                        let dirs: Vec<String> =
                            ready.iter().map(|d| d.to_string_lossy().to_string()).collect();
                        match library_scanner::scan_specific_dirs(&state.storage, &dirs).await {
                            Ok(r) if r.indexed > 0 || r.removed > 0 => {
                                tracing::info!(
                                    indexed = r.indexed,
                                    removed = r.removed,
                                    "watcher scan changed the index"
                                );
                                state.events.publish(EngineEvent::LibraryUpdated {
                                    indexed: r.indexed,
                                    removed: r.removed,
                                });
                            }
                            Ok(_) => {}
                            Err(e) => tracing::warn!("watcher scan failed: {e}"),
                        }
                    }
                }
            }
            drop(watcher);
        }
    })
}
```

- [ ] **Step 6: Spawn the watcher at startup**

In `next/src-tauri/src/lib.rs`, after the `spawn_library_scan_worker` line added in Task 5:

```rust
            engine::library_watcher::spawn_library_watcher(&state);
```

- [ ] **Step 7: Run the tests to verify they pass**

Run (from `next/src-tauri`): `cargo test --test library_watcher_test`
Expected: PASS (4 tests).

- [ ] **Step 8: Run the full backend suite**

Run (from `next/src-tauri`): `cargo test`
Expected: all tests PASS.

- [ ] **Step 9: Manual smoke check (watcher end-to-end)**

Run (from `next/`): `npm run tauri dev` (or the project's dev command). With a library folder configured, copy a video file into it, wait ~6 seconds, and confirm the log shows `watcher scan changed the index`. Close the app.
If a dev run isn't feasible in this session, note it in the commit message body and rely on the pure-function tests + hourly-scan fallback.

- [ ] **Step 10: Commit**

```bash
git add next/src-tauri/src/engine/runtime.rs next/src-tauri/src/engine/library_scanner.rs next/src-tauri/src/engine/library_watcher.rs next/src-tauri/src/commands.rs next/src-tauri/src/lib.rs next/src-tauri/tests/library_watcher_test.rs
git commit -m "feat: filesystem watcher scans changed library folders in near-real-time"
```

---

### Task 7: Frontend — `LibraryUpdated` type and UI auto-refresh

**Files:**
- Modify: `next/src/lib/api.ts:141-155` (event interfaces/union)
- Modify: `next/src/lib/api.test.ts` (drain test)
- Modify: `next/src/App.svelte:141` (pass events to SettingsView)
- Modify: `next/src/lib/SettingsView.svelte` (accept events, pass to FileManager at line 561)
- Modify: `next/src/lib/FileManager.svelte` (reload on event)
- Modify: `next/src/lib/LibraryView.svelte` (refresh stats + episode files on event)

**Interfaces:**
- Consumes: `EngineEvent::LibraryUpdated { indexed, removed }` (Task 5) — serde serializes it as `{ "LibraryUpdated": { "indexed": n, "removed": n } }`, matching the existing externally-tagged event style.
- Produces: `LibraryUpdatedEvent` TypeScript interface in `api.ts`, used by the three components.

- [ ] **Step 1: Write the failing test**

In `next/src/lib/api.test.ts`, next to the existing `drainEngineEvents` tests (~line 127), add:

```ts
  it('passes through LibraryUpdated events', async () => {
    const events = [{ LibraryUpdated: { indexed: 2, removed: 1 } }];
    const invoke = vi.fn().mockResolvedValue(events);
    const result = await drainEngineEvents(invoke);
    expect(result).toEqual(events);
    const ev = result[0];
    if ('LibraryUpdated' in ev) {
      expect(ev.LibraryUpdated.indexed).toBe(2);
      expect(ev.LibraryUpdated.removed).toBe(1);
    } else {
      throw new Error('expected LibraryUpdated event');
    }
  });
```

- [ ] **Step 2: Run the test to verify it fails**

Run (from `next/`): `npm test`
Expected: FAIL — TypeScript error: `LibraryUpdated` is not a member of the `EngineEvent` union (vitest surfaces it as a type/compile error).

- [ ] **Step 3: Add the event type**

In `next/src/lib/api.ts`, after `SyncFailedEvent` (line 147), add:

```ts
export interface LibraryUpdatedEvent {
  LibraryUpdated: {
    indexed: number;
    removed: number;
  };
}
```

and extend the union (line 149-155):

```ts
export type EngineEvent =
  | MediaDetectedEvent
  | PlaybackDetectedEvent
  | AnimeIdentifiedEvent
  | ProgressAdvancedEvent
  | SyncQueuedEvent
  | SyncFailedEvent
  | LibraryUpdatedEvent;
```

- [ ] **Step 4: Run the test to verify it passes**

Run (from `next/`): `npm test`
Expected: PASS.

- [ ] **Step 5: Wire the refresh into the views**

(a) `next/src/App.svelte` line 141 — pass events:

```svelte
    {:else if currentView === 'settings'}
      <SettingsView events={latestEvents} />
```

(b) `next/src/lib/SettingsView.svelte` — accept and forward. Add to the script (after the existing imports, near line 20):

```ts
  import type { EngineEvent } from './api';
  export let events: EngineEvent[] = [];
```

and change line 561:

```svelte
        <FileManager {events} />
```

(c) `next/src/lib/FileManager.svelte` — reload the list when an automatic scan changed something. Add after the `let entries` block (~line 32):

```ts
  import type { EngineEvent } from './api';
  export let events: EngineEvent[] = [];

  // Refresh when an automatic scan (watcher / hourly) changed the index, so
  // newly detected files appear without pressing anything.
  $: applyLibraryUpdated(events);
  function applyLibraryUpdated(evs: EngineEvent[]) {
    if (!loading && evs.some((e) => 'LibraryUpdated' in e)) void load();
  }
```

(imports must stay at the top of the `<script>` block with the existing import statement — merge `type EngineEvent` into the existing `from './api'` import instead of adding a second one).

(d) `next/src/lib/LibraryView.svelte` — refresh stats and per-show episode files. Add next to `applyProgressEvents` (line 130):

```ts
  // Refresh stats and episode-file lists when an automatic scan changes the
  // index (new episode downloaded, file deleted, …).
  $: applyLibraryUpdated(events);
  function applyLibraryUpdated(evs: EngineEvent[]) {
    if (!evs || !evs.some((e) => 'LibraryUpdated' in e)) return;
    void loadStats();
    if (entries.length > 0) void loadEpisodeFiles(entries);
  }
```

- [ ] **Step 6: Run frontend tests + typecheck**

Run (from `next/`): `npm test`
Expected: PASS.
Run (from `next/`): `npx svelte-check --threshold error` (or `npm run build`)
Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add next/src/lib/api.ts next/src/lib/api.test.ts next/src/App.svelte next/src/lib/SettingsView.svelte next/src/lib/FileManager.svelte next/src/lib/LibraryView.svelte
git commit -m "feat: UI auto-refreshes when automatic scans change the library"
```

---

### Task 8: Full verification + version bump

**Files:**
- Modify: `next/package.json:3`, `next/src-tauri/Cargo.toml:3`, `next/src-tauri/tauri.conf.json:4` (0.1.4 → 0.1.5)

**Interfaces:**
- Consumes: everything above.
- Produces: release-ready tree.

- [ ] **Step 1: Run both full test suites**

Run (from `next/src-tauri`): `cargo test`
Expected: all tests PASS.
Run (from `next/`): `npm test`
Expected: all tests PASS.

- [ ] **Step 2: Bump the version in all three manifests**

Change `"version": "0.1.4"` → `"version": "0.1.5"` in `next/package.json` and `next/src-tauri/tauri.conf.json`, and `version = "0.1.4"` → `version = "0.1.5"` in `next/src-tauri/Cargo.toml`. Then run (from `next/src-tauri`): `cargo check` so `Cargo.lock` picks up the new version.

- [ ] **Step 3: End-to-end verification (use the superpowers:verification-before-completion skill)**

Launch the app (`npm run tauri dev` from `next/`) and verify:
1. Settings → General: both Startup rows render as slider switches and flip on click, with the "Saving…/Saved" indicator behaving as before.
2. Settings → Files: with one unmapped test series (two files, gibberish names), manually map file 1 — file 2 should appear mapped after the save without any scan.
3. Drop a new episode file into a watched folder → within ~10 seconds the Files tab (left open) refreshes and shows it mapped.

- [ ] **Step 4: Commit**

```bash
git add next/package.json next/src-tauri/Cargo.toml next/src-tauri/Cargo.lock next/src-tauri/tauri.conf.json
git commit -m "chore: bump version to 0.1.5"
```
