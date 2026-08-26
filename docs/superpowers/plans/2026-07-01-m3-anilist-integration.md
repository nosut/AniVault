# M3 AniList Integration Implementation Plan

> **For agentic workers:** Use subagent-driven-development (recommended) to implement task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Add AniList as single tracker sync target: OAuth auth, library import/merge, debounced progress sync with backoff, sync status UI.

**Architecture:** Hand-rolled GraphQL POST client via `reqwest`. OAuth through Tauri webview. DPAPI-encrypted token in settings. Most-recent-wins merge on import. Background Tokio sync worker polls `sync_queue` every 30s with dedup + exponential backoff.

**Tech Stack:** Rust, Tauri 2.x, reqwest, DPAPI, SQLite, Svelte, TypeScript.

## Global Constraints

- Windows desktop app runtime using Tauri, Svelte, Rust, SQLite
- AniList is the only tracker integration in scope; do not add MAL or Kitsu code
- M3 scope only: AniList auth, GraphQL client, list import, progress sync, sync worker, status UI
- No Sonarr (M6), tray (M5), rebrand (M8), library UI (M4)
- Every fallible command must return `Result<T, String>`
- HTTP via `reqwest`; no other new crate dependencies
- Token encryption via existing `engine::secrets::protect_secret` / `unprotect_secret`
- AniList OAuth: Tauri webview to auth URL, extract token from redirect fragment, store encrypted in settings as `anilist_access_token`
- Sync worker: 30s poll, dedup by anime_id, exp backoff 1s→2s→4s, max 3 retries
- GraphQL endpoint: `https://graphql.anilist.co`, `Authorization: Bearer <token>`
- Merge: most-recent-wins comparing AniList `updatedAt` vs `max(local_updated, remote_updated)`

---

### Task 1: AniList GraphQL Client + HTTP Infrastructure

**Files:**
- Modify: `next/src-tauri/Cargo.toml` (add `reqwest`)
- Create: `next/src-tauri/src/engine/anilist/mod.rs`
- Create: `next/src-tauri/src/engine/anilist/client.rs`
- Modify: `next/src-tauri/src/engine/mod.rs` (add `pub mod anilist;`)
- Create: `next/src-tauri/tests/anilist_client_test.rs`

**Interfaces:**
- Produces: `AniListClient { pub token: String, http: reqwest::Client }` with `new(token)`, `query::<T>(query_str, variables)`, `fetch_user_list(user_name)`, `push_progress(anime_id, episode)`
- Produces: types `MediaListCollectionRaw`, `MediaListEntry`, `Media`, `MediaTitle`, `CoverImage`, `AniListDate`, `GraphQLError`

- [ ] **Step 1: Add `reqwest` to Cargo.toml** — after `anyhow = "1.0.97"` add `reqwest = { version = "0.12", features = ["json"] }`

- [ ] **Step 2: Create `engine/anilist/mod.rs`** — `pub mod auth; pub mod client; pub mod import;`

- [ ] **Step 3: `engine/mod.rs`** — add `pub mod anilist;` after `pub mod scanner;`

- [ ] **Step 4: Write failing test `tests/anilist_client_test.rs`**

```rust
use taiga_next::engine::anilist::client::AniListClient;

#[tokio::test]
async fn client_constructs_with_token() {
    let client = AniListClient::new("t".into());
    assert_eq!(client.token, "t");
}

#[tokio::test]
async fn push_progress_errors_on_bad_token() {
    let client = AniListClient::new("bad".into());
    assert!(client.push_progress(1, 5).await.is_err());
}
```
Run: `cargo test --test anilist_client_test` — FAIL (module not found)

- [ ] **Step 5: Implement `engine/anilist/client.rs`** — full module with `AniListClient` struct, `query<T>` POST helper with 401/429 detection, `fetch_user_list` (MediaListCollection query), `push_progress` (SaveMediaListEntry mutation), and all response type structs (`MediaListCollectionRaw`, `MediaListEntry`, `Media`, `MediaTitle`, `CoverImage`, `AniListDate`, `GraphQLError`). See spec Task 1 in brief for complete type definitions. `query<T>` method deserializes via `serde_json`, checks `errors` field, returns `anyhow::Error` on HTTP 4xx or GraphQL errors.

- [ ] **Step 6: Run tests** — `cargo test --test anilist_client_test` → 2 passed

- [ ] **Step 7: Full suite** — `cargo test` → all pass

---

### Task 2: AniList Auth (OAuth Webview + Token Storage)

**Files:**
- Create: `next/src-tauri/src/engine/anilist/auth.rs`
- Create: `next/src-tauri/tests/anilist_auth_test.rs`

**Interfaces:**
- Consumes: `Storage` (existing), `protect_secret` / `unprotect_secret` (existing)
- Produces: `load_token(storage: &Storage) -> anyhow::Result<Option<String>>` — decrypts from settings key `anilist_access_token`
- Produces: `store_token(storage: &Storage, token: &str) -> anyhow::Result<()>` — encrypts to settings
- Produces: `delete_token(storage: &Storage) -> anyhow::Result<()>` — removes from settings
- Produces: `is_connected(storage: &Storage) -> anyhow::Result<bool>` — checks token exists

- [ ] **Step 1: Write failing test `tests/anilist_auth_test.rs`** — 3 tests: `token_roundtrip_encrypt_decrypt` (store "secret-token", load, assert `Some("secret-token")`), `no_token_returns_none` (load from empty storage → `None`), `delete_token_removes_it` (store, delete, load → `None`). Run: `cargo test --test anilist_auth_test` — FAIL.

- [ ] **Step 2: Implement `engine/anilist/auth.rs`** — `load_token` reads `anilist_access_token` setting, calls `unprotect_secret`. `store_token` calls `protect_secret`, writes via `set_setting`. `delete_token` calls `delete_setting`. `is_connected` checks `load_token` returns `Some`.

- [ ] **Step 3: Run tests** — `cargo test --test anilist_auth_test` → 3 passed. Full suite: all pass.

---

### Task 3: Storage Sync Helpers + List Entry Merge Methods

**Files:**
- Modify: `next/src-tauri/src/engine/storage.rs` (add `ListEntryFullRow` struct + 8 new methods)
- Create: `next/src-tauri/tests/anilist_storage_test.rs`

**Interfaces:**
- Consumes: existing `Storage`, `list_entry`, `sync_queue`, `tracker_mapping` tables
- Produces: `upsert_anime(id, titles_json, episode_count, image_url, last_modified)`, `upsert_list_entry_full(anime_id, status, watched, score, notes, local_updated, remote_updated)`, `get_list_entry_full(anime_id) -> Option<ListEntryFullRow>`, `upsert_tracker_mapping(anime_id, service, remote_id)`, `fetch_pending_sync_rows(service, limit) -> Vec<SyncQueueRow>`, `delete_sync_row(id)`, `update_sync_retry(id, retry_count, next_retry_at)`, `sync_status_counts(service) -> (pending, failed, blocked)`

- [ ] **Step 1: Write failing test `tests/anilist_storage_test.rs`** — 5 tests: `upsert_anime_inserts_row`, `list_entry_full_roundtrip` (upsert with score/notes/remote_updated, read back full), `tracker_mapping_upsert_idempotent`, `sync_queue_lifecycle` (queue → fetch → delete → verify empty), `sync_status_counts_work` (queue one row, verify pending=1/failed=0/blocked=0). Run: FAIL.

- [ ] **Step 2: Add `ListEntryFullRow` and `SyncQueueRow` structs + 8 methods to `storage.rs`** — `upsert_anime` uses `INSERT ... ON CONFLICT DO UPDATE`, `upsert_list_entry_full` writes all columns including `remote_updated`, `fetch_pending_sync_rows` filters by `next_retry_at IS NULL OR next_retry_at <= now`, `sync_status_counts` returns counts for pending (retry=0), failed (retry>0 and <3, next_retry_at <= now), blocked (retry>=3). See spec Task 3 in brief.

- [ ] **Step 3: Run tests** — `cargo test --test anilist_storage_test` → 5 passed. Full suite: all pass.

---

### Task 4: AniList Library Import (Merge Engine)

**Files:**
- Create: `next/src-tauri/src/engine/anilist/import.rs`
- Create: `next/src-tauri/tests/anilist_import_test.rs`

**Interfaces:**
- Consumes: `AniListClient` (Task 1), `Storage` with new methods (Task 3)
- Produces: `ImportReport { pub imported: u64, pub merged: u64, pub skipped: u64 }`
- Produces: `pub async fn import_library(client: &AniListClient, storage: &Storage) -> anyhow::Result<ImportReport>`
- Produces: `pub(crate) async fn merge_entry(storage, anime_id, status, progress, score, notes, anilist_updated_at) -> anyhow::Result<bool>` — returns true if entry was merged (AniList wins), false if skipped (local wins)

- [ ] **Step 1: Write failing test `tests/anilist_import_test.rs`** — 2 tests: `merge_anilist_wins_when_newer` (local has old timestamp 500, AniList has 2000 → AniList wins, watched_episodes becomes 10), `local_wins_when_newer` (local has 3000, AniList has 2000 → local wins, watched_episodes stays 7). Run: FAIL.

- [ ] **Step 2: Implement `engine/anilist/import.rs`** — `merge_entry` compares `anilist_updated_at` vs `max(local_updated, remote_updated)`, writes via `upsert_list_entry_full` if AniList wins, returns `Ok(true/false)`. `import_library` fetches user's `MediaListCollection`, iterates lists/entries, calls `upsert_anime` for each media item, `merge_entry` for list entry, `upsert_tracker_mapping` for link. Maps AniList statuses: CURRENT→watching, COMPLETED→completed, PAUSED→on_hold, DROPPED→dropped, default→plan_to_watch.

- [ ] **Step 3: Run tests** — `cargo test --test anilist_import_test` → 2 passed. Full suite: all pass.

---

### Task 5: Sync Worker (Background Poll + Drain + Backoff)

**Files:**
- Create: `next/src-tauri/src/engine/sync_worker.rs`
- Modify: `next/src-tauri/src/engine/mod.rs` (add `pub mod sync_worker;`)
- Create: `next/src-tauri/tests/sync_worker_test.rs`

**Interfaces:**
- Consumes: `EngineState` (existing, Clone), `AniListClient` (Task 1), `auth::load_token` (Task 2), Storage sync methods (Task 3)
- Produces: `pub fn spawn_sync_worker(state: &EngineState) -> tokio::task::JoinHandle<()>`
- Produces: `pub fn backoff_delay(retry_count: i32) -> u64` — 1, 2, 4, capped at 4

- [ ] **Step 1: Write failing test `tests/sync_worker_test.rs`** — `backoff_delay_increases`: assert backoff_delay(0)==1, (1)==2, (2)==4, (3)==4 (capped). Run: FAIL.

- [ ] **Step 2: Implement `engine/sync_worker.rs`** — `backoff_delay` returns 1/2/4. `drain_queue(state)` loads token, creates `AniListClient`, fetches pending rows, deduplicates by anime_id (keep latest episode), pushes progress via `client.push_progress`, on success deletes row, on failure increments retry and sets `next_retry_at` using backoff. At retry_count >= 3, publishes `EngineEvent::SyncFailed` and leaves row blocked. `spawn_sync_worker` clones state and spawns `tokio::spawn` loop calling `drain_queue` then sleeping 30s.

- [ ] **Step 3: Run tests** — `cargo test --test sync_worker_test` → 1 passed. Full suite: all pass.

---

### Task 6: AniList Commands + Tauri Registration

**Files:**
- Modify: `next/src-tauri/src/commands.rs` (add 6 inner functions + 6 command wrappers)
- Modify: `next/src-tauri/src/lib.rs` (register commands, spawn sync worker)
- Create: `next/src-tauri/tests/anilist_commands_test.rs`

**Interfaces:**
- Consumes: `EngineState`, all anilist modules, `sync_worker::spawn_sync_worker`
- Produces: `SyncStatus { pending, failed, blocked, last_sync_at }` serializable struct
- Produces: `connect_anilist_inner`, `store_anilist_token_inner`, `disconnect_anilist_inner`, `import_anilist_library_inner`, `sync_now_inner`, `get_sync_status_inner`
- Produces: Tauri command wrappers (no `_inner` suffix) returning `Result<T, String>`

- [ ] **Step 1: Write failing test `tests/anilist_commands_test.rs`** — 2 tests: `sync_status_returns_zeros_when_empty` (fresh state → pending/failed/blocked all 0), `disconnect_clears_token` (store token, disconnect, verify loaded token is None). Run: FAIL.

- [ ] **Step 2: Add inner functions to `commands.rs`** — `connect_anilist_inner` placeholder (frontend handles webview), `store_anilist_token_inner` calls `auth::store_token`, `disconnect_anilist_inner` calls `auth::delete_token`, `import_anilist_library_inner` loads token → creates client → calls `import_library`, `sync_now_inner` placeholder (worker drain handles it), `get_sync_status_inner` calls `storage.sync_status_counts`. Add command wrappers with `#[tauri::command]`.

- [ ] **Step 3: Register in `lib.rs`** — add 6 new commands to `generate_handler!` macro. Spawn sync worker after engine initialization via `sync_worker::spawn_sync_worker`.

- [ ] **Step 4: Run tests** — `cargo test --test anilist_commands_test` → 2 passed. Full suite: all pass. No compilation errors.

---

### Task 7: Frontend AniList API Wrappers

**Files:**
- Modify: `next/src/lib/api.ts` (add 2 types + 6 wrapper functions)
- Modify: `next/src/lib/api.test.ts` (add 6 test cases)

**Interfaces:**
- Consumes: Tauri command names from Task 6
- Produces: `AniListSyncStatus { pending, failed, blocked, last_sync_at }` interface
- Produces: `ImportReport { imported, merged, skipped }` interface
- Produces: `connectAniList(clientId)`, `storeAniListToken(token)`, `disconnectAniList()`, `importAniListLibrary()`, `syncNow()`, `getSyncStatus()` wrappers following existing `InvokeFn` pattern

- [ ] **Step 1: Add types to `api.ts`** — `AniListSyncStatus` and `ImportReport` interfaces after `FileIndexEntry`.

- [ ] **Step 2: Add wrapper functions to `api.ts`** — all 6 wrappers using `invokeFn<T>` pattern with `tauriInvoke` default. Argument keys match Tauri command parameter names exactly (`clientId`, `token`).

- [ ] **Step 3: Add 6 test cases to `api.test.ts`** — each test verifies wrapper dials correct `invoke` call with correct args, using `vi.fn().mockResolvedValue(...)` pattern. Add imports for new wrapper names.

- [ ] **Step 4: Run** — `npm run check` → clean. `npm run test` → 21 passed (15 existing + 6 new).

---

### Task 8: AniList UI Components

**Files:**
- Create: `next/src/lib/AniListConnect.svelte`
- Create: `next/src/lib/SyncStatus.svelte`
- Modify: `next/src/App.svelte` (integrate both components)

**Interfaces:**
- Consumes: `api.ts` wrapper functions (Task 7)
- Produces: `AniListConnect.svelte` — client ID input, connect/disconnect buttons, import button, connected status
- Produces: `SyncStatus.svelte` — pending/failed/blocked counts, last sync time, "Sync Now" button

- [ ] **Step 1: Create `AniListConnect.svelte`** — text input for client ID, "Connect AniList" button calling `connectAniList(clientId)`, "Disconnect AniList" button calling `disconnectAniList()`, "Import Library" button calling `importAniListLibrary()`, connected state tracked with boolean. Loading state for import.

- [ ] **Step 2: Create `SyncStatus.svelte`** — calls `getSyncStatus()` on mount and after sync, displays pending/failed/blocked counts, last-sync timestamp formatted, "Sync Now" button calling `syncNow()` then refreshing.

- [ ] **Step 3: Integrate into `App.svelte`** — import both components, add `<AniListConnect />` and `<SyncStatus />` in template near navigation or recognition area. Do not change existing component layout.

- [ ] **Step 4: Verify** — `npm run check` → clean. `npm run test` → 21 passed.

---
