# M4 Library and App UI Design

## Purpose

Make AniVault usable as a daily app: dashboard, library view with search/sort, anime detail with inline editing, settings UI, and consistent empty/loading/error states.

## Prerequisites

- M0 Runtime Foundation: DB, state, commands, event bus
- M1 Local Tracking: process scanner, watch history, Now Playing, Mark Watched
- M2 Recognition Engine: parser, matcher, file index, RecognitionCard, KnownFiles
- M3 AniList Integration: auth, import, sync worker, connect/sync UI

## Scope

### In

- State-based navigation (sidebar rail activates views)
- Dashboard with library stats + existing tracking/recognition/sync widgets
- Library table: search, filter by status, sortable columns
- Anime detail: cover, synopsis, progress/status/score editing, watch history
- Settings: tabbed (Tracking, AniList, About)
- Empty/loading/error states for every view
- 4 new backend commands (search library, fetch detail, update entry, stats)

### Out

- Tray behavior (M5)
- Sonarr integration (M6)
- Rebrand/installer (M8)
- AniList OAuth webview (M3 already has token storage; full webview auth deferred)

## Architecture

```
App.svelte
  currentView: 'dashboard' | 'library' | 'detail' | 'settings'
  detailAnimeId: number | null

  sidebar rail (always visible)
    Home → currentView = 'dashboard'
    Library → currentView = 'library'
    Sync → currentView = 'dashboard'  (sync widgets already on dashboard)
    Settings → currentView = 'settings'

  {#if currentView === 'dashboard'}<DashboardView {events} />
  {:else if currentView === 'library'}<LibraryView onSelect={openDetail} />
  {:else if currentView === 'detail'}<DetailView animeId={detailAnimeId} onBack={goDashboard} />
  {:else if ...}<SettingsView />
```

### Views

**DashboardView** — stats cards (total library, watching, completed, dropped counts), NowPlaying, RecognitionCard, AniListConnect, SyncStatus, KnownFiles. All pulled from `get_library_stats` and existing event pipeline.

**LibraryView** — text search input, status dropdown filter, sortable table with columns: cover thumbnail (small), title, status badge, progress (e.g. "12 / 24"), score. Rows sorted by title default; click column header to sort. Click row → open DetailView.

**DetailView** — back button, cover image (large), title, synopsis, episode count, AniList status. Below: progress slider/input + save button, status dropdown (watching/completed/on_hold/dropped/plan_to_watch) + save, score input (0-10). Below: recent watch history table for this anime. Below: AniList mapping info (remote ID, last sync).

**SettingsView** — tab bar: Tracking, AniList, About.
- Tracking tab: enable/disable toggle, player config text
- AniList tab: connect/disconnect, import button, sync status counts
- About tab: app version, database path, migration count

## Backend Commands

| Command | In/Out |
|---------|--------|
| `search_library(query: String, status_filter: Option<String>, limit: i32, offset: i32) -> Vec<LibraryEntry>` | `query` for title search, `status_filter` for status, paginated |
| `get_library_stats() -> LibraryStats` | Returns `{ total: i64, watching: i64, completed: i64, on_hold: i64, dropped: i64, plan_to_watch: i64 }` |
| `fetch_anime_detail(anime_id: i64) -> AnimeDetail` | Returns `{ anime: AnimeRow, list_entry: Option<ListEntryFullRow>, recent_history: Vec<WatchHistoryRow>, tracker_mapping: Option<String> }` |
| `update_list_entry(anime_id: i64, status: Option<String>, watched_episodes: Option<i32>, score: Option<i32>) -> Result<(), String>` | Partial update — only fields provided are changed |

All commands return `Result<T, String>`. Inner functions use `anyhow::Result` and map errors.

## Frontend Types

```typescript
interface LibraryEntry {
  anime_id: number;
  title: string;
  status: string;
  watched_episodes: number;
  episode_count: number | null;
  score: number | null;
  image_url: string | null;
}

interface LibraryStats {
  total: number;
  watching: number;
  completed: number;
  on_hold: number;
  dropped: number;
  plan_to_watch: number;
}

interface AnimeDetail {
  anime: { id: number; titles_json: string; episode_count: number | null; image_url: string | null; synopsis: string | null; status: string | null; last_modified: number };
  list_entry: { status: string; watched_episodes: number; score: number | null; notes: string | null; local_updated: number; remote_updated: number | null } | null;
  recent_history: { id: number; episode: number; file_path: string | null; player: string | null; watched_at: number }[];
  tracker_mapping: string | null;
}
```

## Storage Methods Needed

- `Storage::search_library(query, status_filter, limit, offset) -> Vec<LibraryRow>` — SELECT with LIKE on titles_json and optional status filter on joined list_entry. ORDER BY anime.id LIMIT/OFFSET.
- `Storage::library_stats() -> LibraryStats` — SELECT with CASE/SUM by status.
- `Storage::anime_detail(anime_id) -> (AnimeRow, Option<ListEntryFullRow>, Vec<WatchHistoryRow>, Option<String>)` — 4 queries or one join.
- `Storage::update_list_entry_partial(anime_id, status, watched, score) -> Result<()>` — UPDATE only non-None fields.

## UI States

Every view must handle:
- **Loading**: skeleton/spinner while fetching data
- **Empty**: message + call to action (e.g. "No anime in library. Connect AniList to import.")
- **Error**: error message with retry button
- **Loaded**: normal content rendering

## Test Strategy

| Test file | Coverage |
|-----------|----------|
| `tests/library_commands_test.rs` | search_library, get_library_stats, fetch_anime_detail, update_list_entry |
| `tests/library_storage_test.rs` | New storage methods: search with/without filter, stats counts, partial update |
| Frontend: `api.test.ts` | New wrapper functions for library commands |
| Frontend: no component tests (Svelte testing infra not yet set up) |

---

## Spec Self-Review

- No TBD/TODO placeholders
- All 4 backend commands mapped to storage methods
- All 4 views have explicit component names and content
- Empty/loading/error states required for every view
- Scope focused: library UI only, no tray/Sonarr/rebrand
- Navigation state-based, no router deps
- Storage methods extend existing patterns (LIKE, CASE, param binding)
