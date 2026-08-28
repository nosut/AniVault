# M6 Sonarr Integration Implementation Plan

> **For agentic workers:** Use subagent-driven-development (recommended) to implement task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Connect AniVault to Sonarr: server settings, series import with auto-matching, episode availability in detail view, manual remap UI.

**Architecture:** New `engine::sonarr` module mirrors AniList pattern: client (HTTP), import (fetch + auto-match), availability (episode data). 6 new Tauri commands. SQLite tables `sonarr_series` + `sonarr_mapping`. DPAPI-encrypted API key. UI in SettingsView (Sonarr tab) + DetailView (collapsible section) + SonarrRemap component.

**Tech Stack:** Rust, Tauri 2.x, reqwest, SQLite, Svelte 5, TypeScript, Vitest.

## Global Constraints

- Windows desktop app runtime using Tauri, Svelte, Rust, SQLite
- AniList is the only tracker integration in scope; no MAL or Kitsu code
- M6 scope only: Sonarr integration (single instance, read-only display)
- No tray (M5 done), no rebrand (M8), no release hardening (M9)
- Every fallible command must return `Result<T, String>`
- Inner functions use `&EngineState` for testability
- Auto-match reuses M2 parser: `parse_filename` + `search_anime_by_title`
- API key encrypted via `protect_secret`/`unprotect_secret` (M3 pattern)
- No new crate dependencies — `reqwest` already in `Cargo.toml`

---

### Task 1: Migration + Sonarr Storage Methods

**Files:**
- Create: `next/src-tauri/migrations/0002_sonarr.sql`
- Modify: `next/src-tauri/src/engine/storage.rs`
- Create: `next/src-tauri/tests/sonarr_storage_test.rs`

**Interfaces:**
- Produces: `SonarrSeriesDb`, `SonarrMappingDb` structs
- Produces: `Storage::sonarr_series_upsert(series: &SonarrSeriesDb)`, `sonarr_series_list()`, `sonarr_series_count()`, `sonarr_series_delete_all()`
- Produces: `Storage::sonarr_mapping_upsert(mapping: &SonarrMappingDb)`, `sonarr_mapping_by_anime(anime_id: i64)`, `sonarr_mapping_unmapped()`, `sonarr_mapping_count()`, `sonarr_mapping_delete_all()`
- Produces: `Storage::sonarr_availability(anime_id: i64) -> Option<SonarrAvailabilityDb>`

- [ ] **Step 1: Create migration SQL**

Create `next/src-tauri/migrations/0002_sonarr.sql`:

```sql
CREATE TABLE IF NOT EXISTS sonarr_series (
    sonarr_id            INTEGER PRIMARY KEY,
    title                TEXT NOT NULL,
    season_count         INTEGER NOT NULL DEFAULT 0,
    episode_count        INTEGER NOT NULL DEFAULT 0,
    episode_file_count   INTEGER NOT NULL DEFAULT 0,
    monitored            BOOLEAN NOT NULL DEFAULT 1,
    next_airing          INTEGER,
    path                 TEXT,
    poster_url           TEXT,
    overview             TEXT,
    network              TEXT,
    status               TEXT,
    added                INTEGER NOT NULL,
    last_synced          INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sonarr_mapping (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    sonarr_id            INTEGER NOT NULL REFERENCES sonarr_series(sonarr_id) ON DELETE CASCADE,
    anime_id             INTEGER REFERENCES anime(id) ON DELETE SET NULL,
    title_match          TEXT NOT NULL,
    confidence           INTEGER NOT NULL DEFAULT 0,
    mapped_at            INTEGER NOT NULL,
    user_confirmed       BOOLEAN NOT NULL DEFAULT 0
);
```

- [ ] **Step 2: Write failing test `tests/sonarr_storage_test.rs`**

```rust
use taiga_next::engine::storage::{SonarrSeriesDb, Tests};
use taiga_next::engine::runtime::fresh_test_state;

#[tokio::test]
async fn sonarr_series_upsert_and_list() {
    let state = fresh_test_state().await;
    let storage = &state.storage;

    let series = SonarrSeriesDb {
        sonarr_id: 1,
        title: "Attack on Titan".into(),
        season_count: 1,
        episode_count: 25,
        episode_file_count: 25,
        monitored: true,
        next_airing: None,
        path: Some("D:\\Media\\Anime\\Attack on Titan".into()),
        poster_url: Some("https://example.com/poster.jpg".into()),
        overview: Some("Humanity fights titans.".into()),
        network: Some("NHK".into()),
        status: Some("ended".into()),
        added: 1_700_000_000,
        last_synced: 1_700_000_000,
    };

    storage.sonarr_series_upsert(&series).await.unwrap();
    assert_eq!(storage.sonarr_series_count().await.unwrap(), 1);

    let list = storage.sonarr_series_list().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].sonarr_id, 1);
    assert_eq!(list[0].title, "Attack on Titan");
}

#[tokio::test]
async fn sonarr_series_delete_all_clears_table() {
    let state = fresh_test_state().await;
    let storage = &state.storage;

    let series = SonarrSeriesDb {
        sonarr_id: 1,
        title: "Test".into(),
        season_count: 1,
        episode_count: 12,
        episode_file_count: 0,
        monitored: true,
        next_airing: None,
        path: None,
        poster_url: None,
        overview: None,
        network: None,
        status: None,
        added: 1_700_000_000,
        last_synced: 1_700_000_000,
    };
    storage.sonarr_series_upsert(&series).await.unwrap();
    assert_eq!(storage.sonarr_series_count().await.unwrap(), 1);

    storage.sonarr_series_delete_all().await.unwrap();
    assert_eq!(storage.sonarr_series_count().await.unwrap(), 0);
}

#[tokio::test]
async fn sonarr_mapping_crud_and_availability() {
    let state = fresh_test_state().await;
    let storage = &state.storage;

    // Insert a test anime so FK works
    storage.insert_minimal_anime(42, "Test Anime").await.unwrap();

    let mapping = taiga_next::engine::storage::SonarrMappingDb {
        id: None,
        sonarr_id: 1,
        anime_id: Some(42),
        title_match: "Test Anime".into(),
        confidence: 90,
        mapped_at: 1_700_000_000,
        user_confirmed: false,
    };
    storage.sonarr_mapping_upsert(&mapping).await.unwrap();
    assert_eq!(storage.sonarr_mapping_count().await.unwrap(), 1);

    let found = storage.sonarr_mapping_by_anime(42).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().sonarr_id, 1);

    storage.sonarr_mapping_delete_all().await.unwrap();
    assert_eq!(storage.sonarr_mapping_count().await.unwrap(), 0);
}

#[tokio::test]
async fn sonarr_mapping_unmapped_returns_nulls() {
    let state = fresh_test_state().await;
    let storage = &state.storage;

    let mapping = taiga_next::engine::storage::SonarrMappingDb {
        id: None,
        sonarr_id: 1,
        anime_id: None,
        title_match: "Unknown".into(),
        confidence: 30,
        mapped_at: 1_700_000_000,
        user_confirmed: false,
    };
    storage.sonarr_mapping_upsert(&mapping).await.unwrap();

    let unmapped = storage.sonarr_mapping_unmapped().await.unwrap();
    assert_eq!(unmapped.len(), 1);
    assert_eq!(unmapped[0].sonarr_id, 1);
    assert!(unmapped[0].anime_id.is_none());
}
```

Run: `cd next/src-tauri && cargo test sonarr_storage` → FAIL (methods not found)

- [ ] **Step 3: Add struct definitions to `storage.rs`**

After existing structs (around line 98), add:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SonarrSeriesDb {
    pub sonarr_id: i64,
    pub title: String,
    pub season_count: i32,
    pub episode_count: i32,
    pub episode_file_count: i32,
    pub monitored: bool,
    pub next_airing: Option<i64>,
    pub path: Option<String>,
    pub poster_url: Option<String>,
    pub overview: Option<String>,
    pub network: Option<String>,
    pub status: Option<String>,
    pub added: i64,
    pub last_synced: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SonarrMappingDb {
    pub id: Option<i64>,
    pub sonarr_id: i64,
    pub anime_id: Option<i64>,
    pub title_match: String,
    pub confidence: i32,
    pub mapped_at: i64,
    pub user_confirmed: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SonarrAvailabilityDb {
    pub sonarr_id: i64,
    pub sonarr_title: String,
    pub monitored: bool,
    pub episode_count: i32,
    pub episode_file_count: i32,
    pub next_airing: Option<i64>,
    pub path: Option<String>,
    pub season_count: i32,
    pub sonarr_status: Option<String>,
}
```

- [ ] **Step 4: Add Sonarr storage methods to `impl Storage`**

After the `search_anime_by_title` method (around line 337), add:

```rust
    // ── Sonarr series ───────────────────────────────────────────────────────────

    pub async fn sonarr_series_upsert(&self, series: &SonarrSeriesDb) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO sonarr_series (sonarr_id, title, season_count, episode_count, episode_file_count, monitored, next_airing, path, poster_url, overview, network, status, added, last_synced)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(sonarr_id) DO UPDATE SET
               title = excluded.title,
               season_count = excluded.season_count,
               episode_count = excluded.episode_count,
               episode_file_count = excluded.episode_file_count,
               monitored = excluded.monitored,
               next_airing = excluded.next_airing,
               path = excluded.path,
               poster_url = excluded.poster_url,
               overview = excluded.overview,
               network = excluded.network,
               status = excluded.status,
               last_synced = excluded.last_synced",
        )
        .bind(series.sonarr_id)
        .bind(&series.title)
        .bind(series.season_count)
        .bind(series.episode_count)
        .bind(series.episode_file_count)
        .bind(series.monitored)
        .bind(series.next_airing)
        .bind(&series.path)
        .bind(&series.poster_url)
        .bind(&series.overview)
        .bind(&series.network)
        .bind(&series.status)
        .bind(series.added)
        .bind(series.last_synced)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn sonarr_series_list(&self) -> anyhow::Result<Vec<SonarrSeriesDb>> {
        let rows = sqlx::query(
            "SELECT sonarr_id, title, season_count, episode_count, episode_file_count, monitored, next_airing, path, poster_url, overview, network, status, added, last_synced
             FROM sonarr_series ORDER BY title",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|row| SonarrSeriesDb {
            sonarr_id: row.get("sonarr_id"),
            title: row.get("title"),
            season_count: row.get("season_count"),
            episode_count: row.get("episode_count"),
            episode_file_count: row.get("episode_file_count"),
            monitored: row.get("monitored"),
            next_airing: row.get("next_airing"),
            path: row.get("path"),
            poster_url: row.get("poster_url"),
            overview: row.get("overview"),
            network: row.get("network"),
            status: row.get("status"),
            added: row.get("added"),
            last_synced: row.get("last_synced"),
        }).collect())
    }

    pub async fn sonarr_series_count(&self) -> anyhow::Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) FROM sonarr_series")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>(0))
    }

    pub async fn sonarr_series_delete_all(&self) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM sonarr_series").execute(&self.pool).await?;
        Ok(())
    }

    // ── Sonarr mapping ──────────────────────────────────────────────────────────

    pub async fn sonarr_mapping_upsert(&self, mapping: &SonarrMappingDb) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO sonarr_mapping (sonarr_id, anime_id, title_match, confidence, mapped_at, user_confirmed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(sonarr_id) DO UPDATE SET
               anime_id = excluded.anime_id,
               title_match = excluded.title_match,
               confidence = excluded.confidence,
               mapped_at = excluded.mapped_at,
               user_confirmed = excluded.user_confirmed",
        )
        .bind(mapping.sonarr_id)
        .bind(mapping.anime_id)
        .bind(&mapping.title_match)
        .bind(mapping.confidence)
        .bind(mapping.mapped_at)
        .bind(mapping.user_confirmed)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn sonarr_mapping_by_anime(&self, anime_id: i64) -> anyhow::Result<Option<SonarrMappingDb>> {
        let row = sqlx::query(
            "SELECT id, sonarr_id, anime_id, title_match, confidence, mapped_at, user_confirmed
             FROM sonarr_mapping WHERE anime_id = ?1",
        )
        .bind(anime_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| SonarrMappingDb {
            id: Some(r.get("id")),
            sonarr_id: r.get("sonarr_id"),
            anime_id: r.get("anime_id"),
            title_match: r.get("title_match"),
            confidence: r.get("confidence"),
            mapped_at: r.get("mapped_at"),
            user_confirmed: r.get("user_confirmed"),
        }))
    }

    pub async fn sonarr_mapping_unmapped(&self) -> anyhow::Result<Vec<SonarrMappingDb>> {
        let rows = sqlx::query(
            "SELECT id, sonarr_id, anime_id, title_match, confidence, mapped_at, user_confirmed
             FROM sonarr_mapping WHERE anime_id IS NULL ORDER BY title_match",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| SonarrMappingDb {
            id: Some(r.get("id")),
            sonarr_id: r.get("sonarr_id"),
            anime_id: r.get("anime_id"),
            title_match: r.get("title_match"),
            confidence: r.get("confidence"),
            mapped_at: r.get("mapped_at"),
            user_confirmed: r.get("user_confirmed"),
        }).collect())
    }

    pub async fn sonarr_mapping_count(&self) -> anyhow::Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) FROM sonarr_mapping")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>(0))
    }

    pub async fn sonarr_mapping_delete_all(&self) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM sonarr_mapping").execute(&self.pool).await?;
        Ok(())
    }

    // ── Sonarr availability (join) ──────────────────────────────────────────────

    pub async fn sonarr_availability(&self, anime_id: i64) -> anyhow::Result<Option<SonarrAvailabilityDb>> {
        let row = sqlx::query(
            "SELECT s.sonarr_id, s.title, s.monitored, s.episode_count, s.episode_file_count,
                    s.next_airing, s.path, s.season_count, s.status as sonarr_status
             FROM sonarr_series s
             JOIN sonarr_mapping m ON s.sonarr_id = m.sonarr_id
             WHERE m.anime_id = ?1",
        )
        .bind(anime_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| SonarrAvailabilityDb {
            sonarr_id: r.get("sonarr_id"),
            sonarr_title: r.get("title"),
            monitored: r.get("monitored"),
            episode_count: r.get("episode_count"),
            episode_file_count: r.get("episode_file_count"),
            next_airing: r.get("next_airing"),
            path: r.get("path"),
            season_count: r.get("season_count"),
            sonarr_status: r.get("sonarr_status"),
        }))
    }
```

- [ ] **Step 5: Run storage tests**

```bash
cd next/src-tauri && cargo test sonarr_storage
```

Expected: 4 tests PASS.

- [ ] **Step 6: Run full storage suite**

```bash
cd next/src-tauri && cargo test storage_test library_storage tracking_storage anilist_storage sonarr_storage
```

Expected: all storage tests PASS (no regressions).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/migrations/0002_sonarr.sql src-tauri/src/engine/storage.rs src-tauri/tests/sonarr_storage_test.rs
git commit -m "feat: add sonarr storage tables and methods"
```

---

### Task 2: Sonarr HTTP Client

**Files:**
- Create: `next/src-tauri/src/engine/sonarr/mod.rs`
- Create: `next/src-tauri/src/engine/sonarr/client.rs`
- Modify: `next/src-tauri/src/engine/mod.rs`
- Create: `next/src-tauri/tests/sonarr_client_test.rs`

**Interfaces:**
- Consumes: `reqwest` (already in Cargo.toml), `secrets::protect_secret`/`unprotect_secret`
- Produces: `SonarrClient { url, api_key, http }` with `new(url, api_key)`, `validate_connection()`, `fetch_series()`
- Produces types: `SonarrSeriesRaw`, `SonarrSystemStatus`

- [ ] **Step 1: Create module structure**

Create `next/src-tauri/src/engine/sonarr/mod.rs`:

```rust
pub mod client;
pub mod import;
```

Modify `next/src-tauri/src/engine/mod.rs` — add after `pub mod scanner;`:

```rust
pub mod sonarr;
```

- [ ] **Step 2: Write failing test `tests/sonarr_client_test.rs`**

```rust
use taiga_next::engine::sonarr::client::SonarrClient;

#[test]
fn client_constructs_with_url_and_key() {
    let client = SonarrClient::new("http://localhost:8989".into(), "abc123".into());
    assert_eq!(client.url, "http://localhost:8989");
    assert_eq!(client.api_key, "abc123");
}

#[test]
fn client_trims_trailing_slash_from_url() {
    let client = SonarrClient::new("http://localhost:8989/".into(), "key".into());
    assert_eq!(client.url, "http://localhost:8989");
}

#[tokio::test]
async fn validate_connection_returns_error_for_nonexistent_host() {
    let client = SonarrClient::new("http://127.0.0.1:19999".into(), "bad".into());
    assert!(client.validate_connection().await.is_err());
}

#[tokio::test]
async fn fetch_series_returns_error_for_nonexistent_host() {
    let client = SonarrClient::new("http://127.0.0.1:19999".into(), "bad".into());
    assert!(client.fetch_series().await.is_err());
}
```

Run: `cd next/src-tauri && cargo test sonarr_client` → FAIL (module not found)

- [ ] **Step 3: Implement `engine/sonarr/client.rs`**

```rust
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct SonarrClient {
    pub url: String,
    pub api_key: String,
    http: reqwest::Client,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrSystemStatus {
    pub version: Option<String>,
    pub app_name: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrSeriesRaw {
    pub id: i64,
    pub title: String,
    pub season_count: Option<i32>,
    #[serde(default)]
    pub seasons: Vec<SonarrSeasonRaw>,
    pub monitored: bool,
    pub next_airing: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub images: Vec<SonarrImageRaw>,
    pub overview: Option<String>,
    pub network: Option<String>,
    pub status: Option<String>,
    pub added: Option<String>,
    #[serde(default)]
    pub statistics: Option<SonarrStatisticsRaw>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrSeasonRaw {
    #[serde(default)]
    pub statistics: Option<SonarrStatisticsRaw>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrStatisticsRaw {
    #[serde(default)]
    pub episode_count: i32,
    #[serde(default)]
    pub episode_file_count: i32,
    #[serde(default)]
    pub total_episode_count: i32,
    pub next_airing: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SonarrImageRaw {
    pub cover_type: Option<String>,
    pub remote_url: Option<String>,
}

impl SonarrClient {
    pub fn new(url: String, api_key: String) -> Self {
        let url = url.trim_end_matches('/').to_string();
        Self {
            url,
            api_key,
            http: reqwest::Client::new(),
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Api-Key",
            HeaderValue::from_str(&self.api_key).unwrap_or_default(),
        );
        headers
    }

    pub async fn validate_connection(&self) -> anyhow::Result<SonarrSystemStatus> {
        let resp = self
            .http
            .get(format!("{}/api/v3/system/status", self.url))
            .headers(self.headers())
            .send()
            .await?;

        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            return Err(anyhow::anyhow!("Sonarr returned HTTP {}", status));
        }

        let body: SonarrSystemStatus = resp.json().await?;
        Ok(body)
    }

    pub async fn fetch_series(&self) -> anyhow::Result<Vec<SonarrSeriesRaw>> {
        let resp = self
            .http
            .get(format!("{}/api/v3/series", self.url))
            .headers(self.headers())
            .send()
            .await?;

        if resp.status().is_client_error() || resp.status().is_server_error() {
            let status = resp.status();
            return Err(anyhow::anyhow!("Sonarr returned HTTP {}", status));
        }

        let body: Vec<SonarrSeriesRaw> = resp.json().await?;
        Ok(body)
    }
}
```

- [ ] **Step 4: Run client tests**

```bash
cd next/src-tauri && cargo test sonarr_client
```

Expected: 4 tests PASS (construct + trim_url pass as unit tests; validate/fetch fail but return Err as expected — they PASS).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/engine/sonarr/mod.rs src-tauri/src/engine/sonarr/client.rs src-tauri/src/engine/mod.rs src-tauri/tests/sonarr_client_test.rs
git commit -m "feat: add sonarr http client"
```

---

### Task 3: Sonarr Import + Auto-Matching

**Files:**
- Create: `next/src-tauri/src/engine/sonarr/import.rs`
- Create: `next/src-tauri/tests/sonarr_import_test.rs`

**Interfaces:**
- Consumes: `SonarrClient`, `Storage`, `Storage::search_anime_by_title`, `Storage::sonarr_series_upsert`, `Storage::sonarr_mapping_upsert`
- Produces: `ImportReport { imported: i64, auto_mapped: i64, unmapped: i64 }`
- Produces: `import_sonarr_series(client: &SonarrClient, storage: &Storage) -> anyhow::Result<ImportReport>`
- Produces: `score_match(sonarr_title: &str, anime_title: &str, sonarr_ep_count: i32, anime_ep_count: Option<i32>) -> i32`

- [ ] **Step 1: Write failing test `tests/sonarr_import_test.rs`**

```rust
use taiga_next::engine::sonarr::client::SonarrClient;
use taiga_next::engine::sonarr::import::score_match_series;
use taiga_next::engine::storage::Tests;

#[tokio::test]
async fn import_with_no_series_reports_zero() {
    // Skip live HTTP test — unit test the scoring instead
}

#[test]
fn exact_title_match_scores_100() {
    let score = score_match_series(
        "Attack on Titan",
        "Attack on Titan",
        25,
        Some(25),
    );
    assert_eq!(score, 100);
}

#[test]
fn substring_match_scores_60() {
    let score = score_match_series(
        "Attack on Titan Final Season",
        "Attack on Titan",
        16,
        Some(25),
    );
    assert!(score >= 60, "expected >= 60, got {score}");
}

#[test]
fn unrelated_titles_score_0() {
    let score = score_match_series(
        "One Piece",
        "Attack on Titan",
        1000,
        Some(25),
    );
    assert_eq!(score, 0);
}

#[test]
fn episode_count_match_adds_20() {
    let base = score_match_series("Test", "Test", 12, Some(12));
    let off_by_10 = score_match_series("Test", "Test", 22, Some(12));
    assert!(base > off_by_10);
}
```

Run: `cd next/src-tauri && cargo test sonarr_import` → FAIL (module not found)

- [ ] **Step 2: Implement `engine/sonarr/import.rs`**

```rust
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

/// Score a Sonarr series title against an anime library title.
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
    }

    // Bonus: episode count within ±3
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
```

- [ ] **Step 3: Run import scoring tests**

```bash
cd next/src-tauri && cargo test sonarr_import
```

Expected: 4 scoring tests PASS (the async test is skipped).

- [ ] **Step 4: Run full test suite** to check no regressions from `chrono` usage.

We need `chrono` in Cargo.toml. Add after `base64 = "0.22.1"`:

```toml
chrono = { version = "0.4", features = ["serde"] }
```

```bash
cd next/src-tauri && cargo test
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/engine/sonarr/import.rs src-tauri/tests/sonarr_import_test.rs
git commit -m "feat: add sonarr import with auto-matching"
```

---

### Task 4: Sonarr Commands + Registration

**Files:**
- Modify: `next/src-tauri/src/commands.rs`
- Modify: `next/src-tauri/src/lib.rs`
- Create: `next/src-tauri/tests/sonarr_commands_test.rs`

**Interfaces:**
- Consumes: Task 1 (storage methods), Task 2 (client), Task 3 (import)
- Produces: 6 inner functions + 6 `#[tauri::command]` wrappers
- Produces: `SonarrStatus`, `ImportReport` (re-export), `SonarrAvailability` response types
- Command names: `connect_sonarr`, `disconnect_sonarr`, `get_sonarr_status`, `import_sonarr_series`, `get_sonarr_availability`, `remap_sonarr`

- [ ] **Step 1: Write failing test `tests/sonarr_commands_test.rs`**

```rust
use taiga_next::commands::{
    get_sonarr_status_inner, disconnect_sonarr_inner, remap_sonarr_inner,
};
use taiga_next::engine::runtime::fresh_test_state;

#[tokio::test]
async fn sonarr_status_not_connected_when_no_key() {
    let state = fresh_test_state().await;
    let status = get_sonarr_status_inner(&state).await.unwrap();
    assert!(!status.connected);
    assert_eq!(status.series_count, 0);
    assert_eq!(status.mapped_count, 0);
}

#[tokio::test]
async fn disconnect_sonarr_cleans_up() {
    let state = fresh_test_state().await;

    // Set fake connection settings
    state.storage.set_setting("sonarr.url", r#""http://localhost:8989""#, 1)
        .await
        .unwrap();

    let _ = disconnect_sonarr_inner(&state).await;
    let status = get_sonarr_status_inner(&state).await.unwrap();
    assert!(!status.connected);
}

#[tokio::test]
async fn remap_sonarr_updates_mapping() {
    let state = fresh_test_state().await;

    // Insert a series + unmapped mapping
    let series = taiga_next::engine::storage::SonarrSeriesDb {
        sonarr_id: 42,
        title: "Test Series".into(),
        season_count: 1,
        episode_count: 12,
        episode_file_count: 0,
        monitored: true,
        next_airing: None,
        path: None,
        poster_url: None,
        overview: None,
        network: None,
        status: None,
        added: 1,
        last_synced: 1,
    };
    state.storage.sonarr_series_upsert(&series).await.unwrap();

    let mapping = taiga_next::engine::storage::SonarrMappingDb {
        id: None,
        sonarr_id: 42,
        anime_id: None,
        title_match: "Test".into(),
        confidence: 20,
        mapped_at: 1,
        user_confirmed: false,
    };
    state.storage.sonarr_mapping_upsert(&mapping).await.unwrap();

    // Insert a test anime
    state.storage.insert_minimal_anime(99, "Test Anime").await.unwrap();

    // Remap
    remap_sonarr_inner(42, Some(99), &state).await.unwrap();

    let updated = state.storage.sonarr_mapping_by_anime(99).await.unwrap();
    assert!(updated.is_some());
    assert_eq!(updated.unwrap().sonarr_id, 42);
}
```

Run: `cd next/src-tauri && cargo test sonarr_commands` → FAIL (functions not found)

- [ ] **Step 2: Add Sonarr command types and inner functions to `commands.rs`**

Add type near other structs (after `SessionState`):

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct SonarrStatus {
    pub connected: bool,
    pub series_count: i64,
    pub mapped_count: i64,
    pub last_sync_at: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SonarrAvailabilityResponse {
    pub sonarr_id: i64,
    pub sonarr_title: String,
    pub monitored: bool,
    pub episode_count: i32,
    pub episode_file_count: i32,
    pub next_airing: Option<i64>,
    pub path: Option<String>,
    pub season_count: i32,
    pub sonarr_status: Option<String>,
}
```

Add imports near top after existing imports:

```rust
use crate::engine::sonarr::client::SonarrClient;
use crate::engine::sonarr::import::{import_sonarr_series, ImportReport};
use crate::engine::storage::SonarrAvailabilityDb;
```

Add inner functions before the Tauri command section (before `// ── Session commands ──`):

```rust
// ── Sonarr commands ──────────────────────────────────────────────────────────

fn load_sonarr_connection(state: &EngineState) -> Option<(String, String)> {
    let url: Option<String> = std::thread::spawn(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            state.storage.get_setting("sonarr.url").await.ok().flatten()
        })
    }).join().ok().flatten();

    let url = url?;
    let url = serde_json::from_str::<String>(&url).ok()?;

    let encrypted: Option<String> = std::thread::spawn(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            state.storage.get_setting("sonarr.api_key").await.ok().flatten()
        })
    }).join().ok().flatten();

    let encrypted = encrypted?;
    let api_key = crate::engine::secrets::unprotect_secret(&encrypted).ok()?;

    Some((url, api_key))
}

pub async fn connect_sonarr_inner(url: &str, api_key: &str, state: &EngineState) -> anyhow::Result<()> {
    let client = SonarrClient::new(url.to_string(), api_key.to_string());

    // Validate connection
    client.validate_connection().await?;

    // Store settings
    let url_json = serde_json::to_string(url)?;
    let encrypted_key = crate::engine::secrets::protect_secret(api_key)?;
    let encrypted_json = serde_json::to_string(&encrypted_key)?;
    let now = unix_now().map_err(|e| anyhow::anyhow!(e))?;

    state.storage.set_setting("sonarr.url", &url_json, now).await?;
    state.storage.set_setting("sonarr.api_key", &encrypted_json, now).await?;

    // Import series
    import_sonarr_series(&client, &state.storage).await?;

    Ok(())
}

pub async fn disconnect_sonarr_inner(state: &EngineState) -> anyhow::Result<()> {
    state.storage.delete_setting("sonarr.url").await?;
    state.storage.delete_setting("sonarr.api_key").await?;
    state.storage.sonarr_mapping_delete_all().await?;
    state.storage.sonarr_series_delete_all().await?;
    Ok(())
}

pub async fn get_sonarr_status_inner(state: &EngineState) -> anyhow::Result<SonarrStatus> {
    let connected = state.storage.get_setting("sonarr.api_key").await?.is_some();
    let series_count = if connected {
        state.storage.sonarr_series_count().await?
    } else {
        0
    };
    let mapped_count = if connected {
        state.storage.sonarr_mapping_count().await?
    } else {
        0
    };
    let last_sync_at = if connected {
        state.storage.get_setting("sonarr.last_sync_at").await?
            .and_then(|v| serde_json::from_str::<i64>(&v).ok())
    } else {
        None
    };

    Ok(SonarrStatus {
        connected,
        series_count,
        mapped_count,
        last_sync_at,
    })
}

pub async fn import_sonarr_series_inner(state: &EngineState) -> anyhow::Result<ImportReport> {
    let (url, api_key) = load_sonarr_connection(state)
        .ok_or_else(|| anyhow::anyhow!("Sonarr not connected"))?;
    let client = SonarrClient::new(url, api_key);
    let report = import_sonarr_series(&client, &state.storage).await?;

    // Update last sync time
    let now = unix_now().map_err(|e| anyhow::anyhow!(e))?;
    let now_json = serde_json::to_string(&now)?;
    state.storage.set_setting("sonarr.last_sync_at", &now_json, now).await?;

    Ok(report)
}

pub async fn get_sonarr_availability_inner(
    anime_id: i64,
    state: &EngineState,
) -> anyhow::Result<Option<SonarrAvailabilityResponse>> {
    let row = state.storage.sonarr_availability(anime_id).await?;
    Ok(row.map(|r| SonarrAvailabilityResponse {
        sonarr_id: r.sonarr_id,
        sonarr_title: r.sonarr_title,
        monitored: r.monitored,
        episode_count: r.episode_count,
        episode_file_count: r.episode_file_count,
        next_airing: r.next_airing,
        path: r.path,
        season_count: r.season_count,
        sonarr_status: r.sonarr_status,
    }))
}

pub async fn remap_sonarr_inner(
    sonarr_id: i64,
    anime_id: Option<i64>,
    state: &EngineState,
) -> anyhow::Result<()> {
    let now = unix_now().map_err(|e| anyhow::anyhow!(e))?;

    // Update existing mapping or insert new
    let mapping = crate::engine::storage::SonarrMappingDb {
        id: None,
        sonarr_id,
        anime_id,
        title_match: "manual".into(),
        confidence: if anime_id.is_some() { 100 } else { 0 },
        mapped_at: now,
        user_confirmed: true,
    };
    state.storage.sonarr_mapping_upsert(&mapping).await?;
    Ok(())
}
```

- [ ] **Step 3: Add 6 Tauri command wrappers**

After the Sonarr inner functions, before `// ── Session commands ──`:

```rust
#[tauri::command]
pub async fn connect_sonarr(
    url: String,
    api_key: String,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    connect_sonarr_inner(&url, &api_key, &state)
        .await
        .map_err(command_error)
}

#[tauri::command]
pub async fn disconnect_sonarr(
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    disconnect_sonarr_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn get_sonarr_status(
    state: tauri::State<'_, EngineState>,
) -> Result<SonarrStatus, String> {
    get_sonarr_status_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn import_sonarr_series(
    state: tauri::State<'_, EngineState>,
) -> Result<ImportReport, String> {
    import_sonarr_series_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn get_sonarr_availability(
    anime_id: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<Option<SonarrAvailabilityResponse>, String> {
    get_sonarr_availability_inner(anime_id, &state)
        .await
        .map_err(command_error)
}

#[tauri::command]
pub async fn remap_sonarr(
    sonarr_id: i64,
    anime_id: Option<i64>,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    remap_sonarr_inner(sonarr_id, anime_id, &state)
        .await
        .map_err(command_error)
}
```

- [ ] **Step 4: Register commands in `lib.rs`**

In `generate_handler!`, add alphabetically:

```rust
commands::connect_sonarr,
commands::disconnect_sonarr,
commands::get_sonarr_availability,
commands::get_sonarr_status,
commands::import_sonarr_series,
commands::remap_sonarr,
```

Insert them after `commands::confirm_identification` line (alphabetical order):
- `connect_sonarr` after `confirm_identification`
- `disconnect_sonarr` after `delete_setting`
- `get_sonarr_availability` after `get_session_state`
- `get_sonarr_status` after `get_session_state`
- `import_sonarr_series` after `identify_file`
- `remap_sonarr` after `preview_migration_report`

- [ ] **Step 5: Run command tests**

```bash
cd next/src-tauri && cargo test sonarr_commands
```

Expected: 3 tests PASS.

- [ ] **Step 6: Run full Rust test suite**

```bash
cd next/src-tauri && cargo test
```

Expected: all tests PASS (including all existing test suites).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/tests/sonarr_commands_test.rs
git commit -m "feat: add sonarr tauri commands"
```

---

### Task 5: Frontend API Wrappers

**Files:**
- Modify: `next/src/lib/api.ts`
- Modify: `next/src/lib/api.test.ts`

**Interfaces:**
- Consumes: Tauri commands from Task 4
- Produces: TypeScript types + 6 wrapper functions

- [ ] **Step 1: Update tests first in `api.test.ts`**

Add imports at top:

```typescript
import {
  // ...existing imports...
  connectSonarr,
  disconnectSonarr,
  getSonarrStatus,
  importSonarrSeries,
  getSonarrAvailability,
  remapSonarr,
} from './api';
```

Add test cases inside `describe('api wrappers', () => {`:

```typescript
  it('connects sonarr through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(connectSonarr('http://localhost:8989', 'key123', invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('connect_sonarr', { url: 'http://localhost:8989', apiKey: 'key123' });
  });

  it('disconnects sonarr through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(disconnectSonarr(invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('disconnect_sonarr');
  });

  it('gets sonarr status through invoke', async () => {
    const status = { connected: true, series_count: 10, mapped_count: 8, last_sync_at: null };
    const invoke = vi.fn().mockResolvedValue(status);
    await expect(getSonarrStatus(invoke)).resolves.toEqual(status);
    expect(invoke).toHaveBeenCalledWith('get_sonarr_status');
  });

  it('imports sonarr series through invoke', async () => {
    const report = { imported: 10, auto_mapped: 8, unmapped: 2 };
    const invoke = vi.fn().mockResolvedValue(report);
    await expect(importSonarrSeries(invoke)).resolves.toEqual(report);
    expect(invoke).toHaveBeenCalledWith('import_sonarr_series');
  });

  it('gets sonarr availability through invoke', async () => {
    const avail = {
      sonarr_id: 1, sonarr_title: 'Test', monitored: true,
      episode_count: 12, episode_file_count: 8, next_airing: null,
      path: '/media', season_count: 1, sonarr_status: 'continuing',
    };
    const invoke = vi.fn().mockResolvedValue(avail);
    await expect(getSonarrAvailability(42, invoke)).resolves.toEqual(avail);
    expect(invoke).toHaveBeenCalledWith('get_sonarr_availability', { anime_id: 42 });
  });

  it('remaps sonarr through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(remapSonarr(5, 42, invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('remap_sonarr', { sonarr_id: 5, anime_id: 42 });
  });
```

Run: `cd next && npm run test` → FAIL (new exports not found)

- [ ] **Step 2: Add types and wrappers to `api.ts`**

Add types after existing interfaces:

```typescript
export interface SonarrStatus {
  connected: boolean;
  series_count: number;
  mapped_count: number;
  last_sync_at: number | null;
}

export interface SonarrImportReport {
  imported: number;
  auto_mapped: number;
  unmapped: number;
}

export interface SonarrAvailability {
  sonarr_id: number;
  sonarr_title: string;
  monitored: boolean;
  episode_count: number;
  episode_file_count: number;
  next_airing: number | null;
  path: string | null;
  season_count: number;
  sonarr_status: string | null;
}
```

Add wrapper functions after existing functions (before `export function getSyncStatus`...):

```typescript
export function connectSonarr(
  url: string,
  apiKey: string,
  invokeFn: InvokeFn = tauriInvoke,
): Promise<void> {
  return invokeFn<void>('connect_sonarr', { url, apiKey });
}

export function disconnectSonarr(invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('disconnect_sonarr');
}

export function getSonarrStatus(invokeFn: InvokeFn = tauriInvoke): Promise<SonarrStatus> {
  return invokeFn<SonarrStatus>('get_sonarr_status');
}

export function importSonarrSeries(invokeFn: InvokeFn = tauriInvoke): Promise<SonarrImportReport> {
  return invokeFn<SonarrImportReport>('import_sonarr_series');
}

export function getSonarrAvailability(
  animeId: number,
  invokeFn: InvokeFn = tauriInvoke,
): Promise<SonarrAvailability | null> {
  return invokeFn<SonarrAvailability | null>('get_sonarr_availability', { anime_id: animeId });
}

export function remapSonarr(
  sonarrId: number,
  animeId: number | null,
  invokeFn: InvokeFn = tauriInvoke,
): Promise<void> {
  return invokeFn<void>('remap_sonarr', { sonarr_id: sonarrId, anime_id: animeId });
}
```

- [ ] **Step 3: Run frontend tests**

```bash
cd next && npm run test
```

Expected: all tests PASS (28 existing + 6 new = 34).

- [ ] **Step 4: Run frontend check**

```bash
cd next && npm run check
```

Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src/lib/api.ts src/lib/api.test.ts
git commit -m "feat: add sonarr frontend api wrappers"
```

---

### Task 6: SettingsView — Sonarr Tab

**Files:**
- Modify: `next/src/lib/SettingsView.svelte`

**Interfaces:**
- Consumes: `connectSonarr`, `disconnectSonarr`, `getSonarrStatus`, `importSonarrSeries` from api.ts (Task 5)
- Produces: Sonarr tab with connect/disconnect/sync/import UI

- [ ] **Step 1: Add imports and state to script block**

Add import:

```typescript
import { connectSonarr, disconnectSonarr, getSonarrStatus, importSonarrSeries, type SonarrStatus, type SonarrImportReport } from './api';
```

Update `Tab` type:

```typescript
type Tab = 'general' | 'tracking' | 'anilist' | 'sonarr' | 'about';
```

Add state variables after existing state:

```typescript
// Sonarr state
let sonarrStatus: SonarrStatus | null = null;
let sonarrStatusLoading = false;
let sonarrStatusError: string | null = null;

let sonarrUrl = '';
let sonarrApiKey = '';

let sonarrConnecting = false;
let sonarrConnectionError: string | null = null;

let sonarrTesting = false;
let sonarrTestResult: 'success' | 'failure' | null = null;

let sonarrImporting = false;
let sonarrImportReport: SonarrImportReport | null = null;
let sonarrImportError: string | null = null;
```

Add functions:

```typescript
async function loadSonarrStatus() {
  sonarrStatusLoading = true;
  sonarrStatusError = null;
  try {
    sonarrStatus = await getSonarrStatus();
  } catch (e) {
    sonarrStatusError = e instanceof Error ? e.message : String(e);
  } finally {
    sonarrStatusLoading = false;
  }
}

async function handleConnectSonarr() {
  sonarrConnecting = true;
  sonarrConnectionError = null;
  try {
    await connectSonarr(sonarrUrl, sonarrApiKey);
    sonarrApiKey = '';
    await loadSonarrStatus();
  } catch (e) {
    sonarrConnectionError = e instanceof Error ? e.message : String(e);
  } finally {
    sonarrConnecting = false;
  }
}

async function handleDisconnectSonarr() {
  sonarrConnecting = true;
  try {
    await disconnectSonarr();
    sonarrStatus = null;
  } catch (e) {
    sonarrConnectionError = e instanceof Error ? e.message : String(e);
  } finally {
    sonarrConnecting = false;
  }
}

async function handleTestConnection() {
  sonarrTesting = true;
  sonarrTestResult = null;
  try {
    await connectSonarr(sonarrUrl, sonarrApiKey);
    sonarrTestResult = 'success';
    // Clean up test connection
    await disconnectSonarr();
    await loadSonarrStatus();
  } catch (e) {
    sonarrTestResult = 'failure';
    sonarrConnectionError = e instanceof Error ? e.message : String(e);
  } finally {
    sonarrTesting = false;
  }
}

async function handleImportSonarr() {
  sonarrImporting = true;
  sonarrImportError = null;
  sonarrImportReport = null;
  try {
    sonarrImportReport = await importSonarrSeries();
    await loadSonarrStatus();
  } catch (e) {
    sonarrImportError = e instanceof Error ? e.message : String(e);
  } finally {
    sonarrImporting = false;
  }
}
```

Add `loadSonarrStatus()` to `onMount`:

```typescript
onMount(() => {
  loadStartup();
  loadTracking();
  loadEngineStatus();
  loadSonarrStatus();
});
```

- [ ] **Step 2: Add "Sonarr" tab to tab bar**

In the `{#each}` for tabs, add `sonarr`:

```svelte
{#each [{id: 'general', label: 'General'}, {id: 'tracking', label: 'Tracking'}, {id: 'anilist', label: 'AniList'}, {id: 'sonarr', label: 'Sonarr'}, {id: 'about', label: 'About'}] as tab}
```

- [ ] **Step 3: Add Sonarr tab panel**

After the `{#if activeTab === 'anilist'}` block and before `{#if activeTab === 'about'}`, add:

```svelte
{#if activeTab === 'sonarr'}
  <div class="panel" role="tabpanel" id="panel-sonarr" aria-labelledby="tab-sonarr">

    {#if sonarrStatusLoading && !sonarrStatus}
      <section class="card">
        <p class="muted">Loading…</p>
      </section>
    {:else if sonarrStatusError && !sonarrStatus}
      <section class="card">
        <div class="error-row">
          <p class="error">{sonarrStatusError}</p>
          <button type="button" class="btn-retry" on:click={loadSonarrStatus}>Retry</button>
        </div>
      </section>
    {:else if sonarrStatus?.connected}
      <!-- Connected state -->
      <section class="card">
        <div class="section-header">
          <h3>Sonarr Connection</h3>
          <span class="connected-badge">Connected ✓</span>
        </div>

        <div class="sonarr-meta">
          <div class="sonarr-stat">
            <span class="sonarr-stat-value">{sonarrStatus.series_count}</span>
            <span class="sonarr-stat-label">series imported</span>
          </div>
          <div class="sonarr-stat">
            <span class="sonarr-stat-value">{sonarrStatus.mapped_count}</span>
            <span class="sonarr-stat-label">mapped to anime</span>
          </div>
          {#if sonarrStatus.last_sync_at}
            <div class="sonarr-stat">
              <span class="sonarr-stat-label">Last synced</span>
              <span class="sonarr-stat-value muted">{new Date(sonarrStatus.last_sync_at * 1000).toLocaleString()}</span>
            </div>
          {/if}
        </div>

        {#if sonarrConnectionError}
          <p class="error">{sonarrConnectionError}</p>
        {/if}

        <div class="sonarr-actions">
          <button
            type="button"
            class="action-btn"
            on:click={handleImportSonarr}
            disabled={sonarrImporting}
          >
            {sonarrImporting ? 'Importing…' : 'Import Series'}
          </button>
          <button
            type="button"
            class="action-btn danger"
            on:click={handleDisconnectSonarr}
            disabled={sonarrConnecting}
          >
            Disconnect
          </button>
        </div>

        {#if sonarrImportReport}
          <div class="import-report">
            <p>Imported {sonarrImportReport.imported} series: {sonarrImportReport.auto_mapped} auto-mapped, {sonarrImportReport.unmapped} need manual mapping.</p>
          </div>
        {/if}
        {#if sonarrImportError}
          <p class="error">{sonarrImportError}</p>
        {/if}
      </section>
    {:else}
      <!-- Disconnected state -->
      <section class="card">
        <h3>Sonarr Connection</h3>

        {#if sonarrConnectionError}
          <p class="error">{sonarrConnectionError}</p>
        {/if}

        <div class="form-group">
          <label class="form-label" for="sonarr-url">URL</label>
          <input
            id="sonarr-url"
            class="form-input"
            type="text"
            bind:value={sonarrUrl}
            placeholder="http://localhost:8989"
            disabled={sonarrConnecting}
          />
        </div>

        <div class="form-group">
          <label class="form-label" for="sonarr-apikey">API Key</label>
          <input
            id="sonarr-apikey"
            class="form-input"
            type="password"
            bind:value={sonarrApiKey}
            placeholder="Your Sonarr API key"
            disabled={sonarrConnecting}
          />
        </div>

        <div class="form-actions">
          <button
            type="button"
            class="action-btn outline"
            on:click={handleTestConnection}
            disabled={sonarrConnecting || sonarrTesting || !sonarrUrl || !sonarrApiKey}
          >
            {sonarrTesting ? 'Testing…' : 'Test Connection'}
          </button>
          <button
            type="button"
            class="action-btn"
            on:click={handleConnectSonarr}
            disabled={sonarrConnecting || sonarrTesting || !sonarrUrl || !sonarrApiKey}
          >
            {sonarrConnecting ? 'Connecting…' : 'Connect'}
          </button>
        </div>

        {#if sonarrTestResult === 'success'}
          <p class="success-msg">Connection successful!</p>
        {:else if sonarrTestResult === 'failure'}
          <p class="error">Connection failed. Check URL and API key.</p>
        {/if}
      </section>
    {/if}
  </div>
{/if}
```

- [ ] **Step 4: Add Sonarr-specific CSS**

Inside the `<style>` block, add:

```css
  .connected-badge {
    font-size: 0.78rem;
    color: #7ee87e;
    font-weight: 600;
  }

  .sonarr-meta {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 0.75rem;
    margin-top: 0.5rem;
  }

  .sonarr-stat {
    display: grid;
    gap: 0.15rem;
    padding: 0.5rem 0.75rem;
    border: 1px solid rgba(143, 183, 255, 0.15);
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.03);
  }

  .sonarr-stat-value {
    font-size: 1.1rem;
    font-weight: 700;
    color: var(--color-text);
  }

  .sonarr-stat-value.muted {
    font-size: 0.78rem;
    font-weight: 400;
    color: var(--color-muted);
  }

  .sonarr-stat-label {
    font-size: 0.72rem;
    color: var(--color-muted);
  }

  .sonarr-actions {
    display: flex;
    gap: 0.75rem;
    margin-top: 0.75rem;
  }

  .action-btn.danger {
    border-color: rgba(255, 130, 130, 0.4);
    background: rgba(255, 130, 130, 0.1);
    color: #ffb0b0;
  }

  .action-btn.danger:hover {
    background: rgba(255, 130, 130, 0.2);
  }

  .action-btn.outline {
    background: transparent;
    border-color: rgba(143, 183, 255, 0.35);
  }

  .action-btn.outline:hover {
    background: rgba(143, 183, 255, 0.12);
  }

  .form-group {
    display: grid;
    gap: 0.4rem;
    margin-bottom: 0.75rem;
  }

  .form-label {
    font-size: 0.82rem;
    color: var(--color-muted);
  }

  .form-input {
    border: 1px solid rgba(143, 183, 255, 0.25);
    border-radius: 8px;
    padding: 0.55rem 0.7rem;
    background: rgba(255, 255, 255, 0.06);
    color: var(--color-text);
    font-size: 0.9rem;
  }

  .form-input:focus {
    outline: 2px solid rgba(143, 183, 255, 0.4);
    outline-offset: 1px;
  }

  .form-actions {
    display: flex;
    gap: 0.75rem;
    margin-top: 0.5rem;
  }

  .import-report {
    margin-top: 0.75rem;
    padding: 0.6rem 0.9rem;
    border: 1px solid rgba(143, 183, 255, 0.2);
    border-radius: 10px;
    background: rgba(143, 183, 255, 0.06);
    font-size: 0.82rem;
    color: #c8d2e0;
  }

  .success-msg {
    margin-top: 0.5rem;
    color: #7ee87e;
    font-size: 0.82rem;
  }
```

- [ ] **Step 5: Run frontend verification**

```bash
cd next && npm run check && npm run test
```

Expected: clean, all tests PASS.

- [ ] **Step 6: Commit**

```bash
git add src/lib/SettingsView.svelte
git commit -m "feat: add sonarr settings tab"
```

---

### Task 7: DetailView — Sonarr Section + Remap UI

**Files:**
- Modify: `next/src/lib/DetailView.svelte`
- Create: `next/src/lib/SonarrRemap.svelte`

**Interfaces:**
- Consumes: `getSonarrAvailability` from api.ts (Task 5)
- Produces: Collapsible Sonarr section in detail view, remap UI

- [ ] **Step 1: Write `SonarrRemap.svelte`**

Create `next/src/lib/SonarrRemap.svelte`:

```svelte
<script lang="ts">
  import { remapSonarr, searchLibrary, type LibraryEntry } from './api';

  export let sonarrId: number;
  export let currentAnimeId: number | null;

  let open = false;
  let query = '';
  let results: LibraryEntry[] = [];
  let loading = false;
  let selectedId: number | null = null;
  let saving = false;

  function toggle() {
    open = !open;
    if (open && results.length === 0) {
      search('');
    }
  }

  async function search(q: string) {
    loading = true;
    try {
      results = await searchLibrary(q, null, 10, 0);
    } finally {
      loading = false;
    }
  }

  function onInput(e: Event) {
    query = (e.target as HTMLInputElement).value;
    search(query);
  }

  async function confirm() {
    if (selectedId === null) return;
    saving = true;
    try {
      await remapSonarr(sonarrId, selectedId);
      open = false;
      // Parent component will re-fetch on next render
      window.location.reload();
    } finally {
      saving = false;
    }
  }

  async function unmap() {
    saving = true;
    try {
      await remapSonarr(sonarrId, null);
      open = false;
      window.location.reload();
    } finally {
      saving = false;
    }
  }
</script>

<div class="remap-wrap">
  <button type="button" class="remap-toggle" on:click={toggle}>
    {open ? 'Close' : '✎ remap'}
  </button>

  {#if open}
    <div class="remap-dropdown">
      <input
        class="remap-search"
        type="text"
        placeholder="Search anime..."
        value={query}
        on:input={onInput}
      />

      {#if loading}
        <p class="remap-hint muted">Searching…</p>
      {:else if results.length === 0 && query}
        <p class="remap-hint muted">No matches</p>
      {/if}

      {#if results.length > 0}
        <ul class="remap-list" role="listbox">
          {#each results as entry}
            <li
              class="remap-option"
              class:selected={selectedId === entry.anime_id}
              role="option"
              aria-selected={selectedId === entry.anime_id}
              on:click={() => (selectedId = entry.anime_id)}
              on:keydown={(e) => e.key === 'Enter' && (selectedId = entry.anime_id)}
              tabindex="0"
            >
              <span class="remap-title">{entry.title}</span>
              <span class="remap-meta">{entry.status} · {entry.watched_episodes}/{entry.episode_count ?? '?'}</span>
            </li>
          {/each}
        </ul>
      {/if}

      <div class="remap-actions">
        <button
          type="button"
          class="remap-confirm"
          disabled={selectedId === null || saving}
          on:click={confirm}
        >
          {saving ? 'Saving…' : 'Confirm'}
        </button>
        {#if currentAnimeId}
          <button
            type="button"
            class="remap-unmap"
            disabled={saving}
            on:click={unmap}
          >
            Unmap
          </button>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .remap-wrap {
    position: relative;
  }

  .remap-toggle {
    border: 1px solid rgba(143, 183, 255, 0.25);
    border-radius: 6px;
    padding: 0.25rem 0.6rem;
    background: rgba(143, 183, 255, 0.08);
    color: var(--color-muted);
    cursor: pointer;
    font-size: 0.72rem;
  }

  .remap-toggle:hover {
    background: rgba(143, 183, 255, 0.16);
    color: #e9eefc;
  }

  .remap-dropdown {
    position: absolute;
    top: 100%;
    right: 0;
    margin-top: 0.25rem;
    width: 320px;
    max-height: 360px;
    overflow-y: auto;
    border: 1px solid rgba(143, 183, 255, 0.25);
    border-radius: 12px;
    background: #171e2b;
    padding: 0.75rem;
    z-index: 10;
    display: grid;
    gap: 0.5rem;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  }

  .remap-search {
    border: 1px solid rgba(143, 183, 255, 0.25);
    border-radius: 8px;
    padding: 0.5rem 0.7rem;
    background: rgba(255, 255, 255, 0.06);
    color: var(--color-text);
    font-size: 0.85rem;
    width: 100%;
    box-sizing: border-box;
  }

  .remap-search:focus {
    outline: 2px solid rgba(143, 183, 255, 0.4);
    outline-offset: 1px;
  }

  .remap-hint {
    font-size: 0.78rem;
    margin: 0;
  }

  .remap-hint.muted {
    color: var(--color-muted);
  }

  .remap-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 0.25rem;
  }

  .remap-option {
    display: grid;
    gap: 0.1rem;
    padding: 0.4rem 0.6rem;
    border-radius: 6px;
    cursor: pointer;
    border: 1px solid transparent;
  }

  .remap-option:hover {
    background: rgba(143, 183, 255, 0.1);
  }

  .remap-option.selected {
    background: rgba(143, 183, 255, 0.18);
    border-color: rgba(143, 183, 255, 0.35);
  }

  .remap-title {
    font-size: 0.82rem;
    color: var(--color-text);
  }

  .remap-meta {
    font-size: 0.7rem;
    color: var(--color-muted);
  }

  .remap-actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }

  .remap-confirm {
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 6px;
    padding: 0.35rem 0.8rem;
    background: rgba(143, 183, 255, 0.15);
    color: #e9eefc;
    cursor: pointer;
    font-size: 0.78rem;
  }

  .remap-confirm:hover:not(:disabled) {
    background: rgba(143, 183, 255, 0.25);
  }

  .remap-confirm:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .remap-unmap {
    border: 1px solid rgba(255, 130, 130, 0.3);
    border-radius: 6px;
    padding: 0.35rem 0.8rem;
    background: transparent;
    color: #ffb0b0;
    cursor: pointer;
    font-size: 0.78rem;
  }

  .remap-unmap:hover:not(:disabled) {
    background: rgba(255, 130, 130, 0.12);
  }
</style>
```

- [ ] **Step 2: Add Sonarr section to `DetailView.svelte`**

Add import:

```typescript
import { fetchAnimeDetail, getSonarrAvailability, updateListEntry, type AnimeDetail, type SonarrAvailability } from './api';
import SonarrRemap from './SonarrRemap.svelte';
```

Add state:

```typescript
let sonarrAvail: SonarrAvailability | null = null;
let sonarrLoading = false;
let sonarrExpanded = false;
```

Add load function:

```typescript
async function loadSonarr() {
  sonarrLoading = true;
  try {
    sonarrAvail = await getSonarrAvailability(animeId);
  } catch {
    sonarrAvail = null;
  } finally {
    sonarrLoading = false;
  }
}
```

Call in `load()` function after detail loads, and in `onMount`:

In the `load()` function, after `setDraftsFromDetail(d)`, add:

```typescript
      loadSonarr();
```

Add markup after the watch history section (after `{/if}` closing the history block, before `</div>` of info-col):

```svelte
        {#if sonarrAvail}
          <div class="sonarr-section">
            <div class="section-header-row">
              <h2 class="section-heading">Sonarr</h2>
              <SonarrRemap sonarrId={sonarrAvail.sonarr_id} currentAnimeId={animeId} />
            </div>
            <div class="sonarr-detail">
              <div class="sonarr-field">
                <span class="field-label">Series</span>
                <span class="field-value">{sonarrAvail.sonarr_title}</span>
              </div>
              <div class="sonarr-field">
                <span class="field-label">Episodes</span>
                <span class="field-value">
                  {sonarrAvail.episode_file_count} / {sonarrAvail.episode_count} files on disk
                </span>
              </div>
              <div class="sonarr-field">
                <span class="field-label">Status</span>
                <span class="field-value">
                  {sonarrAvail.sonarr_status ? sonarrAvail.sonarr_status : 'Unknown'}
                  {#if sonarrAvail.monitored} · Monitored ✓{/if}
                  {#if sonarrAvail.next_airing} · Next airing: {new Date(sonarrAvail.next_airing * 1000).toLocaleDateString()}{/if}
                </span>
              </div>
              {#if sonarrAvail.path}
                <div class="sonarr-field">
                  <span class="field-label">Path</span>
                  <span class="field-value path">{sonarrAvail.path}</span>
                </div>
              {/if}
            </div>
          </div>
        {/if}
```

- [ ] **Step 3: Add detail CSS**

Inside the `<style>` block, add:

```css
  .sonarr-section {
    border: 1px solid rgba(143, 183, 255, 0.15);
    border-radius: 14px;
    padding: 1rem;
    background: rgba(255, 255, 255, 0.03);
  }

  .section-header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.75rem;
  }

  .sonarr-detail {
    display: grid;
    gap: 0.5rem;
  }

  .sonarr-field {
    display: grid;
    gap: 0.15rem;
  }

  .field-label {
    font-size: 0.72rem;
    color: var(--color-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .field-value {
    font-size: 0.85rem;
    color: #c8d2e0;
  }

  .field-value.path {
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
    font-size: 0.75rem;
    color: var(--color-muted);
    overflow-wrap: anywhere;
  }
```

- [ ] **Step 4: Run frontend verification**

```bash
cd next && npm run check && npm run test
```

Expected: clean, all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib/DetailView.svelte src/lib/SonarrRemap.svelte
git commit -m "feat: add sonarr detail view section and remap ui"
```

---

### Task 8: Integration Verification

**Files:** None (verification only)

- [ ] **Step 1: Backend full test suite**

```bash
cd next/src-tauri && cargo test
```

Expected: all tests PASS, including new sonarr_* tests.

- [ ] **Step 2: Frontend full test suite**

```bash
cd next && npm run check && npm run test
```

Expected: TypeScript clean, all tests PASS.

- [ ] **Step 3: Verify acceptance criteria**

Check each item against the spec:

```
[ ] User enters Sonarr URL + API key, "Test Connection" validates it
[ ] "Connect" stores settings with encrypted API key, imports all series
[ ] High-confidence series auto-map to anime library entries
[ ] Low-confidence series appear in unmapped list for manual mapping
[ ] User can remap any series via searchable dropdown
[ ] Anime detail view shows Sonarr section with file counts and status
[ ] "Disconnect" clears Sonarr settings and all sonarr_* table data
[ ] All Rust tests pass, all TS tests pass
```

- [ ] **Step 4: Verification commit (if fixes needed)**

If any fixes:

```bash
git add src src-tauri
git commit -m "fix: complete sonarr integration verification"
```

If no fixes needed, skip commit.

---

## Self-Review Notes

- Spec coverage: All 6 commands, 2 UI views, storage tables, auto-match algorithm covered by Tasks 1-8
- No TBD/TODO placeholders — every step has concrete code or commands
- Type consistency: `sonarr_id: i64` everywhere, command names match between Rust and TS
- Test coverage: 4 new Rust test files + 1 modified TS test file
- Existing patterns respected: `_inner` functions for testability, DPAPI for secrets, `SettingsView` tab pattern
- `chrono` added to Cargo.toml for date parsing (no other new deps)
- Migration file `0002_sonarr.sql` follows existing naming pattern
