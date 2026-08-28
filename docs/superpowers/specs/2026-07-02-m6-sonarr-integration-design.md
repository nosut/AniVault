# M6 Sonarr Integration Design

## Purpose

Connect AniVault to the user's Sonarr instance: server settings, API key validation, series import with auto-matching, anime-to-Sonarr mapping, episode availability display in detail view, and manual remap UI.

## Prerequisites

- M0 Runtime Foundation: DB, state, commands, event bus
- M2 Recognition Engine: parser, matcher, title matching
- M4 Library and App UI: DetailView, SettingsView tabs, state-based navigation

## Scope

### In

- Single Sonarr instance (one URL + API key)
- API key validation via Sonarr `/api/v3/system/status`
- Series import: fetch all series from Sonarr and cache in local SQLite
- Auto-match Sonarr series to anime library using M2 parser/title search
- Episode availability: file counts, monitored status, next airing, path
- Anime-to-Sonarr mapping table with confidence scores
- Manual remap UI for unmapped or mis-matched series
- SettingsView "Sonarr" tab with connect/disconnect/sync/import
- DetailView collapsible Sonarr section
- DPAPI-encrypted API key storage (reuse existing secrets module)
- All data cached locally (same pattern as M3 AniList)

### Out

- Multiple Sonarr instances
- Radarr or other *arr integrations
- Automatic periodic sync (on-demand import only, sync-on-view)
- Sonarr episode file management (read-only display)
- Plex/Jellyfin integration
- Media-source abstraction layers

## Architecture

```
SettingsView (Sonarr tab)          DetailView (Sonarr section)
        │                                    │
        ▼                                    ▼
   api.ts wrappers                      api.ts wrappers
        │                                    │
        ▼                                    ▼
   commands.rs ──────────────────────────────────────
        │           │                │
        ▼           ▼                ▼
  engine::sonarr::client    engine::sonarr::import
        │                    │
        ▼                    ▼
   HTTP → sonarr.mydomain.com/api/v3
        │
        ▼
   SQLite (sonarr_series, sonarr_mapping)
```

### Modules

**`engine::sonarr::client`**
- HTTP client via `reqwest` (already in `Cargo.toml`)
- Base URL construction: `{url}/api/v3` with `X-Api-Key` header
- `validate_connection(url: &str, api_key: &str) -> anyhow::Result<bool>` — hits `/system/status`
- `fetch_series(url: &str, api_key: &str) -> anyhow::Result<Vec<SonarrSeriesRaw>>` — hits `/series`
- Response types: `SonarrSeriesRaw`, `SonarrSeasonRaw`, `SonarrEpisodeRaw`
- Error handling: HTTP 4xx/5xx, JSON parse errors, network timeouts

**`engine::sonarr::import`**
- `import_series(client: &SonarrClient, storage: &Storage) -> anyhow::Result<ImportReport>`
- For each Sonarr series:
  1. INSERT OR REPLACE into `sonarr_series`
  2. Run `parse_filename(&series.title)` to extract cleaned title
  3. Search anime library via `storage.search_anime_by_title(&cleaned_title, 5)`
  4. Score candidates against Sonarr title, year (if available), episode count
  5. High-confidence (score >= 80): auto-map, insert into `sonarr_mapping` with `user_confirmed = 0`
  6. Low-confidence: insert into `sonarr_mapping` with `anime_id = NULL`, `user_confirmed = 0`
- Returns `ImportReport { imported: i64, auto_mapped: i64, unmapped: i64 }`

**`engine::availability`** (or inline in commands)
- `get_availability(anime_id: i64, storage: &Storage) -> anyhow::Result<Option<SonarrAvailability>>`
- Joins `sonarr_mapping` + `sonarr_series` by `anime_id`
- Returns episode file counts, monitored status, next airing, path

### Commands

| Command | Args | Returns | Purpose |
|---------|------|---------|---------|
| `connect_sonarr` | `url: String, api_key: String` | `Result<(), String>` | Validate, store settings, import series |
| `disconnect_sonarr` | none | `Result<(), String>` | Clear settings + sonarr tables |
| `get_sonarr_status` | none | `Result<SonarrStatus, String>` | Connected status, counts, last sync |
| `import_sonarr_series` | none | `Result<ImportReport, String>` | Fetch + auto-match + store |
| `get_sonarr_availability` | `anime_id: i64` | `Result<Option<SonarrAvailability>, String>` | Episode data for one anime |
| `remap_sonarr` | `sonarr_id: i64, anime_id: Option<i64>` | `Result<(), String>` | Manual mapping (None = unmap) |

All commands return `Result<T, String>`. Inner functions take `&EngineState` for testability.

### Data Model

```sql
CREATE TABLE sonarr_series (
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

CREATE TABLE sonarr_mapping (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    sonarr_id            INTEGER NOT NULL REFERENCES sonarr_series(sonarr_id),
    anime_id             INTEGER REFERENCES anime(id),
    title_match          TEXT NOT NULL,
    confidence           INTEGER NOT NULL DEFAULT 0,
    mapped_at            INTEGER NOT NULL,
    user_confirmed       BOOLEAN NOT NULL DEFAULT 0
);
```

Settings keys:
- `sonarr.url` — plain JSON string
- `sonarr.api_key` — DPAPI encrypted (reuse `engine::secrets::protect_secret`)

### Frontend Types

```typescript
interface SonarrStatus {
  connected: boolean;
  series_count: number;
  mapped_count: number;
  last_sync_at: number | null;
}

interface ImportReport {
  imported: number;
  auto_mapped: number;
  unmapped: number;
}

interface SonarrAvailability {
  sonarr_id: number;
  sonarr_title: string;
  monitored: boolean;
  episode_count: number;
  episode_file_count: number;
  next_airing: number | null;
  path: string | null;
  season_count: number;
}

interface SonarrUnmapped {
  sonarr_id: number;
  sonarr_title: string;
  confidence: number;
  title_match: string;
}
```

## UI Design

### SettingsView — "Sonarr" tab

Tab bar: `[General] [Tracking] [AniList] [Sonarr] [About]`

**Disconnected state:**
```
Sonarr Connection
  URL: [________________________] placeholder="http://localhost:8989"
  API Key: [________________________] type="password"
  [Test Connection] [Connect]
```

**Connected state:**
```
Sonarr Connection
  Connected to http://localhost:8989 ✓
  [Disconnect]

Status: 42 series imported, 38 mapped to anime
Last synced: 2 minutes ago
[Sync Now] [Import Series]

Unmapped (3 series need attention)
  "Sousou no Frieren" → [Select anime...]
  "One Piece" → [Select anime...]
  "Fate/strange Fake" → [Select anime...]
```

States: loading (connecting/importing), error (bad URL, bad API key, network), empty (no series), loaded.

### DetailView — Sonarr section

Collapsible section below AniList mapping info. Only shown when anime has a Sonarr mapping.

**Expanded:**
```
▶ Sonarr                                                    [✎ remap]

  Series: "Vinland Saga Season 2"
  Episodes: 24 / 24 files on disk
  Status: Ended  |  Monitored ✓  |  Next airing: —
  Path: D:\Media\Anime\Vinland Saga S02
```

**No files:**
```
▶ Sonarr                                                    [✎ remap]

  Series: "Solo Leveling"
  Episodes: 0 / 12 files on disk
  Status: Continuing  |  Monitored ✓  |  Next airing: Jan 4
  Path: D:\Media\Anime\Solo Leveling
```

### Remap UI

Dropdown/modal for manual mapping:
- List unmapped Sonarr series (from `sonarr_mapping` where `anime_id IS NULL`)
- Per series: searchable dropdown of anime library titles
- Confirm button updates mapping with `user_confirmed = 1`
- Option to unmap (clear `anime_id`) for existing mappings

## Auto-Matching Logic

1. Sonarr series title → `parse_filename(title)` → cleaned title
2. `storage.search_anime_by_title(cleaned_title, 5)` → top 5 candidates
3. Score each candidate:
   - Exact title match: +100
   - Substring match (either direction): +60
   - Levenshtein < 3: +40
   - Episode count within ±2 of Sonarr episode count: +20
   - Year match (if Sonarr year in title): +10
4. Score >= 80: auto-map. Score < 80: store as unmapped.

Reuses `regex` and title-cleaning logic from `engine::parser` (M2).

## Error Handling

| Condition | Behavior |
|-----------|----------|
| Invalid URL format | `Result::Err("invalid url")` — don't store |
| Network error (timeout, DNS) | `Result::Err` with message, don't store settings |
| HTTP 401 (bad API key) | `Result::Err("invalid api key")` — don't store |
| HTTP 404 (wrong URL) | `Result::Err("sonarr not found at url")` |
| HTTP 5xx (Sonarr down) | `Result::Err` with status, retryable |
| JSON parse error | `Result::Err("unexpected sonarr response")` |
| API key not found (disconnected) | Commands return `Result::Err("sonarr not connected")` |

## Storage Methods

New on `Storage`:
- `sonarr_series_upsert(&self, series: &SonarrSeriesDb) -> anyhow::Result<()>`
- `sonarr_series_list(&self) -> anyhow::Result<Vec<SonarrSeriesDb>>`
- `sonarr_series_count(&self) -> anyhow::Result<i64>`
- `sonarr_series_delete_all(&self) -> anyhow::Result<()>`
- `sonarr_mapping_upsert(&self, mapping: &SonarrMappingDb) -> anyhow::Result<()>`
- `sonarr_mapping_by_anime(&self, anime_id: i64) -> anyhow::Result<Option<SonarrMappingDb>>`
- `sonarr_mapping_unmapped(&self) -> anyhow::Result<Vec<SonarrMappingDb>>`
- `sonarr_mapping_count(&self) -> anyhow::Result<i64>`
- `sonarr_mapping_delete_all(&self) -> anyhow::Result<()>`
- `sonarr_availability(&self, anime_id: i64) -> anyhow::Result<Option<SonarrAvailabilityDb>>`

New migration adds `sonarr_series` and `sonarr_mapping` tables.

## File Changes

| File | Action |
|------|--------|
| `next/src-tauri/migrations/<timestamp>_sonarr.sql` | Create |
| `next/src-tauri/src/engine/sonarr/mod.rs` | Create |
| `next/src-tauri/src/engine/sonarr/client.rs` | Create |
| `next/src-tauri/src/engine/sonarr/import.rs` | Create |
| `next/src-tauri/src/engine/availability.rs` | Create |
| `next/src-tauri/src/engine/mod.rs` | Modify — add `pub mod sonarr; pub mod availability;` |
| `next/src-tauri/src/engine/storage.rs` | Modify — add Sonarr CRUD methods + types |
| `next/src-tauri/src/commands.rs` | Modify — add 6 commands + inner functions |
| `next/src-tauri/src/lib.rs` | Modify — register 6 new commands |
| `next/src-tauri/tests/sonarr_client_test.rs` | Create |
| `next/src-tauri/tests/sonarr_import_test.rs` | Create |
| `next/src-tauri/tests/sonarr_commands_test.rs` | Create |
| `next/src-tauri/tests/sonarr_storage_test.rs` | Create |
| `next/src/lib/api.ts` | Modify — add Sonarr types + wrappers |
| `next/src/lib/api.test.ts` | Modify — add Sonarr wrapper tests |
| `next/src/lib/SettingsView.svelte` | Modify — add Sonarr tab |
| `next/src/lib/DetailView.svelte` | Modify — add Sonarr section |
| `next/src/lib/SonarrRemap.svelte` | Create — manual remap UI |

## Tests

| Test file | Coverage |
|-----------|----------|
| `sonarr_client_test.rs` | API key validation (mock or integration), series fetch response parsing, 401/404 error handling |
| `sonarr_import_test.rs` | Auto-match scoring, confidence thresholds, import report counts, unmapped edge cases |
| `sonarr_commands_test.rs` | Inner functions: connect/disconnect, status, availability, remap |
| `sonarr_storage_test.rs` | Table CRUD, cascade on disconnect, upsert behavior |
| `api.test.ts` | New TS wrappers (existing pattern, 6 wrapper tests) |

## Acceptance Criteria

1. User enters Sonarr URL + API key, "Test Connection" validates it
2. "Connect" stores settings with encrypted API key, imports all series
3. High-confidence series auto-map to anime library entries
4. Low-confidence series appear in unmapped list for manual mapping
5. User can remap any series via searchable dropdown
6. Anime detail view shows Sonarr section with file counts and status
7. "Disconnect" clears Sonarr settings and all sonarr_* table data
8. All Rust tests pass, all TS tests pass

## Dependencies

- No new crate dependencies — `reqwest` already in `Cargo.toml`
- Reuses M2 parser for title matching
- Reuses M3 secrets module for DPAPI API key encryption
- Integration with M4 UI components

---

## Spec Self-Review

- No TBD/TODO placeholders
- All 6 commands mapped to implementation modules
- Both UI views (Settings, Detail) specified with states
- Auto-match scoring algorithm defined with thresholds
- Error handling table covers all HTTP/network cases
- Test strategy covers 4 Rust test files + 1 TS test file
- Scope: Sonarr only, single instance, read-only display
- Architecture follows existing AniList pattern for consistency
