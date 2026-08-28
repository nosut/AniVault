# M7 Migration and Data Safety Implementation Plan

> **For agentic workers:** Use subagent-driven-development (recommended) to implement task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Import Taiga v1 data (anime DB, list entries, watch history) with dry-run preview, duplicate handling, backup/restore, and export/import. No silent data loss.

**Architecture:** Rework `engine::migration` module into sub-modules: `discovery.rs` (find v1 data), `v1_read.rs` (parse v1 SQLite + XML), `importer.rs` (dry-run + live import with duplicate handling), `backup.rs` (DB backup/restore, JSON export/import). 6 new Tauri commands. Frontend: Settings → Migration tab with discover/dry-run/import/backup controls.

**Data sources:**
- v1 SQLite `{data}/media.sqlite` — tables `anime` (metadata), `anime_list` (user entries)
- v1 History XML `{data}/history.xml` — watch history items
- v1 XML fallback `{data}/v1/db/anime.xml`, `{data}/v1/user/{user}@{svc}/anime.xml`

**Data path discovery priority:** `{exe_dir}/data/` (portable) → `%APPDATA%/Taiga/data/` (installed).

**Tech Stack:** Rust, Tauri 2.x, rusqlite (for reading v1 DB), quick-xml (for v1 XML), sqlx (v2 DB), Svelte 5, TypeScript, Vitest.

---

## Global Constraints

- Windows desktop app runtime using Tauri, Svelte, Rust, SQLite
- AniList is the only tracker; no MAL/Kitsu import
- M7 scope: migration only — no tray (M5), no Sonarr (M6), no rebrand (M8)
- Every fallible command returns `Result<T, String>`
- Inner functions use `&EngineState` for testability
- Import must be previewable (dry-run) before applying
- Backup must run before live import
- Duplicate handling: skip existing anime_id by default, merge option for metadata
- V1 SQLite read via `rusqlite` (separate from v2's sqlx for isolation)
- V1 XML read via `quick-xml` (serde-compatible)

### New Cargo Dependencies

```toml
rusqlite = { version = "0.31", features = ["bundled"] }
quick-xml = { version = "0.36", features = ["serialize"] }
```

---

### Task 1: V1 Data Discovery Module

**Files:**
- Create: `next/src-tauri/src/engine/migration/discovery.rs`
- Modify: `next/src-tauri/src/engine/migration/mod.rs`

**Interfaces:**
- Produces: `V1DataPaths { sqlite_path: Option<String>, history_xml_path: Option<String>, anime_xml_path: Option<String>, list_xml_path: Option<String> }`
- Produces: `discover_v1_data() -> V1DataPaths`

**Steps:**

- [ ] **Step 1: Create `discovery.rs`**

Scan standard locations for v1 data files. Check portable path first, then AppData. Return discovered paths.

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct V1DataPaths {
    pub sqlite_path: Option<String>,       // {data}/media.sqlite
    pub history_xml_path: Option<String>,  // {data}/history.xml
    pub anime_xml_path: Option<String>,    // {data}/v1/db/anime.xml
    pub list_xml_path: Option<String>,     // {data}/v1/user/*/anime.xml (first match)
    pub data_dir: Option<String>,          // detected data directory
}

fn candidate_data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    // Portable: next to exe
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("data"));
        }
    }
    // Installed: %APPDATA%/Taiga/data
    if let Ok(appdata) = std::env::var("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("Taiga").join("data"));
    }
    // Also try %LOCALAPPDATA%
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Taiga").join("data"));
    }
    dirs
}

pub fn discover_v1_data() -> V1DataPaths {
    for dir in candidate_data_dirs() {
        let sqlite = dir.join("media.sqlite");
        if sqlite.exists() {
            let history = dir.join("history.xml");
            let anime_xml = dir.join("v1").join("db").join("anime.xml");
            // Find first list XML under v1/user/
            let list_xml = glob_list_xml(&dir);
            return V1DataPaths {
                sqlite_path: Some(sqlite.to_string_lossy().to_string()),
                history_xml_path: history.exists().then(|| history.to_string_lossy().to_string()),
                anime_xml_path: anime_xml.exists().then(|| anime_xml.to_string_lossy().to_string()),
                list_xml_path: list_xml,
                data_dir: Some(dir.to_string_lossy().to_string()),
            };
        }
    }
    V1DataPaths::default()
}

fn glob_list_xml(data_dir: &PathBuf) -> Option<String> {
    let user_dir = data_dir.join("v1").join("user");
    if !user_dir.exists() { return None; }
    // Walk user_dir for any anime.xml (skip settings.xml)
    for entry in std::fs::read_dir(&user_dir).ok()? {
        let entry = entry.ok()?;
        if entry.path().is_dir() {
            let list = entry.path().join("anime.xml");
            if list.exists() {
                return Some(list.to_string_lossy().to_string());
            }
        }
    }
    None
}
```

- [ ] **Step 2: Wire module into `mod.rs`**

Rewrite `next/src-tauri/src/engine/migration/mod.rs`:

```rust
pub mod backup;
pub mod discovery;
pub mod importer;
pub mod v1_read;

pub use discovery::{discover_v1_data, V1DataPaths};
pub use importer::{dry_run_import, live_import, DuplicateStrategy, MigrationReport, MigrationWarning};
pub use backup::{backup_database, restore_database, export_database, import_database};
```

- [ ] **Step 3: Build check**

```bash
cd next/src-tauri && cargo check --tests
```

- [ ] **Step 4: Commit**

---

### Task 2: V1 Data Reader (SQLite + XML)

**Files:**
- Create: `next/src-tauri/src/engine/migration/v1_read.rs`
- Create: `next/src-tauri/tests/migration_v1_read_test.rs`

**Interfaces:**
- Produces: `V1Anime`, `V1ListEntry`, `V1HistoryItem` structs
- Produces: `read_v1_sqlite(path: &str) -> (Vec<V1Anime>, Vec<V1ListEntry>)`
- Produces: `read_v1_history_xml(path: &str) -> Vec<V1HistoryItem>`
- Produces: `read_v1_anime_xml(path: &str) -> Vec<V1Anime>`

**Steps:**

- [ ] **Step 1: Write failing test `tests/migration_v1_read_test.rs`**

Test reading from in-memory SQLite with v1 schema:

```rust
use taiga_next::engine::migration::v1_read::*;

fn create_test_v1_db() -> String {
    // Create in-memory SQLite with v1 anime + anime_list tables, return path string
    // Actually use tempfile to create on-disk test DB
    todo!("create temp v1 db with test data, read it back, assert parsed correctly")
}
```

- [ ] **Step 2: Implement `v1_read.rs`**

```rust
use rusqlite::Connection;
use quick_xml::de::from_str;
use serde::Deserialize;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct V1Anime {
    pub id: i64,
    pub title: String,
    pub english: String,
    pub japanese: String,
    pub synonyms: Vec<String>,
    pub anime_type: i32,       // enum: 1=Tv, 2=Ova, 3=Movie, 4=Special, 5=Ona, 6=Music
    pub status: i32,           // enum: 1=FinishedAiring, 2=Airing, 3=NotYetAired
    pub episode_count: i32,
    pub image_url: String,
    pub synopsis: String,
    pub score: f32,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub last_modified: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct V1ListEntry {
    pub anime_id: i64,
    pub watched_episodes: i32,
    pub score: i32,             // 0-100
    pub status: i32,            // 0=NotInList, 1=Watching, 2=Completed, 3=OnHold, 4=Dropped, 5=PlanToWatch
    pub date_started: String,
    pub date_completed: String,
    pub notes: String,
    pub last_updated: i64,
    pub rewatched_times: i32,
    pub rewatching: bool,
    pub rewatching_ep: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct V1HistoryItem {
    pub anime_id: i64,
    pub episode: i32,
    pub timestamp: String,      // ISO datetime string
}

pub fn read_v1_sqlite(path: &str) -> Result<(Vec<V1Anime>, Vec<V1ListEntry>), anyhow::Error> {
    let conn = Connection::open(path)?;

    // Read anime table
    let mut anime_stmt = conn.prepare("SELECT * FROM anime")?;
    let anime: Vec<V1Anime> = anime_stmt.query_map([], |row| {
        Ok(V1Anime {
            id: row.get("id")?,
            title: row.get::<_, String>("title").unwrap_or_default(),
            english: row.get::<_, String>("english").unwrap_or_default(),
            japanese: row.get::<_, String>("japanese").unwrap_or_default(),
            synonyms: split_comma(&row.get::<_, String>("synonym").unwrap_or_default()),
            anime_type: row.get("type").unwrap_or(0),
            status: row.get("status").unwrap_or(0),
            episode_count: row.get("episode_count").unwrap_or(-1),
            image_url: row.get::<_, String>("image").unwrap_or_default(),
            synopsis: row.get::<_, String>("synopsis").unwrap_or_default(),
            score: row.get::<_, f32>("score").unwrap_or(0.0),
            genres: split_comma(&row.get::<_, String>("genres").unwrap_or_default()),
            tags: split_comma(&row.get::<_, String>("tags").unwrap_or_default()),
            last_modified: row.get("modified").unwrap_or(0),
        })
    })?.filter_map(|r| r.ok()).collect();

    // Read anime_list table
    let mut list_stmt = conn.prepare("SELECT * FROM anime_list")?;
    let entries: Vec<V1ListEntry> = list_stmt.query_map([], |row| {
        Ok(V1ListEntry {
            anime_id: row.get("media_id").unwrap_or(0),
            watched_episodes: row.get("progress").unwrap_or(0),
            score: row.get("score").unwrap_or(0),
            status: row.get("status").unwrap_or(0),
            date_started: row.get::<_, String>("date_start").unwrap_or_default(),
            date_completed: row.get::<_, String>("date_end").unwrap_or_default(),
            notes: row.get::<_, String>("notes").unwrap_or_default(),
            last_updated: row.get("last_updated").unwrap_or(0),
            rewatched_times: row.get("rewatched_times").unwrap_or(0),
            rewatching: row.get("rewatching").unwrap_or(false),
            rewatching_ep: row.get("rewatching_ep").unwrap_or(0),
        })
    })?.filter_map(|r| r.ok()).collect();

    Ok((anime, entries))
}

fn split_comma(s: &str) -> Vec<String> {
    s.split(", ").filter(|p| !p.is_empty()).map(String::from).collect()
}

// XML readers for v1 legacy format
pub fn read_v1_history_xml(path: &str) -> Result<Vec<V1HistoryItem>, anyhow::Error> {
    let xml = std::fs::read_to_string(path)?;
    // Parse XML: <history><items><item><anime_id>...</anime_id><episode>...</episode><time>...</time></item>...
    // Use quick-xml for parsing
    todo!("implement XML history reader")
}

pub fn read_v1_anime_xml(path: &str) -> Result<Vec<V1Anime>, anyhow::Error> {
    let xml = std::fs::read_to_string(path)?;
    todo!("implement XML anime reader")
}
```

- [ ] **Step 3: Add Cargo deps**

```toml
rusqlite = { version = "0.31", features = ["bundled"] }
quick-xml = { version = "0.36", features = ["serialize"] }
```

- [ ] **Step 4: Build check + test**

```bash
cd next/src-tauri && cargo check --tests
```

- [ ] **Step 5: Commit**

---

### Task 3: Import Logic (Dry-run + Live with Duplicates)

**Files:**
- Create: `next/src-tauri/src/engine/migration/importer.rs`
- Create: `next/src-tauri/tests/migration_import_test.rs`
- Modify: `next/src-tauri/src/engine/storage.rs` (add migration_log methods)

**Interfaces:**
- Consumes: Task 1 (discovery), Task 2 (v1_read)
- Produces: `DuplicateStrategy { Skip, Merge }`
- Produces: `MigrationReport { imported_anime, imported_entries, imported_history, skipped_anime, skipped_entries, warnings: Vec<MigrationWarning> }`
- Produces: `dry_run_import(paths: &V1DataPaths) -> MigrationReport`
- Produces: `live_import(storage: &Storage, paths: &V1DataPaths, strategy: DuplicateStrategy) -> MigrationReport`

**Steps:**

- [ ] **Step 1: Update MigrationReport + MigrationWarning types in importer.rs**

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MigrationReport {
    pub imported_anime: usize,
    pub imported_entries: usize,
    pub imported_history: usize,
    pub skipped_anime: usize,
    pub skipped_entries: usize,
    pub warnings: Vec<MigrationWarning>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MigrationWarning {
    pub source: String,
    pub source_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DuplicateStrategy {
    Skip,
    Merge,
}
```

- [ ] **Step 2: Implement dry_run_import**

Parses v1 data, builds report without touching v2 DB.

- [ ] **Step 3: Implement live_import**

Writes to v2 DB: inserts anime rows, list_entry rows, watch_history rows. Checks for existing anime_id — skips or merges based on strategy. Logs to migration_log table.

- [ ] **Step 4: Add migration_log storage methods**

```rust
// In storage.rs
pub async fn log_migration(&self, source: &str, source_id: &str, status: &str, message: &str) -> anyhow::Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
    sqlx::query("INSERT INTO migration_log (source, source_id, status, message, created_at) VALUES (?1, ?2, ?3, ?4, ?5)")
        .bind(source).bind(source_id).bind(status).bind(message).bind(now)
        .execute(&self.pool).await?;
    Ok(())
}
```

- [ ] **Step 5: Write tests**

- V1 data → dry run → correct counts
- V1 data → live import → rows exist in v2 DB
- Duplicate anime_id → skipped when Skip strategy
- Empty v1 → zero report

- [ ] **Step 6: Build check + test**

```bash
cd next/src-tauri && cargo test migration_import
```

- [ ] **Step 7: Commit**

---

### Task 4: Backup/Restore + Export/Import

**Files:**
- Create: `next/src-tauri/src/engine/migration/backup.rs`
- Create: `next/src-tauri/tests/migration_backup_test.rs`

**Interfaces:**
- Produces: `backup_database(storage: &Storage) -> anyhow::Result<String>` (returns backup path)
- Produces: `restore_database(storage: &Storage, backup_path: &str) -> anyhow::Result<()>`
- Produces: `export_database(storage: &Storage) -> anyhow::Result<String>` (returns JSON)
- Produces: `import_database(storage: &Storage, json: &str) -> anyhow::Result<MigrationReport>`

**Steps:**

- [ ] **Step 1: Implement backup.rs**

Backup: copy the v2 SQLite file to `{db_path}.backup.{timestamp}`.
Restore: copy backup file over the current DB file (close connections first).
Export: serialize all anime + list_entry + watch_history rows to JSON.
Import: deserialize JSON and upsert into v2 DB.

- [ ] **Step 2: Tests**

- Backup creates file, restore recreates data
- Export JSON contains expected fields
- Import JSON into fresh DB → data matches
- Round-trip: export → import → export → same JSON

- [ ] **Step 3: Commit**

---

### Task 5: Tauri Commands + Registration

**Files:**
- Modify: `next/src-tauri/src/commands.rs`
- Modify: `next/src-tauri/src/lib.rs`

**Commands:**
- `discover_v1_data` → `V1DataPaths`
- `preview_migration` → `MigrationReport` (real, not stub)
- `run_migration` → `MigrationReport`
- `backup_database` → `String` (backup path)
- `restore_database` → `()`
- `export_database` → `String` (JSON)
- `import_database` → `MigrationReport`

**Steps:**

- [ ] **Step 1: Add inner functions to commands.rs**

- [ ] **Step 2: Add Tauri command wrappers**

- [ ] **Step 3: Register in lib.rs generate_handler!**

- [ ] **Step 4: Write command tests `tests/migration_commands_test.rs`**

- [ ] **Step 5: Build check + test**

```bash
cd next/src-tauri && cargo test migration
```

- [ ] **Step 6: Commit**

---

### Task 6: Frontend API Wrappers

**Files:**
- Modify: `next/src/lib/api.ts`
- Modify: `next/src/lib/api.test.ts`

**Functions:**
- `discoverV1Data()` → `V1DataPaths`
- `previewMigration()` → `MigrationReport`
- `runMigration(strategy)` → `MigrationReport`
- `backupDatabase()` → `string`
- `restoreDatabase(path)` → `void`
- `exportDatabase()` → `string`
- `importDatabase(json)` → `MigrationReport`

**Steps:**

- [ ] **Step 1: Add types to api.ts**

- [ ] **Step 2: Add wrapper functions**

- [ ] **Step 3: Add tests to api.test.ts**

- [ ] **Step 4: Run frontend tests**

```bash
cd next && npm run test && npm run check
```

- [ ] **Step 5: Commit**

---

### Task 7: Migration UI (Settings → Migration Tab)

**Files:**
- Modify: `next/src/lib/SettingsView.svelte`
- Create: `next/src/lib/MigrationPanel.svelte` (optional; can inline in SettingsView)

**UI Sections:**
1. **Discover** — button to scan for v1 data, shows found paths
2. **Dry Run** — button to preview import, shows table: anime count, entry count, warnings
3. **Import** — button to run migration (with duplicate strategy dropdown: Skip/Merge), shows results
4. **Backup** — button to backup current DB, shows backup path/timestamp
5. **Restore** — file picker or path input to restore from backup
6. **Export/Import** — export button (downloads JSON), import button (uploads JSON)

**Steps:**

- [ ] **Step 1: Add Migration tab to SettingsView**

- [ ] **Step 2: Implement discover + dry-run flow**

- [ ] **Step 3: Implement import flow**

- [ ] **Step 4: Implement backup/restore flow**

- [ ] **Step 5: Implement export/import flow**

- [ ] **Step 6: Styling and polish (delegate to @designer if needed)**

- [ ] **Step 7: Commit**

---

### Task 8: End-to-End Verification

- [ ] `cd next/src-tauri && cargo check --tests` — clean
- [ ] `cd next && npm run check` — clean
- [ ] `cd next && npm run test` — all pass (existing 35 + new migration tests)
- [ ] `cd next/src-tauri && cargo test` — all pass (in environment where VC++ runtime is available)

---

## Verification Commands

```bash
# Backend
cd next/src-tauri && cargo check --tests
cd next/src-tauri && cargo test migration

# Frontend
cd next && npm run check && npm run test
```
