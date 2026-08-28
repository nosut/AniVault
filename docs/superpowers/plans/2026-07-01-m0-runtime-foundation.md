# M0 Runtime Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current Tauri shell stubs with a real app runtime: initialized SQLite storage, migrations, state-backed commands, settings CRUD, and an engine event delivery path.

**Architecture:** Add a small runtime layer that owns shared engine state (`Storage`, `EventBus`, and database path) and registers it with Tauri during startup. Commands consume managed state instead of returning hardcoded values. Frontend API wrappers expose status, settings, and event-drain calls without building feature UI yet.

**Tech Stack:** Rust 2021, Tauri 2.4, SQLx SQLite, Tokio, Svelte 5, TypeScript, Vitest, PowerShell verification script.

## Global Constraints

- Windows desktop app runtime using Tauri, Svelte, Rust, SQLite.
- AniList is the only tracker integration in scope; do not add MAL or Kitsu code.
- M0 scope only: runtime foundation, settings, state-backed commands, event delivery scaffolding.
- No media detection, recognition, sync worker, Sonarr, tray, rebrand, or installer work in this plan.
- Keep files small and focused; do not turn `commands.rs` into the runtime owner.
- Every command must return a frontend-safe `Result<T, String>` when it can fail.

---

## File Structure

- Create `next/src-tauri/src/engine/runtime.rs`  
  Owns `EngineState`, database path creation, SQLite URL conversion, storage migration, and runtime initialization.

- Modify `next/src-tauri/src/engine/mod.rs`  
  Exports the new `runtime` module.

- Modify `next/src-tauri/src/engine/storage.rs`  
  Adds settings CRUD and a simple migration-health query.

- Modify `next/src-tauri/src/commands.rs`  
  Converts stub commands into state-backed commands and adds `get_setting`, `set_setting`, `delete_setting`, and `drain_engine_events`.

- Modify `next/src-tauri/src/lib.rs`  
  Initializes runtime in Tauri setup, registers state, and exposes new commands.

- Create `next/src-tauri/tests/runtime_test.rs`  
  Tests runtime DB URL/path helpers and state initialization using a temp directory.

- Create `next/src-tauri/tests/settings_test.rs`  
  Tests settings CRUD against migrated in-memory storage.

- Create `next/src-tauri/tests/commands_test.rs`  
  Tests command functions directly using `EngineState` without launching a full Tauri window.

- Modify `next/src-tauri/tests/storage_test.rs`  
  Fixes journal-mode assertion so it tests actual behavior.

- Modify `next/src/lib/api.ts`  
  Adds frontend wrappers/types for settings and engine events; expands engine status type.

- Modify `next/src/lib/api.test.ts`  
  Tests new wrappers and changed status shape.

- Modify `next/src/App.svelte`  
  Displays real status fields and a minimal settings smoke path; no broad UI redesign.

---

### Task 1: Storage Settings and Health

**Files:**
- Modify: `next/src-tauri/src/engine/storage.rs`
- Modify: `next/src-tauri/tests/storage_test.rs`
- Create: `next/src-tauri/tests/settings_test.rs`

**Interfaces:**
- Consumes: existing `Storage::connect(database_url: &str) -> anyhow::Result<Storage>` and `Storage::migrate(&self) -> anyhow::Result<()>`.
- Produces:
  - `Storage::migration_count(&self) -> anyhow::Result<i64>`
  - `Storage::get_setting(&self, key: &str) -> anyhow::Result<Option<String>>`
  - `Storage::set_setting(&self, key: &str, value_json: &str, updated_at: i64) -> anyhow::Result<()>`
  - `Storage::delete_setting(&self, key: &str) -> anyhow::Result<bool>`

- [ ] **Step 1: Write failing settings tests**

Create `next/src-tauri/tests/settings_test.rs`:

```rust
use taiga_next::engine::storage::Storage;

#[tokio::test]
async fn settings_roundtrip_json_values() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    assert_eq!(storage.get_setting("tracking.enabled").await.unwrap(), None);

    storage
        .set_setting("tracking.enabled", "true", 1_782_769_008)
        .await
        .unwrap();
    assert_eq!(
        storage.get_setting("tracking.enabled").await.unwrap(),
        Some("true".to_string())
    );

    storage
        .set_setting("tracking.enabled", "false", 1_782_769_009)
        .await
        .unwrap();
    assert_eq!(
        storage.get_setting("tracking.enabled").await.unwrap(),
        Some("false".to_string())
    );
}

#[tokio::test]
async fn settings_delete_reports_whether_row_existed() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    assert!(!storage.delete_setting("theme").await.unwrap());

    storage
        .set_setting("theme", r#"\"dark\""#, 1_782_769_008)
        .await
        .unwrap();

    assert!(storage.delete_setting("theme").await.unwrap());
    assert_eq!(storage.get_setting("theme").await.unwrap(), None);
}

#[tokio::test]
async fn migrated_storage_reports_migration_count() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    assert!(storage.migration_count().await.unwrap() >= 1);
}
```

- [ ] **Step 2: Run tests to verify failure**

Run from `next/src-tauri`:

```bash
cargo test settings_test
```

Expected: FAIL with missing methods `get_setting`, `set_setting`, `delete_setting`, and `migration_count`.

- [ ] **Step 3: Fix journal-mode test**

Modify `next/src-tauri/tests/storage_test.rs` test `storage_migrates_and_uses_wal_mode`:

```rust
#[tokio::test]
async fn storage_migrates_and_uses_journal_mode_supported_by_memory_sqlite() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let journal_mode = storage.journal_mode().await.unwrap();
    assert_eq!(journal_mode.to_lowercase(), "memory");
}
```

Reason: in-memory SQLite reports `memory`; file-backed runtime DB will use WAL.

- [ ] **Step 4: Implement storage methods**

Add these methods inside `impl Storage` in `next/src-tauri/src/engine/storage.rs`:

```rust
    pub async fn migration_count(&self) -> anyhow::Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>(0))
    }

    pub async fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>> {
        let row = sqlx::query("SELECT value_json FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|row| row.get::<String, _>(0)))
    }

    pub async fn set_setting(
        &self,
        key: &str,
        value_json: &str,
        updated_at: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value_json)
        .bind(updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_setting(&self, key: &str) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM settings WHERE key = ?1")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
```

- [ ] **Step 5: Run storage/settings tests**

Run from `next/src-tauri`:

```bash
cargo test storage_test settings_test
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/engine/storage.rs src-tauri/tests/storage_test.rs src-tauri/tests/settings_test.rs
git commit -m "feat: add settings storage"
```

---

### Task 2: Engine Runtime State

**Files:**
- Create: `next/src-tauri/src/engine/runtime.rs`
- Modify: `next/src-tauri/src/engine/mod.rs`
- Create: `next/src-tauri/tests/runtime_test.rs`

**Interfaces:**
- Consumes:
  - `Storage::connect(&str)`
  - `Storage::migrate()`
  - `EventBus::default()`
- Produces:
  - `pub struct EngineState { pub storage: Storage, pub events: EventBus, pub database_path: PathBuf }`
  - `pub async fn initialize_engine_at(database_path: PathBuf) -> anyhow::Result<EngineState>`
  - `pub fn sqlite_url_for_path(path: &Path) -> String`

- [ ] **Step 1: Export runtime module**

Modify `next/src-tauri/src/engine/mod.rs` so it includes:

```rust
pub mod event_bus;
pub mod events;
pub mod migration;
pub mod models;
pub mod runtime;
pub mod secrets;
pub mod storage;
```

- [ ] **Step 2: Write failing runtime tests**

Create `next/src-tauri/tests/runtime_test.rs`:

```rust
use std::path::PathBuf;

use taiga_next::engine::events::EngineEvent;
use taiga_next::engine::runtime::{initialize_engine_at, sqlite_url_for_path};

#[test]
fn sqlite_url_for_path_normalizes_windows_separators() {
    let path = PathBuf::from(r"C:\Users\example\AppData\Roaming\AniVault\anivault.db");
    assert_eq!(
        sqlite_url_for_path(&path),
        "sqlite:C:/Users/example/AppData/Roaming/AniVault/anivault.db"
    );
}

#[tokio::test]
async fn initialize_engine_creates_parent_dir_and_migrates_database() {
    let root = std::env::temp_dir().join(format!(
        "anivault-runtime-test-{}",
        std::process::id()
    ));
    let db_path = root.join("nested").join("anivault.db");

    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }

    let state = initialize_engine_at(db_path.clone()).await.unwrap();

    assert_eq!(state.database_path, db_path);
    assert!(state.database_path.exists());
    assert!(state.storage.migration_count().await.unwrap() >= 1);
    assert!(state.events.drain().is_empty());

    state.events.publish(EngineEvent::SyncQueued {
        service: "anilist".to_string(),
        anime_id: 1,
    });
    assert_eq!(state.events.drain().len(), 1);

    std::fs::remove_dir_all(root).unwrap();
}
```

- [ ] **Step 3: Run tests to verify failure**

Run from `next/src-tauri`:

```bash
cargo test runtime_test
```

Expected: FAIL because `engine::runtime` does not exist.

- [ ] **Step 4: Implement runtime module**

Create `next/src-tauri/src/engine/runtime.rs`:

```rust
use std::path::{Path, PathBuf};

use crate::engine::event_bus::EventBus;
use crate::engine::storage::Storage;

#[derive(Clone)]
pub struct EngineState {
    pub storage: Storage,
    pub events: EventBus,
    pub database_path: PathBuf,
}

pub fn sqlite_url_for_path(path: &Path) -> String {
    format!("sqlite:{}", path.to_string_lossy().replace('\\', "/"))
}

pub async fn initialize_engine_at(database_path: PathBuf) -> anyhow::Result<EngineState> {
    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let database_url = sqlite_url_for_path(&database_path);
    let storage = Storage::connect(&database_url).await?;
    storage.migrate().await?;

    Ok(EngineState {
        storage,
        events: EventBus::default(),
        database_path,
    })
}
```

- [ ] **Step 5: Run runtime tests**

Run from `next/src-tauri`:

```bash
cargo test runtime_test
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/engine/mod.rs src-tauri/src/engine/runtime.rs src-tauri/tests/runtime_test.rs
git commit -m "feat: add engine runtime state"
```

---

### Task 3: State-Backed Commands

**Files:**
- Modify: `next/src-tauri/src/commands.rs`
- Create: `next/src-tauri/tests/commands_test.rs`

**Interfaces:**
- Consumes: `EngineState`, `Storage` settings methods, `EventBus::drain()`.
- Produces commands:
  - `get_engine_status(state: tauri::State<'_, EngineState>) -> Result<EngineStatus, String>`
  - `get_setting(key: String, state: tauri::State<'_, EngineState>) -> Result<Option<serde_json::Value>, String>`
  - `set_setting(key: String, value: serde_json::Value, state: tauri::State<'_, EngineState>) -> Result<(), String>`
  - `delete_setting(key: String, state: tauri::State<'_, EngineState>) -> Result<bool, String>`
  - `drain_engine_events(state: tauri::State<'_, EngineState>) -> Result<Vec<EngineEvent>, String>`
  - Direct test helpers with the same logic but `&EngineState` inputs.

- [ ] **Step 1: Write command tests against direct helpers**

Create `next/src-tauri/tests/commands_test.rs`:

```rust
use taiga_next::commands::{
    delete_setting_inner, drain_engine_events_inner, get_engine_status_inner, get_setting_inner,
    set_setting_inner,
};
use taiga_next::engine::events::EngineEvent;
use taiga_next::engine::runtime::initialize_engine_at;

async fn test_state(name: &str) -> taiga_next::engine::runtime::EngineState {
    let root = std::env::temp_dir().join(format!("anivault-command-test-{}-{name}", std::process::id()));
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    initialize_engine_at(root.join("anivault.db")).await.unwrap()
}

#[tokio::test]
async fn engine_status_uses_runtime_state() {
    let state = test_state("status").await;
    let status = get_engine_status_inner(&state).await.unwrap();

    assert!(status.ok);
    assert_eq!(status.database, "ready");
    assert!(status.database_path.ends_with("anivault.db"));
    assert!(status.migration_count >= 1);
}

#[tokio::test]
async fn settings_commands_roundtrip_json() {
    let state = test_state("settings").await;

    assert_eq!(get_setting_inner("tracking.enabled", &state).await.unwrap(), None);

    set_setting_inner("tracking.enabled", serde_json::json!(true), &state)
        .await
        .unwrap();
    assert_eq!(
        get_setting_inner("tracking.enabled", &state).await.unwrap(),
        Some(serde_json::json!(true))
    );

    assert!(delete_setting_inner("tracking.enabled", &state).await.unwrap());
    assert_eq!(get_setting_inner("tracking.enabled", &state).await.unwrap(), None);
}

#[tokio::test]
async fn drain_engine_events_returns_and_clears_events() {
    let state = test_state("events").await;
    state.events.publish(EngineEvent::SyncQueued {
        service: "anilist".to_string(),
        anime_id: 42,
    });

    let events = drain_engine_events_inner(&state).await.unwrap();
    assert_eq!(events.len(), 1);
    assert!(drain_engine_events_inner(&state).await.unwrap().is_empty());
}
```

- [ ] **Step 2: Run tests to verify failure**

Run from `next/src-tauri`:

```bash
cargo test commands_test
```

Expected: FAIL because command helper functions do not exist.

- [ ] **Step 3: Replace `commands.rs` with state-backed command logic**

Replace `next/src-tauri/src/commands.rs` with:

```rust
use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::events::EngineEvent;
use crate::engine::migration::MigrationReport;
use crate::engine::runtime::EngineState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct EngineStatus {
    pub ok: bool,
    pub database: String,
    pub database_path: String,
    pub migration_count: i64,
}

fn command_error(error: anyhow::Error) -> String {
    error.to_string()
}

fn unix_now() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?;
    Ok(duration.as_secs() as i64)
}

pub async fn get_engine_status_inner(state: &EngineState) -> Result<EngineStatus, String> {
    let migration_count = state
        .storage
        .migration_count()
        .await
        .map_err(command_error)?;

    Ok(EngineStatus {
        ok: true,
        database: "ready".to_string(),
        database_path: state.database_path.to_string_lossy().to_string(),
        migration_count,
    })
}

pub async fn get_setting_inner(
    key: &str,
    state: &EngineState,
) -> Result<Option<serde_json::Value>, String> {
    let Some(value_json) = state.storage.get_setting(key).await.map_err(command_error)? else {
        return Ok(None);
    };
    let value = serde_json::from_str(&value_json).map_err(|error| error.to_string())?;
    Ok(Some(value))
}

pub async fn set_setting_inner(
    key: &str,
    value: serde_json::Value,
    state: &EngineState,
) -> Result<(), String> {
    let value_json = serde_json::to_string(&value).map_err(|error| error.to_string())?;
    state
        .storage
        .set_setting(key, &value_json, unix_now()?)
        .await
        .map_err(command_error)
}

pub async fn delete_setting_inner(key: &str, state: &EngineState) -> Result<bool, String> {
    state.storage.delete_setting(key).await.map_err(command_error)
}

pub async fn drain_engine_events_inner(state: &EngineState) -> Result<Vec<EngineEvent>, String> {
    Ok(state.events.drain())
}

#[tauri::command]
pub async fn get_engine_status(
    state: tauri::State<'_, EngineState>,
) -> Result<EngineStatus, String> {
    get_engine_status_inner(&state).await
}

#[tauri::command]
pub async fn preview_migration_report() -> Result<MigrationReport, String> {
    Ok(MigrationReport::default())
}

#[tauri::command]
pub async fn get_setting(
    key: String,
    state: tauri::State<'_, EngineState>,
) -> Result<Option<serde_json::Value>, String> {
    get_setting_inner(&key, &state).await
}

#[tauri::command]
pub async fn set_setting(
    key: String,
    value: serde_json::Value,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    set_setting_inner(&key, value, &state).await
}

#[tauri::command]
pub async fn delete_setting(
    key: String,
    state: tauri::State<'_, EngineState>,
) -> Result<bool, String> {
    delete_setting_inner(&key, &state).await
}

#[tauri::command]
pub async fn drain_engine_events(
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<EngineEvent>, String> {
    drain_engine_events_inner(&state).await
}
```

- [ ] **Step 4: Run command tests**

Run from `next/src-tauri`:

```bash
cargo test commands_test
```

Expected: PASS.

- [ ] **Step 5: Run all Rust tests**

Run from `next/src-tauri`:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/tests/commands_test.rs
git commit -m "feat: back commands with engine state"
```

---

### Task 4: Tauri Startup Initialization

**Files:**
- Modify: `next/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `initialize_engine_at(database_path: PathBuf) -> anyhow::Result<EngineState>`.
- Produces: Tauri-managed `EngineState` available to all commands.

- [ ] **Step 1: Build to expose current missing command/state wiring**

Run from `next/src-tauri` after Task 3:

```bash
cargo test
```

Expected: FAIL or compile error because `lib.rs` still registers old command list and does not manage `EngineState`.

- [ ] **Step 2: Wire runtime into Tauri builder**

Replace `next/src-tauri/src/lib.rs` with:

```rust
pub mod commands;
pub mod engine;

use crate::engine::runtime::initialize_engine_at;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let database_path = app
                .path()
                .app_data_dir()
                .map_err(|error| Box::<dyn std::error::Error>::from(error))?
                .join("anivault.db");

            let state = tauri::async_runtime::block_on(initialize_engine_at(database_path))
                .map_err(|error| Box::<dyn std::error::Error>::from(error))?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_engine_status,
            commands::preview_migration_report,
            commands::get_setting,
            commands::set_setting,
            commands::delete_setting,
            commands::drain_engine_events,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Taiga Next");
}
```

If this fails because `.path()` requires a trait import in this Tauri version, add:

```rust
use tauri::Manager;
```

near the top of `lib.rs`.

- [ ] **Step 3: Run Rust tests**

Run from `next/src-tauri`:

```bash
cargo test
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: initialize engine on startup"
```

---

### Task 5: Frontend API Wrappers

**Files:**
- Modify: `next/src/lib/api.ts`
- Modify: `next/src/lib/api.test.ts`

**Interfaces:**
- Consumes Tauri commands from Task 3:
  - `get_engine_status`
  - `get_setting`
  - `set_setting`
  - `delete_setting`
  - `drain_engine_events`
- Produces TypeScript wrappers:
  - `getEngineStatus(invokeFn?: InvokeFn): Promise<EngineStatus>`
  - `getSetting<T>(key: string, invokeFn?: InvokeFn): Promise<T | null>`
  - `setSetting(key: string, value: unknown, invokeFn?: InvokeFn): Promise<void>`
  - `deleteSetting(key: string, invokeFn?: InvokeFn): Promise<boolean>`
  - `drainEngineEvents(invokeFn?: InvokeFn): Promise<EngineEvent[]>`

- [ ] **Step 1: Update frontend wrapper tests first**

Replace `next/src/lib/api.test.ts` with:

```ts
import { describe, expect, it, vi } from 'vitest';
import {
  deleteSetting,
  drainEngineEvents,
  getEngineStatus,
  getSetting,
  previewMigrationReport,
  setSetting,
} from './api';

describe('api wrappers', () => {
  it('gets engine status through invoke', async () => {
    const status = {
      ok: true,
      database: 'ready',
      database_path: 'C:/Users/example/AppData/Roaming/AniVault/anivault.db',
      migration_count: 1,
    };
    const invoke = vi.fn().mockResolvedValue(status);

    await expect(getEngineStatus(invoke)).resolves.toEqual(status);
    expect(invoke).toHaveBeenCalledWith('get_engine_status');
  });

  it('previews migration report through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue({ imported_anime: 0, skipped_records: 0, warnings: [] });
    await expect(previewMigrationReport(invoke)).resolves.toEqual({ imported_anime: 0, skipped_records: 0, warnings: [] });
    expect(invoke).toHaveBeenCalledWith('preview_migration_report');
  });

  it('gets setting through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(true);
    await expect(getSetting<boolean>('tracking.enabled', invoke)).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledWith('get_setting', { key: 'tracking.enabled' });
  });

  it('sets setting through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(setSetting('tracking.enabled', true, invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('set_setting', { key: 'tracking.enabled', value: true });
  });

  it('deletes setting through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(true);
    await expect(deleteSetting('tracking.enabled', invoke)).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledWith('delete_setting', { key: 'tracking.enabled' });
  });

  it('drains engine events through invoke', async () => {
    const events = [{ SyncQueued: { service: 'anilist', anime_id: 1 } }];
    const invoke = vi.fn().mockResolvedValue(events);
    await expect(drainEngineEvents(invoke)).resolves.toEqual(events);
    expect(invoke).toHaveBeenCalledWith('drain_engine_events');
  });
});
```

- [ ] **Step 2: Run frontend tests to verify failure**

Run from `next`:

```bash
npm run test
```

Expected: FAIL because new exports do not exist and `EngineStatus` type is stale.

- [ ] **Step 3: Update API wrappers**

Replace `next/src/lib/api.ts` with:

```ts
import { invoke as tauriInvoke } from '@tauri-apps/api/core';

export type InvokeFn = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

export interface EngineStatus {
  ok: boolean;
  database: 'ready';
  database_path: string;
  migration_count: number;
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

export interface MediaDetectedEvent {
  MediaDetected: {
    player_name: string;
    file_path: string | null;
    window_title: string | null;
    detected_at_unix: number;
  };
}

export interface AnimeIdentifiedEvent {
  AnimeIdentified: {
    anime_id: number;
    episode: number;
    confidence: number;
    evidence: string;
  };
}

export interface ProgressAdvancedEvent {
  ProgressAdvanced: {
    anime_id: number;
    old_episode: number;
    new_episode: number;
    source: string;
  };
}

export interface SyncQueuedEvent {
  SyncQueued: {
    service: string;
    anime_id: number;
  };
}

export interface SyncFailedEvent {
  SyncFailed: {
    service: string;
    anime_id: number;
    message: string;
  };
}

export type EngineEvent =
  | MediaDetectedEvent
  | AnimeIdentifiedEvent
  | ProgressAdvancedEvent
  | SyncQueuedEvent
  | SyncFailedEvent;

export function getEngineStatus(invokeFn: InvokeFn = tauriInvoke): Promise<EngineStatus> {
  return invokeFn<EngineStatus>('get_engine_status');
}

export function previewMigrationReport(invokeFn: InvokeFn = tauriInvoke): Promise<MigrationReport> {
  return invokeFn<MigrationReport>('preview_migration_report');
}

export function getSetting<T>(key: string, invokeFn: InvokeFn = tauriInvoke): Promise<T | null> {
  return invokeFn<T | null>('get_setting', { key });
}

export function setSetting(key: string, value: unknown, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('set_setting', { key, value });
}

export function deleteSetting(key: string, invokeFn: InvokeFn = tauriInvoke): Promise<boolean> {
  return invokeFn<boolean>('delete_setting', { key });
}

export function drainEngineEvents(invokeFn: InvokeFn = tauriInvoke): Promise<EngineEvent[]> {
  return invokeFn<EngineEvent[]>('drain_engine_events');
}
```

- [ ] **Step 4: Run frontend checks**

Run from `next`:

```bash
npm run check
npm run test
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lib/api.ts src/lib/api.test.ts
git commit -m "feat: expose runtime API wrappers"
```

---

### Task 6: Minimal Runtime Status UI

**Files:**
- Modify: `next/src/App.svelte`

**Interfaces:**
- Consumes: `getEngineStatus`, `getSetting`, `setSetting`, `drainEngineEvents` from `src/lib/api.ts`.
- Produces: visible runtime status and a minimal settings write/read smoke path.

- [ ] **Step 1: Replace script block with runtime calls**

In `next/src/App.svelte`, replace the existing `<script lang="ts">` block with:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { drainEngineEvents, getEngineStatus, getSetting, setSetting, type EngineStatus } from './lib/api';

  const navItems = ['Home', 'Library', 'Tracking', 'Sync', 'Settings'];

  let status: EngineStatus | null = null;
  let statusError = '';
  let trackingEnabled = true;
  let eventCount = 0;

  async function refreshRuntime() {
    statusError = '';
    try {
      status = await getEngineStatus();
      trackingEnabled = (await getSetting<boolean>('tracking.enabled')) ?? true;
      eventCount = (await drainEngineEvents()).length;
    } catch (error) {
      statusError = error instanceof Error ? error.message : String(error);
    }
  }

  async function toggleTracking() {
    trackingEnabled = !trackingEnabled;
    try {
      await setSetting('tracking.enabled', trackingEnabled);
      await refreshRuntime();
    } catch (error) {
      statusError = error instanceof Error ? error.message : String(error);
    }
  }

  onMount(() => {
    void refreshRuntime();
  });
</script>
```

- [ ] **Step 2: Add runtime status card markup**

Inside the existing main content area, add this card near the Phase 0 card:

```svelte
<section class="status-card">
  <div>
    <p class="eyebrow">Runtime</p>
    <h2>{status?.database === 'ready' ? 'Engine ready' : 'Engine loading'}</h2>
  </div>

  {#if statusError}
    <p class="error">{statusError}</p>
  {:else if status}
    <dl class="status-list">
      <div>
        <dt>Database</dt>
        <dd>{status.database}</dd>
      </div>
      <div>
        <dt>Migrations</dt>
        <dd>{status.migration_count}</dd>
      </div>
      <div>
        <dt>Events drained</dt>
        <dd>{eventCount}</dd>
      </div>
    </dl>
    <p class="database-path">{status.database_path}</p>
  {:else}
    <p>Checking engine status…</p>
  {/if}

  <button class="toggle" type="button" on:click={toggleTracking}>
    Tracking setting: {trackingEnabled ? 'enabled' : 'disabled'}
  </button>
</section>
```

- [ ] **Step 3: Add minimal styles**

Add styles to `next/src/App.svelte`:

```css
.status-card {
  border: 1px solid rgba(143, 183, 255, 0.18);
  border-radius: var(--radius-card);
  padding: 1.25rem;
  background: rgba(255, 255, 255, 0.04);
  display: grid;
  gap: 1rem;
}

.status-list {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 1rem;
  margin: 0;
}

.status-list div {
  display: grid;
  gap: 0.25rem;
}

.status-list dt {
  color: var(--color-muted);
  font-size: 0.78rem;
}

.status-list dd {
  margin: 0;
  font-weight: 700;
}

.database-path {
  color: var(--color-muted);
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 0.78rem;
  overflow-wrap: anywhere;
}

.error {
  color: #ff9d9d;
}

.toggle {
  justify-self: start;
  border: 1px solid rgba(143, 183, 255, 0.35);
  border-radius: 999px;
  padding: 0.65rem 1rem;
  background: rgba(143, 183, 255, 0.12);
  color: #e9eefc;
  cursor: pointer;
}
```

- [ ] **Step 4: Run frontend checks**

Run from `next`:

```bash
npm run check
npm run test
```

Expected: PASS. If Svelte reports `on:click` deprecation only as warning, leave it unless current project convention already uses Svelte 5 event attributes.

- [ ] **Step 5: Commit**

```bash
git add src/App.svelte
git commit -m "feat: show runtime status"
```

---

### Task 7: Full Verification and M0 Acceptance

**Files:**
- Modify only if a previous task missed a compile/test issue.

**Interfaces:**
- Consumes all prior task outputs.
- Produces verified M0 Runtime Foundation.

- [ ] **Step 1: Run complete verification**

Run from `next`:

```bash
npm run verify
```

Expected:

```text
npm run check
npm run test
cargo test
```

All commands PASS.

- [ ] **Step 2: Manually run the app**

Run from `next`:

```bash
npm run dev
```

Expected:

- Tauri app starts.
- Runtime card shows `Engine ready`.
- Database path ends in `anivault.db`.
- Migration count is at least `1`.
- Tracking setting toggle changes value and persists after refresh.

- [ ] **Step 3: Check M0 acceptance criteria**

Confirm each item:

```text
[ ] App boot creates or opens the database.
[ ] get_engine_status reports real DB and migration state.
[ ] Frontend displays real engine status.
[ ] Settings can be read and written through commands.
[ ] Tests prove commands use real runtime state.
```

- [ ] **Step 4: Commit verification fixes if any**

If Step 1 or Step 2 required fixes:

```bash
git add src src-tauri
git commit -m "fix: complete runtime foundation verification"
```

If no fixes were needed, do not create an empty commit.

---

## Self-Review Notes

- Spec coverage: M0 deliverables are covered by Tasks 1-7.
- Out-of-scope guard: no MAL/Kitsu, media detection, recognition, AniList, Sonarr, tray, installer, or rebrand work included.
- Placeholder scan: no unresolved placeholder markers remain.
- Type consistency: Rust command helpers consume `&EngineState`; Tauri commands consume `tauri::State<'_, EngineState>`; frontend wrapper names match command names.
- Risk note: `preview_migration_report` remains a default report in M0 because real migration import belongs to M7. It is changed to `Result<MigrationReport, String>` for command consistency only.
