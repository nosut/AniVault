# M4 Library and App UI Implementation Plan

> **For agentic workers:** Use subagent-driven-development (recommended) to implement task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make AniVault a daily-use desktop app — dashboard, sortable library table, anime detail with inline editing, tabbed settings, empty/loading/error states throughout.

**Architecture:** State-based navigation via `currentView` Svelte reactive variable. New storage methods for library search/stats/detail. New Tauri commands wrapping them. Frontend wrappers in `api.ts`. Five new Svelte views replacing flat component stacking.

**Tech Stack:** Rust, Tauri 2.x, SQLite, Svelte, TypeScript.

## Global Constraints

- Windows desktop app runtime using Tauri, Svelte, Rust, SQLite
- AniList is the only tracker in scope; no MAL or Kitsu code
- M4 scope only: dashboard, library view, detail view, settings, empty/loading/error states
- No tray (M5), Sonarr (M6), rebrand (M8), AniList OAuth webview
- Every fallible command returns `Result<T, String>`
- No commits per user instruction

---

### Task 1: Library Storage Methods

**Files:**
- Modify: `next/src-tauri/src/engine/storage.rs`
- Create: `next/src-tauri/tests/library_storage_test.rs`

**Interfaces:**
- Produces: `Storage::search_library(query: &str, status_filter: Option<&str>, limit: i64, offset: i64) -> anyhow::Result<Vec<LibraryRow>>`
- Produces: `Storage::library_stats() -> anyhow::Result<LibraryStats>`
- Produces: `Storage::anime_detail(anime_id: i64) -> anyhow::Result<AnimeDetailRow>`
- Produces: `Storage::update_list_entry_partial(anime_id: i64, status: Option<&str>, watched_episodes: Option<i32>, score: Option<i32>) -> anyhow::Result<()>`

**Structs needed:**
```rust
pub struct LibraryRow {
    pub anime_id: i64,
    pub title: String,
    pub status: String,
    pub watched_episodes: i32,
    pub episode_count: Option<i32>,
    pub score: Option<i32>,
    pub image_url: Option<String>,
}

pub struct LibraryStats {
    pub total: i64,
    pub watching: i64,
    pub completed: i64,
    pub on_hold: i64,
    pub dropped: i64,
    pub plan_to_watch: i64,
}

pub struct AnimeDetailRow {
    pub anime_id: i64,
    pub titles_json: String,
    pub episode_count: Option<i32>,
    pub image_url: Option<String>,
    pub synopsis: Option<String>,
    pub anime_status: Option<String>,
    pub last_modified: i64,
    pub list_status: Option<String>,
    pub watched_episodes: Option<i32>,
    pub score: Option<i32>,
    pub notes: Option<String>,
    pub local_updated: Option<i64>,
    pub remote_updated: Option<i64>,
    pub tracker_id: Option<String>,
}
```

- [ ] **Step 1: Write failing test `tests/library_storage_test.rs`** — 4 tests: `search_library_finds_by_title` (insert anime, search by partial title, assert found), `library_stats_counts_statuses` (insert 2 watching + 1 completed, assert counts), `anime_detail_returns_full_row` (insert anime + list_entry + tracker_mapping, fetch detail, assert all fields), `update_list_entry_partial_changes_only_status` (upsert entry, call partial with only status, assert status changed but progress unchanged). Use `Tests::new_in_memory()`. Run: FAIL.

- [ ] **Step 2: Implement 4 methods in `storage.rs`** — `search_library` does `SELECT a.id as anime_id, a.titles_json, COALESCE(le.status, 'unlisted') as status, COALESCE(le.watched_episodes, 0) as watched_episodes, a.episode_count, le.score, a.image_url FROM anime a LEFT JOIN list_entry le ON a.id = le.anime_id WHERE a.titles_json LIKE ?1` with optional `AND le.status = ?2` filter, ORDER BY a.id, LIMIT/OFFSET. `library_stats` does `SELECT COALESCE(SUM(CASE WHEN le.status='watching' THEN 1 ELSE 0 END), 0)...` for each status. `anime_detail` selects from anime + LEFT JOIN list_entry + LEFT JOIN tracker_mapping. `update_list_entry_partial` does `UPDATE list_entry SET ... WHERE anime_id = ?` only including fields where `Option` is `Some`, using `watched_episodes = COALESCE(?N, watched_episodes)` pattern.

- [ ] **Step 3: Run tests** — 4 passed. Full suite: all pass.

---

### Task 2: Library Commands + Tauri Registration

**Files:**
- Modify: `next/src-tauri/src/commands.rs`
- Modify: `next/src-tauri/src/lib.rs`
- Create: `next/src-tauri/tests/library_commands_test.rs`

**Interfaces:**
- Consumes: `Storage` new methods (Task 1), `EngineState`
- Produces: `search_library_inner`, `get_library_stats_inner`, `fetch_anime_detail_inner`, `update_list_entry_inner` + 4 `#[tauri::command]` wrappers
- Produces: Names: `search_library`, `get_library_stats`, `fetch_anime_detail`, `update_list_entry`
- Produces: `AnimeDetailResponse` struct — wraps `AnimeDetailRow` fields plus `recent_history: Vec<WatchHistoryRow>` (assembled from `storage.anime_detail()` + `storage.list_recent_watch_history()`)

- [ ] **Step 1: Write failing test `tests/library_commands_test.rs`** — 4 tests: `search_library_returns_matches`, `get_library_stats_returns_counts`, `fetch_anime_detail_returns_data`, `update_list_entry_edits_progress`. Use `fresh_test_state()`. Insert test anime/entries via storage. Call inner functions. Assert results. Run: FAIL.

- [ ] **Step 2: Add inner fns to `commands.rs`** — `search_library_inner(query, status_filter, limit, offset, &EngineState)` calls `state.storage.search_library`, returns `Vec<LibraryRow>`. `get_library_stats_inner` calls `state.storage.library_stats`. `fetch_anime_detail_inner(anime_id, &EngineState)` calls `state.storage.anime_detail` + `state.storage.list_recent_watch_history` for that anime. `update_list_entry_inner(anime_id, status, watched, score, &EngineState)` calls `state.storage.update_list_entry_partial`. Add `#[tauri::command]` wrappers returning `Result<T, String>`.

- [ ] **Step 3: Register in `lib.rs`** — add 4 new commands to `generate_handler!`.

- [ ] **Step 4: Run tests** — 4 passed. Full suite: all pass.

---

### Task 3: Frontend Library Wrappers

**Files:**
- Modify: `next/src/lib/api.ts`
- Modify: `next/src/lib/api.test.ts`

**Interfaces:**
- Consumes: Tauri command names from Task 2
- Produces: TS interfaces `LibraryEntry`, `LibraryStats`, `AnimeDetail`, wrappers `searchLibrary`, `getLibraryStats`, `fetchAnimeDetail`, `updateListEntry`

- [ ] **Step 1: Add types to `api.ts`** — 3 new interfaces matching Rust structs. `LibraryEntry { anime_id, title, status, watched_episodes, episode_count, score, image_url }`. `LibraryStats { total, watching, completed, on_hold, dropped, plan_to_watch }`. `AnimeDetail { anime: {...}, list_entry: {...} | null, recent_history: {...}[], tracker_mapping: string | null }`.

- [ ] **Step 2: Add 4 wrapper functions to `api.ts`** — `searchLibrary(query, statusFilter?, limit?, offset?)`, `getLibraryStats()`, `fetchAnimeDetail(animeId)`, `updateListEntry(animeId, updates?)`. Follow `InvokeFn` pattern.

- [ ] **Step 3: Add 4 test cases to `api.test.ts`** — each test verifies correct `invoke` call with args. Run: `npm run check` clean, `npm run test` → 24 passed.

---

### Task 4: Dashboard View

**Files:**
- Create: `next/src/lib/DashboardView.svelte`
- Modify: `next/src/App.svelte` (integrate dashboard)

**Interfaces:**
- Consumes: `getLibraryStats` (Task 3), existing `NowPlaying`, `RecognitionCard`, `AniListConnect`, `SyncStatus`, `KnownFiles`
- Props: `events: EngineEvent[]`

- [ ] **Step 1: Create `DashboardView.svelte`** — calls `getLibraryStats` on mount. Displays 6 stat cards (total, watching, completed, on_hold, dropped, plan_to_watch) in a flex grid. Below: `NowPlaying`, `RecognitionCard`, `AniListConnect`, `SyncStatus`, `KnownFiles`. All wrapped from existing components. Has loading state (skeleton cards), error state (message + retry), empty state (when total=0: "No anime yet. Connect AniList to import."). Re-exports events prop to child components.

- [ ] **Step 2: Wire in `App.svelte`** — import `DashboardView`. Dashboard defaults as `{#if currentView === 'dashboard'}`. Pass `latestEvents` as `events` prop. Keep existing event polling unchanged.

- [ ] **Step 3: Run** — `npm run check` clean. `npm run test` → still 24 passed (no new component tests, Svelte testing not set up).

---

### Task 5: Library View

**Files:**
- Create: `next/src/lib/LibraryView.svelte`
- Modify: `next/src/App.svelte` (wire library view)

**Interfaces:**
- Consumes: `searchLibrary` (Task 3)
- Produces: `LibraryView.svelte` — search bar, status filter dropdown, sortable table
- Dispatch: `onSelect` event with `anime_id: number`

- [ ] **Step 1: Create `LibraryView.svelte`** — search input (debounced 300ms, binds to query reactive var), status filter `<select>` (all/watching/completed/on_hold/dropped/plan_to_watch), sortable table. Columns: thumbnail (24x24, fallback placeholder), title (sortable), status badge (colored pill), progress ("X / Y" format, sortable), score (sortable). Table uses `searchLibrary` on mount and when query/filter changes. Click row dispatches `onSelect` with `anime_id`. Empty state: "No anime found." Loading: skeleton rows.

- [ ] **Step 2: Wire in `App.svelte`** — import `LibraryView`. On `onSelect`, set `currentView = 'detail'` and `detailAnimeId = event.detail.anime_id`.

- [ ] **Step 3: Run** — `npm run check` clean. `npm run test` → 24 passed.

---

### Task 6: Detail View

**Files:**
- Create: `next/src/lib/DetailView.svelte`
- Modify: `next/src/App.svelte` (wire detail view)

**Interfaces:**
- Consumes: `fetchAnimeDetail`, `updateListEntry` (Task 3)
- Props: `animeId: number`
- Dispatch: `onBack` event

- [ ] **Step 1: Create `DetailView.svelte`** — on mount, calls `fetchAnimeDetail(animeId)`. Displays: back button, large cover image (or placeholder), title, synopsis, episode count, AniList media status. Below: progress editor (number input + numeric up/down buttons, min 0, max episode_count, "Save" button calling `updateListEntry` with `watched_episodes`), status dropdown ("Save" calling `updateListEntry` with `status`), score input (0-10, "Save" calling `updateListEntry` with `score`). After any successful save, re-fetch via `fetchAnimeDetail(animeId)` to refresh all data. Below: "Recent Watch History" table (episode, file, player, date). Below: AniList mapping (remote ID if present). Loader: skeleton detail card. Error: message + retry. Empty list_entry: "Add to list" prompt.

- [ ] **Step 2: Wire in `App.svelte`** — `{#if currentView === 'detail'}` renders `<DetailView {animeId} on:back={() => currentView = 'library'} />`.

- [ ] **Step 3: Run** — `npm run check` clean. `npm run test` → 24 passed.

---

### Task 7: Settings View (Designer-Owned)

**Files:**
- Create: `next/src/lib/SettingsView.svelte`
- Modify: `next/src/App.svelte` (wire settings view)

**Interfaces:**
- Consumes: `getEngineStatus`, settings get/set, `AniListConnect`, `SyncStatus`
- Props: none (self-contained)

- [ ] **Step 1: Create `SettingsView.svelte`** — tabs: Tracking, AniList, About. Tracking tab: enable/disable toggle (reads/writes `tracking.enabled` setting). AniList tab: reuses `AniListConnect` and `SyncStatus` components inline. About tab: app name, version (hardcoded "0.1.0"), database path (from `getEngineStatus`), migration count. Tab switching via reactive `activeTab` variable. Clean card-based layout matching existing style.

- [ ] **Step 2: Wire in `App.svelte`** — `{#if currentView === 'settings'}` renders `<SettingsView />`.

- [ ] **Step 3: Run** — `npm run check` clean. `npm run test` → 24 passed.

---

### Task 8: View Switching + Rail Nav Activation

**Files:**
- Modify: `next/src/App.svelte` (rework sidebar navigation, add view switching)

**Interfaces:**
- Consumes: all views from Tasks 4-7
- Produces: working sidebar rail with active state, view switching logic

- [ ] **Step 1: Rewrite `App.svelte` navigation** — add `currentView` reactive variable (`'dashboard'`), `detailAnimeId` (`null`). Sidebar buttons set `currentView` on click. `class:active` bound to `currentView === 'home'` etc. Nav items: Dashboard, Library, Settings (remove Tracking and Sync — their widgets are on dashboard). Add conditional view rendering: `{#if currentView === 'dashboard'}<DashboardView ...>{:else if ...}`. Keep event polling in App for single-owner drain. Keep `handleConfirmed` callback pattern.

- [ ] **Step 2: Verify** — `npm run check` clean. `npm run test` → 24 passed. Manual: sidebar buttons switch views, dashboard shows stats + widgets, library shows table, clicking row opens detail, back returns to library, settings shows tabs.

---
