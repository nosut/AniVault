# AniVault Phase 0 Foundation Implementation Plan

> **Status:** Complete. App rebranded to AniVault (2026-06-30). See `docs/superpowers/plans/2026-06-30-anivault-rebrand-installer.md` for rebrand details.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the non-user-facing foundation for AniVault: Tauri/Svelte/Rust scaffold, event boundary, SQLite schema, encrypted secrets, migration skeleton, and verification harness.

**Architecture:** Create a clean-room app under `next/` so the existing C++ Taiga remains untouched. The Rust engine owns storage, events, settings, secrets, migration, and Tauri commands. The Svelte UI is a thin consumer of commands/events and must not access SQLite directly.

**Tech Stack:** Tauri v2, Svelte, TypeScript, Rust 2021, SQLite via `sqlx`, DPAPI via `windows`, tests via Rust `cargo test` and Vitest.

**Status:** All 7 tasks completed. Merged to `develop` at `1e8b033e`. Rust crate renamed from `taiga_next` → `anivault_core`. App product name: AniVault. Tauri identifier: `app.anivault.desktop`.

## Global Constraints

- Product direction: clean-room rewrite, not in-place refactor.
- Target platform: Windows only.
- User model: single local profile for one Windows user.
- Source of truth: local-first SQLite database.
- UX style: premium dark media app with poster art, restrained depth, and smooth interactions.
- MVP scope: include tracking, library, and tracker flows, but deliver watch detection → auto-match → progress update → sync first.
- Old torrent/RSS section: not carried forward.
- Sonarr: action-capable integration first, designed for fuller coordination after core foundation.
- Existing Taiga source is reference material only; do not modify `src/` for this plan.
- Keep UI and engine decoupled: UI calls Tauri commands; engine modules do not import UI code.
- Every task must end with tests or a build command and a commit.

---

## File Structure

Create this new subtree:

```text
next/
  package.json                         # Node scripts and frontend deps
  index.html                           # Vite entry document
  tsconfig.json                        # TypeScript config
  vite.config.ts                       # Svelte + Vite config
  vitest.config.ts                     # Frontend test config
  src/
    main.ts                            # Mounts Svelte app
    App.svelte                         # Minimal dark shell
    lib/api.ts                         # Typed Tauri command wrappers
    lib/api.test.ts                    # API wrapper unit tests
    styles/tokens.css                  # Dark design tokens
  src-tauri/
    Cargo.toml                         # Rust crate manifest
    build.rs                           # Tauri build hook
    tauri.conf.json                    # Tauri app config
    src/
      main.rs                          # Tauri entry point
      lib.rs                           # Module exports and app builder
      commands.rs                      # Tauri command surface
      engine/
        mod.rs                         # Engine module exports
        events.rs                      # Typed events
        event_bus.rs                   # In-process event queue
        models.rs                      # Domain structs shared by modules
        storage.rs                     # SQLite connection and migrations
        secrets.rs                     # DPAPI secret storage
        migration.rs                   # Taiga import skeleton and report
        settings.rs                    # Local settings model
      tests/
        event_bus_test.rs              # Event tests
        storage_test.rs                # Schema/storage tests
        migration_test.rs              # Import report tests
        secrets_test.rs                # DPAPI round-trip tests
    migrations/
      0001_initial.sql                 # Initial SQLite schema
```

---

### Task 1: Clean-room Tauri/Svelte scaffold

**Files:**
- Create: `next/package.json`
- Create: `next/index.html`
- Create: `next/tsconfig.json`
- Create: `next/vite.config.ts`
- Create: `next/vitest.config.ts`
- Create: `next/src/main.ts`
- Create: `next/src/App.svelte`
- Create: `next/src/styles/tokens.css`
- Create: `next/src-tauri/Cargo.toml`
- Create: `next/src-tauri/build.rs`
- Create: `next/src-tauri/tauri.conf.json`
- Create: `next/src-tauri/src/main.rs`
- Create: `next/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: none.
- Produces: `next` app root with working `npm run check`, `npm run test`, and `cargo test` entry points.

- [x] **Step 1: Create frontend manifest**

Create `next/package.json`:

```json
{
  "name": "taiga-next",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "check": "tsc --noEmit",
    "test": "vitest run"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.5.0",
    "@vitejs/plugin-svelte": "^5.0.3",
    "svelte": "^5.25.0"
  },
  "devDependencies": {
    "typescript": "^5.8.2",
    "vite": "^6.2.3",
    "vitest": "^3.0.9"
  }
}
```

- [x] **Step 2: Create Vite and TypeScript config**

Create `next/index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Taiga Next</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

Create `next/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "module": "ESNext",
    "resolveJsonModule": true,
    "allowJs": false,
    "checkJs": false,
    "isolatedModules": true,
    "moduleDetection": "force",
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "moduleResolution": "Bundler",
    "types": ["vitest/globals"]
  },
  "include": ["src/**/*.ts", "src/**/*.svelte", "vite.config.ts", "vitest.config.ts"]
}
```

Create `next/vite.config.ts`:

```ts
import { svelte } from '@vitejs/plugin-svelte';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    strictPort: true,
    port: 1420,
  },
});
```

Create `next/vitest.config.ts`:

```ts
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
  },
});
```

- [x] **Step 3: Create minimal dark shell**

Create `next/src/styles/tokens.css`:

```css
:root {
  color-scheme: dark;
  --color-bg: #080a0f;
  --color-surface: #111520;
  --color-surface-raised: #1a2030;
  --color-text: #f4f7fb;
  --color-muted: #9aa6b8;
  --color-accent: #8fb7ff;
  --radius-card: 22px;
  --shadow-card: 0 24px 80px rgb(0 0 0 / 45%);
  --font-ui: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
}

html,
body,
#app {
  min-height: 100%;
  margin: 0;
}

body {
  background: radial-gradient(circle at top left, #18233a 0, #080a0f 42rem);
  color: var(--color-text);
  font-family: var(--font-ui);
}
```

Create `next/src/main.ts`:

```ts
import './styles/tokens.css';
import App from './App.svelte';

const app = new App({
  target: document.getElementById('app') as HTMLElement,
});

export default app;
```

Create `next/src/App.svelte`:

```svelte
<script lang="ts">
  const navItems = ['Home', 'Library', 'Watching', 'Calendar', 'Sync', 'Integrations', 'Settings'];
</script>

<main class="shell">
  <aside class="rail" aria-label="Main navigation">
    <div class="brand">Taiga Next</div>
    {#each navItems as item}
      <button class:active={item === 'Home'}>{item}</button>
    {/each}
  </aside>

  <section class="home">
    <p class="eyebrow">Foundation build</p>
    <h1>Premium dark anime library, local-first engine.</h1>
    <div class="card">
      <span>Phase 0</span>
      <strong>Engine scaffold ready for storage, migration, sync, and Sonarr integration.</strong>
    </div>
  </section>
</main>

<style>
  .shell {
    display: grid;
    grid-template-columns: 16rem 1fr;
    min-height: 100vh;
  }

  .rail {
    border-right: 1px solid rgb(255 255 255 / 8%);
    background: rgb(10 13 20 / 72%);
    padding: 1.5rem;
    backdrop-filter: blur(24px);
  }

  .brand {
    font-weight: 800;
    letter-spacing: -0.04em;
    margin-bottom: 2rem;
  }

  button {
    display: block;
    width: 100%;
    border: 0;
    border-radius: 999px;
    margin: 0.25rem 0;
    padding: 0.8rem 1rem;
    text-align: left;
    color: var(--color-muted);
    background: transparent;
  }

  button.active,
  button:hover {
    color: var(--color-text);
    background: rgb(255 255 255 / 8%);
  }

  .home {
    padding: 4rem;
  }

  .eyebrow {
    color: var(--color-accent);
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.78rem;
    font-weight: 800;
  }

  h1 {
    max-width: 54rem;
    font-size: clamp(3rem, 7vw, 6rem);
    line-height: 0.94;
    letter-spacing: -0.08em;
  }

  .card {
    display: grid;
    gap: 0.5rem;
    max-width: 34rem;
    border: 1px solid rgb(255 255 255 / 10%);
    border-radius: var(--radius-card);
    background: linear-gradient(145deg, rgb(255 255 255 / 12%), rgb(255 255 255 / 4%));
    box-shadow: var(--shadow-card);
    padding: 1.5rem;
  }

  .card span {
    color: var(--color-muted);
  }
</style>
```

- [x] **Step 4: Create Tauri Rust manifest and config**

Create `next/src-tauri/Cargo.toml`:

```toml
[package]
name = "taiga-next"
version = "0.1.0"
description = "Clean-room Taiga successor"
authors = ["Taiga Next Contributors"]
edition = "2021"

[lib]
name = "taiga_next"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2.2.0", features = [] }

[dependencies]
anyhow = "1.0.97"
serde = { version = "1.0.219", features = ["derive"] }
serde_json = "1.0.140"
sqlx = { version = "0.8.3", features = ["runtime-tokio", "sqlite", "macros", "migrate", "chrono"] }
tauri = { version = "2.4.0", features = [] }
thiserror = "2.0.12"
tokio = { version = "1.44.1", features = ["macros", "rt-multi-thread", "sync"] }
uuid = { version = "1.16.0", features = ["v4", "serde"] }
windows = { version = "0.61.1", features = ["Win32_Security_Cryptography", "Win32_Foundation"] }

[dev-dependencies]
tempfile = "3.19.1"
```

Create `next/src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build();
}
```

Create `next/src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Taiga Next",
  "version": "0.1.0",
  "identifier": "app.taiga.next",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Taiga Next",
        "width": 1280,
        "height": 820,
        "minWidth": 960,
        "minHeight": 640
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": false,
    "targets": "all"
  }
}
```

Create `next/src-tauri/src/main.rs`:

```rust
fn main() {
    taiga_next::run();
}
```

Create `next/src-tauri/src/lib.rs`:

```rust
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Taiga Next");
}
```

- [x] **Step 5: Install dependencies and verify scaffold**

Run from `next`:

```bash
npm install
npm run check
npm run test
```

Expected:

```text
Found 0 errors.
No test files found
```

Run from `next/src-tauri`:

```bash
cargo test
```

Expected: compilation succeeds and reports zero Rust tests.

- [x] **Step 6: Commit scaffold**

```bash
git add next/package.json next/package-lock.json next/index.html next/tsconfig.json next/vite.config.ts next/vitest.config.ts next/src next/src-tauri
git commit -m "feat: scaffold taiga next"
```

---

### Task 2: Engine models and event bus

**Files:**
- Create: `next/src-tauri/src/engine/mod.rs`
- Create: `next/src-tauri/src/engine/models.rs`
- Create: `next/src-tauri/src/engine/events.rs`
- Create: `next/src-tauri/src/engine/event_bus.rs`
- Create: `next/src-tauri/tests/event_bus_test.rs`
- Modify: `next/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: scaffold from Task 1.
- Produces: `EngineEvent`, `EventBus`, `EventSubscriber`, `AnimeId`, `ServiceId`, `EpisodeNumber`.

- [x] **Step 1: Write failing event bus tests**

Create `next/src-tauri/tests/event_bus_test.rs`:

```rust
use taiga_next::engine::event_bus::EventBus;
use taiga_next::engine::events::{EngineEvent, MediaDetected};

#[test]
fn event_bus_records_published_events_in_order() {
    let bus = EventBus::default();

    bus.publish(EngineEvent::MediaDetected(MediaDetected {
        player_name: "mpv".to_string(),
        file_path: Some("D:/Anime/Episode 01.mkv".to_string()),
        window_title: Some("Episode 01".to_string()),
        detected_at_unix: 1_782_769_008,
    }));

    bus.publish(EngineEvent::SyncFailed {
        service: "anilist".to_string(),
        anime_id: 42,
        message: "network offline".to_string(),
    });

    let events = bus.drain();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], EngineEvent::MediaDetected(_)));
    assert!(matches!(events[1], EngineEvent::SyncFailed { .. }));
    assert!(bus.drain().is_empty());
}
```

- [x] **Step 2: Run test to verify it fails**

Run from `next/src-tauri`:

```bash
cargo test --test event_bus_test
```

Expected: FAIL because `taiga_next::engine` is not defined.

- [x] **Step 3: Add engine module exports**

Create `next/src-tauri/src/engine/mod.rs`:

```rust
pub mod event_bus;
pub mod events;
pub mod models;
```

Create `next/src-tauri/src/engine/models.rs`:

```rust
pub type AnimeId = i64;
pub type EpisodeNumber = i32;
pub type ServiceId = String;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WatchStatus {
    Watching,
    Completed,
    OnHold,
    Dropped,
    PlanToWatch,
}
```

Create `next/src-tauri/src/engine/events.rs`:

```rust
use crate::engine::models::{AnimeId, EpisodeNumber, ServiceId};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MediaDetected {
    pub player_name: String,
    pub file_path: Option<String>,
    pub window_title: Option<String>,
    pub detected_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AnimeIdentified {
    pub anime_id: AnimeId,
    pub episode: EpisodeNumber,
    pub confidence: u8,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EngineEvent {
    MediaDetected(MediaDetected),
    AnimeIdentified(AnimeIdentified),
    ProgressAdvanced {
        anime_id: AnimeId,
        old_episode: EpisodeNumber,
        new_episode: EpisodeNumber,
        source: String,
    },
    SyncQueued {
        service: ServiceId,
        anime_id: AnimeId,
    },
    SyncFailed {
        service: ServiceId,
        anime_id: AnimeId,
        message: String,
    },
}
```

Create `next/src-tauri/src/engine/event_bus.rs`:

```rust
use std::sync::{Arc, Mutex};

use crate::engine::events::EngineEvent;

#[derive(Debug, Clone, Default)]
pub struct EventBus {
    events: Arc<Mutex<Vec<EngineEvent>>>,
}

impl EventBus {
    pub fn publish(&self, event: EngineEvent) {
        self.events.lock().expect("event bus poisoned").push(event);
    }

    pub fn drain(&self) -> Vec<EngineEvent> {
        let mut events = self.events.lock().expect("event bus poisoned");
        std::mem::take(&mut *events)
    }
}
```

Modify `next/src-tauri/src/lib.rs`:

```rust
pub mod engine;

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Taiga Next");
}
```

- [x] **Step 4: Run test to verify it passes**

Run from `next/src-tauri`:

```bash
cargo test --test event_bus_test
```

Expected: PASS.

- [x] **Step 5: Commit event boundary**

```bash
git add next/src-tauri/src/lib.rs next/src-tauri/src/engine next/src-tauri/tests/event_bus_test.rs
git commit -m "feat: add engine event boundary"
```

---

### Task 3: SQLite schema and storage foundation

**Files:**
- Create: `next/src-tauri/migrations/0001_initial.sql`
- Create: `next/src-tauri/src/engine/storage.rs`
- Create: `next/src-tauri/tests/storage_test.rs`
- Modify: `next/src-tauri/src/engine/mod.rs`

**Interfaces:**
- Consumes: `AnimeId`, `EpisodeNumber` from Task 2.
- Produces: `Storage::connect(database_url: &str) -> anyhow::Result<Storage>`, `Storage::migrate(&self) -> anyhow::Result<()>`, `Storage::append_watch_history(...) -> anyhow::Result<i64>`, `Storage::queue_sync(...) -> anyhow::Result<i64>`.

- [x] **Step 1: Write failing storage tests**

Create `next/src-tauri/tests/storage_test.rs`:

```rust
use taiga_next::engine::storage::Storage;

#[tokio::test]
async fn storage_migrates_and_uses_wal_mode() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let journal_mode = storage.journal_mode().await.unwrap();
    assert_eq!(journal_mode.to_lowercase(), "memory");
}

#[tokio::test]
async fn storage_appends_history_and_queues_sync() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();

    let history_id = storage
        .append_watch_history(1, 7, Some("D:/Anime/Cowboy Bebop 07.mkv"), Some("mpv"), 1_782_769_008)
        .await
        .unwrap();
    assert!(history_id > 0);

    let sync_id = storage
        .queue_sync(1, "anilist", "update_progress", r#"{"episode":7}"#, 1_782_769_008)
        .await
        .unwrap();
    assert!(sync_id > 0);

    let pending = storage.pending_sync_count("anilist").await.unwrap();
    assert_eq!(pending, 1);
}
```

- [x] **Step 2: Run test to verify it fails**

Run from `next/src-tauri`:

```bash
cargo test --test storage_test
```

Expected: FAIL because `engine::storage` is not defined.

- [x] **Step 3: Add schema migration**

Create `next/src-tauri/migrations/0001_initial.sql`:

```sql
CREATE TABLE IF NOT EXISTS anime (
  id INTEGER PRIMARY KEY,
  titles_json TEXT NOT NULL,
  type TEXT,
  status TEXT,
  episode_count INTEGER,
  image_url TEXT,
  synopsis TEXT,
  last_modified INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS list_entry (
  anime_id INTEGER PRIMARY KEY REFERENCES anime(id) ON DELETE CASCADE,
  status TEXT NOT NULL,
  watched_episodes INTEGER NOT NULL DEFAULT 0,
  score INTEGER,
  notes TEXT,
  date_started TEXT,
  date_completed TEXT,
  local_updated INTEGER NOT NULL,
  remote_updated INTEGER
);

CREATE TABLE IF NOT EXISTS watch_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  anime_id INTEGER NOT NULL REFERENCES anime(id) ON DELETE CASCADE,
  episode INTEGER NOT NULL,
  file_path TEXT,
  player TEXT,
  watched_at INTEGER NOT NULL,
  source TEXT NOT NULL DEFAULT 'taiga_next'
);

CREATE TABLE IF NOT EXISTS tracker_mapping (
  anime_id INTEGER NOT NULL REFERENCES anime(id) ON DELETE CASCADE,
  service TEXT NOT NULL,
  remote_id TEXT NOT NULL,
  PRIMARY KEY (anime_id, service)
);

CREATE TABLE IF NOT EXISTS file_index (
  file_path TEXT PRIMARY KEY,
  anime_id INTEGER REFERENCES anime(id) ON DELETE SET NULL,
  episode INTEGER,
  confidence INTEGER NOT NULL DEFAULT 0,
  indexed_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_queue (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  anime_id INTEGER NOT NULL REFERENCES anime(id) ON DELETE CASCADE,
  service TEXT NOT NULL,
  operation TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  retry_count INTEGER NOT NULL DEFAULT 0,
  next_retry_at INTEGER
);

CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS migration_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source TEXT NOT NULL,
  source_id TEXT,
  status TEXT NOT NULL,
  message TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sonarr_mapping (
  anime_id INTEGER PRIMARY KEY REFERENCES anime(id) ON DELETE CASCADE,
  sonarr_series_id INTEGER NOT NULL,
  sonarr_title TEXT NOT NULL,
  monitored INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS integration_queue (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  integration TEXT NOT NULL,
  operation TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  retry_count INTEGER NOT NULL DEFAULT 0,
  next_retry_at INTEGER
);
```

- [x] **Step 4: Add storage implementation**

Create `next/src-tauri/src/engine/storage.rs`:

```rust
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

#[derive(Clone)]
pub struct Storage {
    pool: SqlitePool,
}

impl Storage {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::query("PRAGMA journal_mode = WAL").execute(&self.pool).await?;
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    pub async fn journal_mode(&self) -> anyhow::Result<String> {
        let row = sqlx::query("PRAGMA journal_mode").fetch_one(&self.pool).await?;
        Ok(row.get::<String, _>(0))
    }

    pub async fn insert_minimal_anime(&self, id: i64, title: &str) -> anyhow::Result<()> {
        let titles_json = serde_json::json!({ "romaji": title, "english": null, "japanese": null, "synonyms": [] }).to_string();
        sqlx::query(
            "INSERT OR REPLACE INTO anime (id, titles_json, last_modified) VALUES (?1, ?2, 0)",
        )
        .bind(id)
        .bind(titles_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn append_watch_history(
        &self,
        anime_id: i64,
        episode: i32,
        file_path: Option<&str>,
        player: Option<&str>,
        watched_at: i64,
    ) -> anyhow::Result<i64> {
        let result = sqlx::query(
            "INSERT INTO watch_history (anime_id, episode, file_path, player, watched_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(anime_id)
        .bind(episode)
        .bind(file_path)
        .bind(player)
        .bind(watched_at)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn queue_sync(
        &self,
        anime_id: i64,
        service: &str,
        operation: &str,
        payload_json: &str,
        created_at: i64,
    ) -> anyhow::Result<i64> {
        let result = sqlx::query(
            "INSERT INTO sync_queue (anime_id, service, operation, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(anime_id)
        .bind(service)
        .bind(operation)
        .bind(payload_json)
        .bind(created_at)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn pending_sync_count(&self, service: &str) -> anyhow::Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) FROM sync_queue WHERE service = ?1")
            .bind(service)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>(0))
    }
}
```

Modify `next/src-tauri/src/engine/mod.rs`:

```rust
pub mod event_bus;
pub mod events;
pub mod models;
pub mod storage;
```

- [x] **Step 5: Run storage tests**

Run from `next/src-tauri`:

```bash
cargo test --test storage_test
```

Expected: PASS.

- [x] **Step 6: Commit storage foundation**

```bash
git add next/src-tauri/migrations next/src-tauri/src/engine/mod.rs next/src-tauri/src/engine/storage.rs next/src-tauri/tests/storage_test.rs
git commit -m "feat: add local storage foundation"
```

---

### Task 4: DPAPI secret storage

**Files:**
- Create: `next/src-tauri/src/engine/secrets.rs`
- Create: `next/src-tauri/tests/secrets_test.rs`
- Modify: `next/src-tauri/src/engine/mod.rs`

**Interfaces:**
- Consumes: none from earlier engine modules.
- Produces: `protect_secret(plaintext: &str) -> anyhow::Result<String>` and `unprotect_secret(ciphertext_b64: &str) -> anyhow::Result<String>`.

- [x] **Step 1: Write failing DPAPI test**

Create `next/src-tauri/tests/secrets_test.rs`:

```rust
use taiga_next::engine::secrets::{protect_secret, unprotect_secret};

#[test]
fn dpapi_round_trips_secret() {
    let encrypted = protect_secret("sonarr-api-key-123").unwrap();
    assert_ne!(encrypted, "sonarr-api-key-123");

    let decrypted = unprotect_secret(&encrypted).unwrap();
    assert_eq!(decrypted, "sonarr-api-key-123");
}
```

- [x] **Step 2: Run test to verify it fails**

Run from `next/src-tauri`:

```bash
cargo test --test secrets_test
```

Expected: FAIL because `engine::secrets` is not defined.

- [x] **Step 3: Add base64 dependency**

Modify `next/src-tauri/Cargo.toml` dependencies by adding:

```toml
base64 = "0.22.1"
```

- [x] **Step 4: Implement DPAPI wrapper**

Create `next/src-tauri/src/engine/secrets.rs`:

```rust
use base64::{engine::general_purpose::STANDARD, Engine as _};
use windows::core::PWSTR;
use windows::Win32::Foundation::LocalFree;
use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, DATA_BLOB};

pub fn protect_secret(plaintext: &str) -> anyhow::Result<String> {
    let mut input = DATA_BLOB {
        cbData: plaintext.as_bytes().len() as u32,
        pbData: plaintext.as_bytes().as_ptr() as *mut u8,
    };
    let mut output = DATA_BLOB::default();

    unsafe {
        CryptProtectData(
            &mut input,
            PWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )?;

        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize);
        let encoded = STANDARD.encode(bytes);
        LocalFree(Some(output.pbData as _));
        Ok(encoded)
    }
}

pub fn unprotect_secret(ciphertext_b64: &str) -> anyhow::Result<String> {
    let mut encrypted = STANDARD.decode(ciphertext_b64)?;
    let mut input = DATA_BLOB {
        cbData: encrypted.len() as u32,
        pbData: encrypted.as_mut_ptr(),
    };
    let mut output = DATA_BLOB::default();

    unsafe {
        CryptUnprotectData(
            &mut input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )?;

        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize);
        let plaintext = String::from_utf8(bytes.to_vec())?;
        LocalFree(Some(output.pbData as _));
        Ok(plaintext)
    }
}
```

Modify `next/src-tauri/src/engine/mod.rs`:

```rust
pub mod event_bus;
pub mod events;
pub mod models;
pub mod secrets;
pub mod storage;
```

- [x] **Step 5: Run DPAPI test**

Run from `next/src-tauri`:

```bash
cargo test --test secrets_test
```

Expected: PASS on Windows.

- [x] **Step 6: Commit secret storage**

```bash
git add next/src-tauri/Cargo.toml next/src-tauri/Cargo.lock next/src-tauri/src/engine/mod.rs next/src-tauri/src/engine/secrets.rs next/src-tauri/tests/secrets_test.rs
git commit -m "feat: add encrypted secret storage"
```

---

### Task 5: Migration report skeleton

**Files:**
- Create: `next/src-tauri/src/engine/migration.rs`
- Create: `next/src-tauri/tests/migration_test.rs`
- Modify: `next/src-tauri/src/engine/mod.rs`

**Interfaces:**
- Consumes: `Storage` from Task 3.
- Produces: `MigrationReport`, `MigrationWarning`, `import_taiga_snapshot(storage: &Storage, snapshot: TaigaSnapshot) -> anyhow::Result<MigrationReport>`.

- [x] **Step 1: Write failing migration test**

Create `next/src-tauri/tests/migration_test.rs`:

```rust
use taiga_next::engine::migration::{import_taiga_snapshot, TaigaAnime, TaigaSnapshot};
use taiga_next::engine::storage::Storage;

#[tokio::test]
async fn import_snapshot_reports_imported_and_skipped_records() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let snapshot = TaigaSnapshot {
        anime: vec![
            TaigaAnime { id: 10, title: "Frieren".to_string(), watched_episodes: 12 },
            TaigaAnime { id: 0, title: "Broken".to_string(), watched_episodes: 1 },
        ],
    };

    let report = import_taiga_snapshot(&storage, snapshot).await.unwrap();
    assert_eq!(report.imported_anime, 1);
    assert_eq!(report.skipped_records, 1);
    assert_eq!(report.warnings[0].source_id, "0");
}
```

- [x] **Step 2: Run test to verify it fails**

Run from `next/src-tauri`:

```bash
cargo test --test migration_test
```

Expected: FAIL because `engine::migration` is not defined.

- [x] **Step 3: Add migration implementation**

Create `next/src-tauri/src/engine/migration.rs`:

```rust
use crate::engine::storage::Storage;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaigaSnapshot {
    pub anime: Vec<TaigaAnime>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaigaAnime {
    pub id: i64,
    pub title: String,
    pub watched_episodes: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MigrationWarning {
    pub source: String,
    pub source_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct MigrationReport {
    pub imported_anime: usize,
    pub skipped_records: usize,
    pub warnings: Vec<MigrationWarning>,
}

pub async fn import_taiga_snapshot(storage: &Storage, snapshot: TaigaSnapshot) -> anyhow::Result<MigrationReport> {
    let mut report = MigrationReport::default();

    for anime in snapshot.anime {
        if anime.id <= 0 || anime.title.trim().is_empty() {
            report.skipped_records += 1;
            report.warnings.push(MigrationWarning {
                source: "taiga_anime".to_string(),
                source_id: anime.id.to_string(),
                message: "Skipped anime with invalid id or blank title".to_string(),
            });
            continue;
        }

        storage.insert_minimal_anime(anime.id, &anime.title).await?;
        if anime.watched_episodes > 0 {
            storage
                .append_watch_history(anime.id, anime.watched_episodes, None, None, 0)
                .await?;
        }
        report.imported_anime += 1;
    }

    Ok(report)
}
```

Modify `next/src-tauri/src/engine/mod.rs`:

```rust
pub mod event_bus;
pub mod events;
pub mod migration;
pub mod models;
pub mod secrets;
pub mod storage;
```

- [x] **Step 4: Run migration test**

Run from `next/src-tauri`:

```bash
cargo test --test migration_test
```

Expected: PASS.

- [x] **Step 5: Commit migration skeleton**

```bash
git add next/src-tauri/src/engine/mod.rs next/src-tauri/src/engine/migration.rs next/src-tauri/tests/migration_test.rs
git commit -m "feat: add migration report skeleton"
```

---

### Task 6: Tauri command API and frontend wrapper

**Files:**
- Create: `next/src-tauri/src/commands.rs`
- Modify: `next/src-tauri/src/lib.rs`
- Create: `next/src/lib/api.ts`
- Create: `next/src/lib/api.test.ts`

**Interfaces:**
- Consumes: `MigrationReport` from Task 5.
- Produces: Tauri commands `get_engine_status()` and `preview_migration_report()`; frontend functions `getEngineStatus(invokeFn)` and `previewMigrationReport(invokeFn)`.

- [x] **Step 1: Write failing frontend API tests**

Create `next/src/lib/api.test.ts`:

```ts
import { describe, expect, it, vi } from 'vitest';
import { getEngineStatus, previewMigrationReport } from './api';

describe('api wrappers', () => {
  it('gets engine status through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue({ ok: true, database: 'ready' });
    await expect(getEngineStatus(invoke)).resolves.toEqual({ ok: true, database: 'ready' });
    expect(invoke).toHaveBeenCalledWith('get_engine_status');
  });

  it('previews migration report through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue({ imported_anime: 0, skipped_records: 0, warnings: [] });
    await expect(previewMigrationReport(invoke)).resolves.toEqual({ imported_anime: 0, skipped_records: 0, warnings: [] });
    expect(invoke).toHaveBeenCalledWith('preview_migration_report');
  });
});
```

- [x] **Step 2: Run frontend test to verify it fails**

Run from `next`:

```bash
npm run test -- src/lib/api.test.ts
```

Expected: FAIL because `src/lib/api.ts` does not exist.

- [x] **Step 3: Add frontend command wrappers**

Create `next/src/lib/api.ts`:

```ts
import { invoke as tauriInvoke } from '@tauri-apps/api/core';

export type InvokeFn = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

export interface EngineStatus {
  ok: boolean;
  database: 'ready' | 'uninitialized';
}

export interface MigrationWarning {
  source: string;
  source_id: string;
  message: string;
}

export interface MigrationReport {
  imported_anime: number;
  skipped_records: number;
  warnings: MigrationWarning[];
}

export function getEngineStatus(invokeFn: InvokeFn = tauriInvoke): Promise<EngineStatus> {
  return invokeFn<EngineStatus>('get_engine_status');
}

export function previewMigrationReport(invokeFn: InvokeFn = tauriInvoke): Promise<MigrationReport> {
  return invokeFn<MigrationReport>('preview_migration_report');
}
```

- [x] **Step 4: Add Rust Tauri commands**

Create `next/src-tauri/src/commands.rs`:

```rust
use crate::engine::migration::MigrationReport;

#[derive(Debug, Clone, serde::Serialize)]
pub struct EngineStatus {
    pub ok: bool,
    pub database: &'static str,
}

#[tauri::command]
pub fn get_engine_status() -> EngineStatus {
    EngineStatus {
        ok: true,
        database: "uninitialized",
    }
}

#[tauri::command]
pub fn preview_migration_report() -> MigrationReport {
    MigrationReport::default()
}
```

Modify `next/src-tauri/src/lib.rs`:

```rust
pub mod commands;
pub mod engine;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_engine_status,
            commands::preview_migration_report,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Taiga Next");
}
```

- [x] **Step 5: Run API tests and Rust tests**

Run from `next`:

```bash
npm run test -- src/lib/api.test.ts
```

Expected: PASS.

Run from `next/src-tauri`:

```bash
cargo test
```

Expected: PASS.

- [x] **Step 6: Commit command API**

```bash
git add next/src/lib/api.ts next/src/lib/api.test.ts next/src-tauri/src/commands.rs next/src-tauri/src/lib.rs
git commit -m "feat: expose foundation command api"
```

---

### Task 7: Foundation verification script and documentation

**Files:**
- Create: `next/README.md`
- Create: `next/scripts/verify.ps1`
- Modify: `next/package.json`

**Interfaces:**
- Consumes: all prior tasks.
- Produces: `npm run verify` command that runs frontend checks, frontend tests, and Rust tests.

- [x] **Step 1: Add verify script**

Create `next/scripts/verify.ps1`:

```powershell
$ErrorActionPreference = "Stop"

npm run check
npm run test
Push-Location -LiteralPath "src-tauri"
try {
  cargo test
} finally {
  Pop-Location
}
```

Modify `next/package.json` scripts block to include `verify`:

```json
{
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "check": "tsc --noEmit",
    "test": "vitest run",
    "verify": "pwsh -NoProfile -ExecutionPolicy Bypass -File ./scripts/verify.ps1"
  }
}
```

- [x] **Step 2: Add foundation README**

Create `next/README.md`:

```markdown
# Taiga Next

Clean-room Windows-only successor to Taiga.

## Phase 0 scope

- Tauri v2 shell
- Svelte + TypeScript frontend
- Rust engine boundary
- SQLite storage foundation
- DPAPI secret storage
- Migration report skeleton
- Narrow Tauri command API

## Commands

```powershell
npm install
npm run verify
```

Run the desktop shell during development:

```powershell
npm run dev
```

In another terminal:

```powershell
Set-Location -LiteralPath src-tauri
cargo run
```
```

- [x] **Step 3: Run full verification**

Run from `next`:

```bash
npm run verify
```

Expected: TypeScript check passes, Vitest passes, Rust tests pass.

- [x] **Step 4: Commit verification docs**

```bash
git add next/README.md next/scripts/verify.ps1 next/package.json
git commit -m "docs: add foundation verification"
```

---

## Final Verification

Run from repo root:

```bash
git status --short
```

Expected: no uncommitted files except intentional work outside `next/`.

Run from `next`:

```bash
npm run verify
```

Expected: all checks pass.

Run from `next/src-tauri`:

```bash
cargo test
```

Expected: all Rust tests pass.

## Phase 0 Completion Criteria

- `next/` app compiles.
- Rust event bus tests pass.
- SQLite migration tests pass.
- DPAPI secret round-trip passes on Windows.
- Migration report test passes.
- Frontend API wrapper tests pass.
- `npm run verify` succeeds.
- Existing C++ Taiga source under `src/` remains untouched.
