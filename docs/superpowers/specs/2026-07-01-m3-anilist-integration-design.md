# M3 AniList Integration Design

## Purpose

Add AniList as the single tracker sync target: OAuth authentication, library import, progress sync, and sync status monitoring. AniList is the only tracker in scope.

## Prerequisites

- M0 Runtime Foundation: DB, state, commands, event bus
- M1 Local Tracking: process scanner, watch history, progress tracking
- M2 Recognition Engine: filename parser, candidate matching, file-index persistence

## Scope

### In

- AniList OAuth authentication via embedded Tauri webview
- DPAPI-encrypted token storage
- Hand-rolled GraphQL client (POST to `graphql.anilist.co`)
- Full library import with `MediaListCollection` query
- Merge-on-import using most-recent-wins (`updatedAt` comparison)
- Debounced sync worker polling `sync_queue` every 30s
- Push progress via `SaveMediaListEntry` mutation
- Exponential backoff on failure (1s → 2s → 4s, max 3 retries)
- HTTP 429 respect `Retry-After` header
- 401 handling: mark blocked, attempt token refresh
- Sync status command and UI (pending/failed/blocked counts)

### Out

- MAL or Kitsu tracking
- Multi-tracker abstraction layers
- Tray behavior (M5)
- Rebrand/installer (M8)
- AniList OAuth client ID management UI (client ID passed as config/code)

## Architecture

```
Browser (Tauri Webview)
       │
       ├─ OAuth flow ──▶ AniList ──▶ access_token (DPAPI + settings)
       │
       ▼
    commands.rs ──▶ engine::anilist::client ──HTTP POST──▶ graphql.anilist.co
       │
       ├── engine::anilist::auth       ◀── refresh / re-auth ─── 401
       │
       ├── engine::anilist::import    ──▶ anime + list_entry merge
       │
       └── engine::sync_worker        ── poll sync_queue / 30s ──▶ push progress
              │
              ▼
          sync_queue (retry_count, next_retry_at, exp backoff)
```

## Modules

### `engine::anilist::client`

- Generic GraphQL POST function: serialize query + variables, attach `Authorization: Bearer`, parse response JSON
- Request: `POST https://graphql.anilist.co` with `Content-Type: application/json`
- Response parsing: unwrap `data` / `errors` fields
- Rate-limit handling: detect HTTP 429, respect `Retry-After` header
- 401 handling: return distinguishable error for auth refresh

### `engine::anilist::auth`

- `start_oauth(client_id: &str)` — launch Tauri webview to AniList authorize URL
- `extract_token(redirect_url: &str)` — parse `access_token` from URL fragment
- `store_token(token: &str)` — DPAPI encrypt → settings key `anilist_access_token`
- `load_token()` — settings → DPAPI decrypt
- `is_connected()` — check token exists and is valid (quick test query)
- `disconnect()` — delete token from settings, clear `tracker_mapping`

### `engine::anilist::import`

- `import_library(client, storage)` — full `MediaListCollection` fetch
- Parse: anime metadata (id, title, episode_count, cover image, etc.)
- Upsert: `INSERT OR REPLACE INTO anime` for each entry
- Merge: for each `list_entry`, compare AniList `updatedAt` vs local `local_updated` and `remote_updated`
- Most-recent-wins: if AniList has newer timestamp, overwrite local; if local newer, skip
- Record: `tracker_mapping(anime_id, "anilist", remote_id)`
- Return `ImportReport { imported: u64, merged: u64, skipped: u64 }`

### `engine::sync_worker`

- Tokio background task, spawned on app boot
- Poll loop: `pending_sync_count("anilist")` at 30s interval
- Bypassable: `sync_now()` forces immediate drain
- Drain logic:
  1. Fetch batch of pending rows ordered by `created_at ASC`
  2. Group by `anime_id`, keep only latest per anime (dedup)
  3. Call `client.push_progress(anime_id, episode)` via `SaveMediaListEntry`
  4. On success: delete queue row, update `remote_updated`
  5. On failure: increment `retry_count`, compute `next_retry_at`
  6. On 3+ retries: publish `SyncFailed`, stop retrying that row

### Commands

| Command | Type | In/Out |
|---------|------|--------|
| `connect_anilist` | Tauri | `(client_id: String) → Result<(), String>` |
| `disconnect_anilist` | Tauri | `() → Result<(), String>` |
| `import_anilist_library` | Tauri | `() → Result<ImportReport, String>` |
| `sync_now` | Tauri | `() → Result<(), String>` |
| `get_sync_status` | Tauri | `() → Result<SyncStatus, String>` |

`SyncStatus`: `{ pending: i64, failed: i64, blocked: i64, last_sync_at: Option<i64> }`

### Frontend

- `next/src/lib/api.ts`: `connectAniList`, `disconnectAniList`, `importAniListLibrary`, `syncNow`, `getSyncStatus` wrappers + `SyncStatus` interface
- `next/src/lib/AniListConnect.svelte`: connect/disconnect button, status indicator
- `next/src/lib/SyncStatus.svelte`: pending/failed counts, last sync, "sync now" button
- Integration into `App.svelte`

## Data Flow

### Auth

```
User [Connect AniList] → Tauri webview → AniList OAuth
  → redirect with access_token
  → extract_token → DPAPI encrypt → settings store
```

### Import

```
Trigger: post-auth or manual "import" button
  → GraphQL MediaListCollection query
  → foreach entry:
      INSERT OR REPLACE anime(id, titles, ...)
      IF AniList.updatedAt > max(local_updated, remote_updated):
          UPDATE list_entry (AniList data wins)
      ELSE:
          skip (local data wins)
      INSERT OR IGNORE tracker_mapping(anime_id, "anilist", remote_id)
  → publish EngineEvent::SyncQueued for each anime
```

### Push

```
Trigger: ProgressAdvanced event or manual mark-watched
  → commands.rs publishes ProgressAdvanced
  → (existing) storage.queue_sync(anime_id, "anilist", ...)
  → sync_worker polls every 30s
  → drain: fetch batch, dedup by anime_id, push via SaveMediaListEntry
  → success: delete row, update remote_updated
  → failure: retry with backoff, capped at 3 attempts
```

## Error Handling

| Condition | Behavior |
|-----------|----------|
| Network error (timeout, DNS) | Exponential backoff, max 3 retries |
| HTTP 429 (rate limit) | Respect `Retry-After`, set `next_retry_at` |
| HTTP 401 (unauthorized) | Attempt `refresh_token`, mark queue rows blocked, publish `SyncFailed` |
| GraphQL errors in response | Log payload, publish `SyncFailed`, mark row blocked |
| JSON parse error | Log raw response, mark row blocked |
| Token not found (disconnected) | Skip sync worker loop, publish `SyncFailed` |

## Test Strategy

| Test file | Coverage |
|-----------|----------|
| `tests/anilist_client_test.rs` | GraphQL request shaping, response parsing, 401/429 handling |
| `tests/anilist_auth_test.rs` | Token encrypt/decrypt roundtrip, stored/loaded cycle |
| `tests/anilist_import_test.rs` | Merge logic: AniList-wins, local-wins, equal timestamps, empty library |
| `tests/sync_worker_test.rs` | Queue drain, anime dedup, backoff math, max-retry gating |
| `tests/anilist_commands_test.rs` | Command wiring: status report, sync trigger, connect/disconnect |

Frontend: existing `api.test.ts` pattern for new wrapper functions.

## Dependencies

No new crate dependencies. HTTP via `reqwest` (transitive Tauri dependency), OAuth webview via Tauri's built-in webview API. GraphQL queries hand-written as strings with `serde_json`.

---

## Spec Self-Review

- No TBD/TODO placeholders
- Architecture matches feature descriptions: auth → import → sync worker → commands → UI
- Scope focused: only AniList, no multi-tracker abstraction, no tray/rebrand
- Ambiguity resolved: most-recent-wins for merge, exponential backoff for retry, debounced 30s polling, dedup by anime_id in drain
- Existing schema leveraged: `sync_queue`, `tracker_mapping`, `list_entry.local_updated`/`remote_updated`, `settings`, `integration_queue`
