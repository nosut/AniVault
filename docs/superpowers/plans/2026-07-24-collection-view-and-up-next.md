# Collection view, series size & Up Next prompt — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Plex-style "Collection" view of downloaded series, show each series' on-disk size on its detail page, and prompt (never autoplay) to play the next episode when it becomes watchable.

**Architecture:** Three independent parts. Each adds a thin Rust `#[tauri::command]` backed by a pure, unit-tested helper (mirroring the existing `ready_to_watch_entries` pattern), a typed wrapper in `next/src/lib/api.ts`, pure Svelte-agnostic UI helpers in `next/src/lib/*.ts` (mirroring `homeUi.ts`/`seasonUi.ts`), and Svelte UI that reuses existing `LibraryView`/`SettingsView`/`NowPlaying` patterns and design tokens.

**Tech Stack:** Rust + Tauri v2 + sqlx (SQLite) backend; Svelte + TypeScript + Vite frontend; vitest (frontend) and `cargo test` (backend). OS notifications via the already-present `tauri-plugin-notification`.

## Global Constraints

- Windows-only Tauri desktop app; active code lives under `next/`.
- Rust commands must be registered in `next/src-tauri/src/lib.rs` inside `tauri::generate_handler![…]`.
- Serde structs serialize with snake_case field names (no rename); TypeScript interfaces must match exactly.
- `api.ts` functions take a trailing `invokeFn: InvokeFn = tauriInvoke` param so tests can inject a fake.
- Verification: `npm run verify` in `next/` (typecheck + vitest + `cargo check --tests`); `cargo test` in `next/src-tauri`.
- Do **not** bump the version or build/release as part of this plan — that happens only when the user asks for a build.
- No reclaim-space / delete-file-from-disk feature. Never autoplay — the Up Next feature only ever *prompts*.

---

# Part A — Collection view

## Task A1: `get_collection` backend command

**Files:**
- Modify: `next/src-tauri/src/commands.rs` (add struct, pure helper, `_inner`, command, tests)
- Modify: `next/src-tauri/src/lib.rs:198-279` (register command)

**Interfaces:**
- Consumes: `storage.search_library(query, status_filter, limit, offset) -> Vec<LibraryRow>`; `storage.file_index_by_anime(anime_id) -> Vec<FileIndexRow>`. `LibraryRow` fields: `anime_id: i64, title: String, status: String, watched_episodes: i32, episode_count: Option<i32>, score: Option<i32>, image_url: Option<String>, season, season_year, airing_status`. `FileIndexRow` fields: `file_path: String, anime_id: Option<i64>, episode: Option<i32>, confidence: i32, mapping_source: MappingSource, indexed_at: i64, ignored: bool`.
- Produces: `get_collection() -> Vec<CollectionEntry>` and the pure helper `collection_entry(&LibraryRow, &[FileIndexRow]) -> Option<CollectionEntry>`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `commands.rs` (near `ready_to_watch_*` tests). Add a small `FileIndexRow` builder if one isn't already present in the test module:

```rust
    fn lib_row(anime_id: i64, title: &str, watched: i32, episode_count: Option<i32>) -> LibraryRow {
        LibraryRow {
            anime_id,
            title: title.to_string(),
            status: "watching".to_string(),
            watched_episodes: watched,
            episode_count,
            score: None,
            image_url: None,
            season: None,
            season_year: None,
            airing_status: None,
        }
    }

    fn file_row(anime_id: i64, episode: Option<i32>, indexed_at: i64, ignored: bool) -> FileIndexRow {
        FileIndexRow {
            file_path: format!("C:/lib/a{anime_id}/e{}.mkv", episode.unwrap_or(0)),
            anime_id: Some(anime_id),
            episode,
            confidence: 100,
            mapping_source: MappingSource::from_db("manual"),
            indexed_at,
            ignored,
        }
    }

    #[test]
    fn collection_entry_summarizes_downloaded_episodes() {
        let row = lib_row(1, "Frieren", 2, Some(4));
        let files = vec![
            file_row(1, Some(1), 100, false),
            file_row(1, Some(2), 200, false),
            file_row(1, Some(3), 300, false),
            file_row(1, Some(4), 400, false),
        ];
        let e = collection_entry(&row, &files).expect("has files");
        assert_eq!(e.downloaded_count, 4);
        assert_eq!(e.max_downloaded_episode, 4);
        assert_eq!(e.next_unwatched_episode, Some(3));
        assert_eq!(e.new_count, 2);
        assert_eq!(e.last_indexed_at, 400);
    }

    #[test]
    fn collection_entry_skips_anime_with_no_usable_files() {
        let row = lib_row(2, "No Files", 0, Some(12));
        assert!(collection_entry(&row, &[]).is_none());
        // ignored / episode-less files don't count
        let junk = vec![file_row(2, None, 50, false), file_row(2, Some(1), 60, true)];
        assert!(collection_entry(&row, &junk).is_none());
    }

    #[test]
    fn collection_entry_dedupes_and_handles_unknown_total() {
        let row = lib_row(3, "Dupes", 0, None);
        let files = vec![
            file_row(3, Some(1), 10, false),
            file_row(3, Some(1), 20, false), // duplicate episode number
            file_row(3, Some(2), 30, false),
        ];
        let e = collection_entry(&row, &files).expect("has files");
        assert_eq!(e.downloaded_count, 2, "duplicate episode counted once");
        assert_eq!(e.next_unwatched_episode, Some(1));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd next/src-tauri && cargo test collection_entry`
Expected: FAIL — `cannot find function collection_entry` / `cannot find type CollectionEntry`.

- [ ] **Step 3: Write the struct, pure helper, `_inner`, and command**

Add near `ReadyToWatchEntry` (around `commands.rs:1801`):

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct CollectionEntry {
    pub anime_id: i64,
    pub title: String,
    pub image_url: Option<String>,
    pub status: String,
    pub watched_episodes: i32,
    pub episode_count: Option<i32>,
    pub downloaded_count: i32,
    pub max_downloaded_episode: i32,
    pub next_unwatched_episode: Option<i32>,
    pub new_count: i32,
    pub last_indexed_at: i64,
}

/// Summarize one anime's downloaded episodes into a Collection row. Returns
/// `None` when the anime has no usable (non-ignored, numbered) files on disk —
/// those anime are not part of the downloaded collection.
pub fn collection_entry(row: &LibraryRow, files: &[FileIndexRow]) -> Option<CollectionEntry> {
    let mut eps: Vec<i32> = files
        .iter()
        .filter(|f| !f.ignored)
        .filter_map(|f| f.episode)
        .collect();
    if eps.is_empty() {
        return None;
    }
    let last_indexed_at = files
        .iter()
        .filter(|f| !f.ignored)
        .map(|f| f.indexed_at)
        .max()
        .unwrap_or(0);
    eps.sort_unstable();
    eps.dedup();
    let watched = row.watched_episodes;
    Some(CollectionEntry {
        anime_id: row.anime_id,
        title: row.title.clone(),
        image_url: row.image_url.clone(),
        status: row.status.clone(),
        watched_episodes: watched,
        episode_count: row.episode_count,
        downloaded_count: eps.len() as i32,
        max_downloaded_episode: *eps.last().unwrap(),
        next_unwatched_episode: eps.iter().copied().find(|&e| e > watched),
        new_count: eps.iter().filter(|&&e| e > watched).count() as i32,
        last_indexed_at,
    })
}

pub async fn get_collection_inner(state: &EngineState) -> anyhow::Result<Vec<CollectionEntry>> {
    // search_library("", None, …) returns every listed anime plus any anime with
    // non-ignored files (see its WHERE clause), so it is a superset of the
    // collection; collection_entry drops the ones without files.
    let library = state.storage.search_library("", None, 5000, 0).await?;
    let mut out = Vec::new();
    for row in &library {
        let files = state.storage.file_index_by_anime(row.anime_id).await?;
        if let Some(entry) = collection_entry(row, &files) {
            out.push(entry);
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn get_collection(
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<CollectionEntry>, String> {
    get_collection_inner(&state).await.map_err(command_error)
}
```

- [ ] **Step 4: Register the command**

In `next/src-tauri/src/lib.rs`, add inside `generate_handler![…]` (alphabetically near `get_calendar`):

```rust
            commands::get_collection,
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd next/src-tauri && cargo test collection_entry && cargo check`
Expected: PASS; `cargo check` clean.

- [ ] **Step 6: Commit**

```bash
git add next/src-tauri/src/commands.rs next/src-tauri/src/lib.rs
git commit -m "feat: get_collection command for downloaded-series view"
```

---

## Task A2: `getCollection` API wrapper + type

**Files:**
- Modify: `next/src/lib/api.ts` (add interface + wrapper near `ReadyToWatchEntry`, ~line 569)
- Modify: `next/src/lib/api.test.ts` (add wrapper test)

**Interfaces:**
- Produces: `CollectionEntry` interface and `getCollection(invokeFn?) -> Promise<CollectionEntry[]>`.

- [ ] **Step 1: Write the failing test**

Add to `api.test.ts` (follow the existing pattern of injecting a fake `invokeFn`):

```ts
import { getCollection } from './api';

test('getCollection calls get_collection and returns entries', async () => {
  const fake = vi.fn().mockResolvedValue([
    { anime_id: 1, title: 'Frieren', image_url: null, status: 'watching', watched_episodes: 2,
      episode_count: 4, downloaded_count: 4, max_downloaded_episode: 4,
      next_unwatched_episode: 3, new_count: 2, last_indexed_at: 400 },
  ]);
  const res = await getCollection(fake);
  expect(fake).toHaveBeenCalledWith('get_collection');
  expect(res[0].title).toBe('Frieren');
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd next && npx vitest run src/lib/api.test.ts -t getCollection`
Expected: FAIL — `getCollection` is not exported.

- [ ] **Step 3: Implement the interface + wrapper**

Add to `api.ts` near `ReadyToWatchEntry`:

```ts
export interface CollectionEntry {
  anime_id: number;
  title: string;
  image_url: string | null;
  status: string;
  watched_episodes: number;
  episode_count: number | null;
  downloaded_count: number;
  max_downloaded_episode: number;
  next_unwatched_episode: number | null;
  new_count: number;
  last_indexed_at: number;
}

export function getCollection(invokeFn: InvokeFn = tauriInvoke): Promise<CollectionEntry[]> {
  return invokeFn<CollectionEntry[]>('get_collection');
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd next && npx vitest run src/lib/api.test.ts -t getCollection`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/api.ts next/src/lib/api.test.ts
git commit -m "feat: getCollection api wrapper"
```

---

## Task A3: `collectionUi.ts` filter/sort helpers

**Files:**
- Create: `next/src/lib/collectionUi.ts`
- Create: `next/src/lib/collectionUi.test.ts`

**Interfaces:**
- Consumes: `CollectionEntry` from `./api`.
- Produces: `CollectionFilter`, `CollectionSort`, `isComplete(e)`, `filterCollection(entries, filter, query)`, `sortCollection(entries, sort)`.

- [ ] **Step 1: Write the failing test**

```ts
// next/src/lib/collectionUi.test.ts
import { describe, it, expect } from 'vitest';
import { isComplete, filterCollection, sortCollection } from './collectionUi';
import type { CollectionEntry } from './api';

function e(p: Partial<CollectionEntry>): CollectionEntry {
  return {
    anime_id: 1, title: 'A', image_url: null, status: 'watching', watched_episodes: 0,
    episode_count: null, downloaded_count: 1, max_downloaded_episode: 1,
    next_unwatched_episode: 1, new_count: 1, last_indexed_at: 0, ...p,
  };
}

describe('isComplete', () => {
  it('is true only when every episode is on disk', () => {
    expect(isComplete(e({ episode_count: 12, max_downloaded_episode: 12 }))).toBe(true);
    expect(isComplete(e({ episode_count: 12, max_downloaded_episode: 8 }))).toBe(false);
    expect(isComplete(e({ episode_count: null, max_downloaded_episode: 8 }))).toBe(false);
  });
});

describe('filterCollection', () => {
  const list = [
    e({ anime_id: 1, title: 'Frieren', episode_count: 4, max_downloaded_episode: 4, new_count: 0 }),
    e({ anime_id: 2, title: 'Spy Family', episode_count: 12, max_downloaded_episode: 6, new_count: 3 }),
  ];
  it('filters by new', () => {
    expect(filterCollection(list, 'new', '').map((x) => x.anime_id)).toEqual([2]);
  });
  it('filters by complete / incomplete', () => {
    expect(filterCollection(list, 'complete', '').map((x) => x.anime_id)).toEqual([1]);
    expect(filterCollection(list, 'incomplete', '').map((x) => x.anime_id)).toEqual([2]);
  });
  it('applies a case-insensitive title query on top of the filter', () => {
    expect(filterCollection(list, 'all', 'spy').map((x) => x.anime_id)).toEqual([2]);
  });
});

describe('sortCollection', () => {
  const list = [
    e({ anime_id: 1, title: 'Bravo', last_indexed_at: 100, watched_episodes: 5 }),
    e({ anime_id: 2, title: 'Alpha', last_indexed_at: 300, watched_episodes: 1 }),
  ];
  it('recent sorts by last_indexed_at desc', () => {
    expect(sortCollection(list, 'recent').map((x) => x.anime_id)).toEqual([2, 1]);
  });
  it('title sorts alphabetically', () => {
    expect(sortCollection(list, 'title').map((x) => x.anime_id)).toEqual([2, 1]);
  });
  it('progress sorts by watched desc', () => {
    expect(sortCollection(list, 'progress').map((x) => x.anime_id)).toEqual([1, 2]);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd next && npx vitest run src/lib/collectionUi.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the helpers**

```ts
// next/src/lib/collectionUi.ts
import type { CollectionEntry } from './api';

export type CollectionFilter = 'all' | 'new' | 'complete' | 'incomplete';
export type CollectionSort = 'recent' | 'title' | 'progress';

/** Every episode of a known-length series is on disk. */
export function isComplete(e: CollectionEntry): boolean {
  return e.episode_count != null && e.episode_count > 0 && e.max_downloaded_episode >= e.episode_count;
}

export function filterCollection(
  entries: CollectionEntry[],
  filter: CollectionFilter,
  query: string,
): CollectionEntry[] {
  const q = query.trim().toLowerCase();
  return entries.filter((e) => {
    if (q && !e.title.toLowerCase().includes(q)) return false;
    switch (filter) {
      case 'new': return e.new_count > 0;
      case 'complete': return isComplete(e);
      case 'incomplete': return !isComplete(e);
      default: return true;
    }
  });
}

export function sortCollection(entries: CollectionEntry[], sort: CollectionSort): CollectionEntry[] {
  const list = [...entries];
  switch (sort) {
    case 'title': list.sort((a, b) => a.title.localeCompare(b.title)); break;
    case 'progress': list.sort((a, b) => b.watched_episodes - a.watched_episodes); break;
    case 'recent':
    default: list.sort((a, b) => b.last_indexed_at - a.last_indexed_at); break;
  }
  return list;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd next && npx vitest run src/lib/collectionUi.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/collectionUi.ts next/src/lib/collectionUi.test.ts
git commit -m "feat: collection filter/sort helpers"
```

---

## Task A4: `CollectionView.svelte` + nav wiring

**Files:**
- Create: `next/src/lib/CollectionView.svelte`
- Modify: `next/src/App.svelte` (nav item, `View` type, render block, select handler)

**Interfaces:**
- Consumes: `getCollection`, `getEpisodeFiles`, `openEpisodeFile`, `openContainingFolder`, `deleteAnime`, `updateListEntry` from `./api`; `filterCollection`, `sortCollection`, `isComplete` from `./collectionUi`; the app-wide `events: EngineEvent[]` prop.
- Produces: dispatches `select` → `{ anime_id }` (App opens `DetailView`).

- [ ] **Step 1: Build the component**

Create `CollectionView.svelte`. Reuse the poster-card grid, `.ep-download-bar`, and context-menu markup/styles from `LibraryView.svelte` (poster grid: `LibraryView.svelte:610-663`; context menu: `LibraryView.svelte:877-903`; associated styles `.poster-*`, `.ctx-*`, `.badge`, `.progress-*`). Script essentials:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { getCollection, getEpisodeFiles, openEpisodeFile, openContainingFolder,
    deleteAnime, updateListEntry, type CollectionEntry, type FileIndexEntry, type EngineEvent } from './api';
  import { filterCollection, sortCollection, isComplete,
    type CollectionFilter, type CollectionSort } from './collectionUi';
  import { LayoutGrid, Play, FolderOpen, Info, Trash2, ChevronRight, ChevronLeft } from 'lucide-svelte';

  export let events: EngineEvent[] = [];
  const dispatch = createEventDispatcher<{ select: { anime_id: number } }>();

  function loadPref(k: string, f: string) { try { return localStorage.getItem(k) ?? f; } catch { return f; } }
  function persistPref(k: string, v: string) { try { localStorage.setItem(k, v); } catch {} }

  let entries: CollectionEntry[] = [];
  let loading = false;
  let error = '';
  let query = loadPref('anivault-collection-query', '');
  let filter = loadPref('anivault-collection-filter', 'all') as CollectionFilter;
  let sort = loadPref('anivault-collection-sort', 'recent') as CollectionSort;
  let episodeFilesMap = new Map<number, FileIndexEntry[]>();

  $: persistPref('anivault-collection-query', query);
  $: persistPref('anivault-collection-filter', filter);
  $: persistPref('anivault-collection-sort', sort);
  $: visible = sortCollection(filterCollection(entries, filter, query), sort);

  const FILTERS: { value: CollectionFilter; label: string }[] = [
    { value: 'all', label: 'All' }, { value: 'new', label: 'New' },
    { value: 'complete', label: 'Complete' }, { value: 'incomplete', label: 'Incomplete' },
  ];

  async function load() {
    loading = true; error = '';
    try {
      entries = await getCollection();
      for (const e of entries.slice(0, 100)) {
        try {
          const files = await getEpisodeFiles(e.anime_id);
          if (files.length > 0) episodeFilesMap.set(e.anime_id, files);
        } catch {}
      }
      episodeFilesMap = new Map(episodeFilesMap);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally { loading = false; }
  }

  function completeness(e: CollectionEntry): number {
    if (e.episode_count && e.episode_count > 0) return Math.min(100, (e.max_downloaded_episode / e.episode_count) * 100);
    return 100;
  }

  function playNext(e: CollectionEntry) {
    const files = episodeFilesMap.get(e.anime_id);
    if (!files) return;
    const ep = e.next_unwatched_episode ?? e.max_downloaded_episode;
    const f = files.find((x) => (x.episode ?? 0) === ep) ?? files[0];
    if (f) openEpisodeFile(f.file_path);
  }

  function open(e: CollectionEntry) { dispatch('select', { anime_id: e.anime_id }); }

  // Reload when the file index or progress changes (mirrors LibraryView).
  $: if (events && events.some((ev) => 'LibraryUpdated' in ev || 'ProgressAdvanced' in ev)) void load();

  onMount(load);
</script>
```

Markup: a `.lib-header` toolbar with a search input (bound to `query`), the `FILTERS` chips (button per filter, `class:active={filter === f.value}`, `on:click={() => filter = f.value}`), and a sort `<select bind:value={sort}>` with options Recently added (`recent`) / Title (`title`) / Progress (`progress`). Below it, a `.poster-grid` over `visible` (keyed by `anime_id`). Each poster card:
- cover image or placeholder (copy `.poster-thumb` markup from `LibraryView.svelte:624-628`),
- an "N new" badge when `entry.new_count > 0`,
- a hover **▶ Play next** button calling `playNext(entry)` with `on:click|stopPropagation`,
- a completeness bar at the bottom: reuse `.ep-download-bar`, or a single bar `style="width: {completeness(entry)}%"` colored green via `class:complete={isComplete(entry)}` / amber otherwise,
- `on:click={() => open(entry)}`, `on:contextmenu` opening the context menu.

Context menu items (reuse `.ctx-menu` markup/handlers from `LibraryView.svelte:877-903`, dropping any reclaim/rescan you don't need): **Play next**, **Play previous** (find highest downloaded ep ≤ watched), **Open folder** (`openContainingFolder` on the first file path), **Full details** (`open(entry)`), **Set status ▸** (submenu calling `updateListEntry(anime_id, { status })` then `load()`), **Remove from library** (`deleteAnime(anime_id)` then drop from `entries`). Add a `loading` skeleton and an empty state ("No downloaded series yet.").

- [ ] **Step 2: Wire it into App.svelte**

In `next/src/App.svelte`:
- Add `import CollectionView from './lib/CollectionView.svelte';` (near the other view imports, ~line 8).
- Add `HardDrive` to the `lucide-svelte` import (~line 18-29).
- Extend the `View` type (line 31) to include `'collection'`.
- Add to `navItems` (after the `library` entry, line 35): `{ id: 'collection' as View, label: 'Collection' },`.
- Add to `navIcons` (line 44): `collection: HardDrive,`.
- Add a select handler mirroring `handleLibrarySelect`:

```ts
  function handleCollectionSelect(event: CustomEvent<{ anime_id: number }>) {
    previousView = currentView;
    detailAnimeId = event.detail.anime_id;
    currentView = 'detail';
  }
```

- Add a render branch after the `library` branch (~line 228):

```svelte
    {:else if currentView === 'collection'}
      <CollectionView events={latestEvents} on:select={handleCollectionSelect} />
```

- [ ] **Step 3: Verify build + typecheck**

Run: `cd next && npm run verify`
Expected: typecheck passes, all vitest suites pass, `cargo check --tests` clean.

- [ ] **Step 4: Manual smoke check**

Run the app (`/run` or the project's dev command). Confirm the **Collection** rail item appears below Library, shows only series with files, filter chips + sort work, hover **Play next** launches the next episode, right-click actions work, and clicking a poster opens the detail page.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/CollectionView.svelte next/src/App.svelte
git commit -m "feat: Collection view (poster wall of downloaded series)"
```

---

# Part B — Series disk size on the detail page

## Task B1: `get_series_disk_size` backend command

**Files:**
- Modify: `next/src-tauri/src/commands.rs` (pure `sum_file_sizes`, command, test)
- Modify: `next/src-tauri/src/lib.rs` (register)

**Interfaces:**
- Consumes: `storage.file_index_by_anime(anime_id) -> Vec<FileIndexRow>`.
- Produces: `get_series_disk_size(anime_id: i64) -> u64`; pure `sum_file_sizes(&[String]) -> u64`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `commands.rs`:

```rust
    #[test]
    fn sum_file_sizes_totals_existing_files_and_ignores_missing() {
        let dir = std::env::temp_dir().join(format!("av_size_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.bin");
        let b = dir.join("b.bin");
        std::fs::write(&a, vec![0u8; 1000]).unwrap();
        std::fs::write(&b, vec![0u8; 2500]).unwrap();
        let missing = dir.join("gone.bin").to_string_lossy().to_string();

        let paths = vec![
            a.to_string_lossy().to_string(),
            b.to_string_lossy().to_string(),
            missing,
        ];
        assert_eq!(sum_file_sizes(&paths), 3500);
        std::fs::remove_dir_all(&dir).ok();
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd next/src-tauri && cargo test sum_file_sizes`
Expected: FAIL — `cannot find function sum_file_sizes`.

- [ ] **Step 3: Implement helper + command**

Add to `commands.rs` (near the other file commands):

```rust
/// Sum the on-disk byte size of the given paths. Missing/unreadable files
/// contribute 0 so a moved file never breaks the total.
pub fn sum_file_sizes(paths: &[String]) -> u64 {
    paths
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum()
}

#[tauri::command]
pub async fn get_series_disk_size(
    anime_id: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<u64, String> {
    let files = state
        .storage
        .file_index_by_anime(anime_id)
        .await
        .map_err(command_error)?;
    let paths: Vec<String> = files
        .into_iter()
        .filter(|f| !f.ignored)
        .map(|f| f.file_path)
        .collect();
    Ok(sum_file_sizes(&paths))
}
```

Register in `lib.rs` `generate_handler![…]`:

```rust
            commands::get_series_disk_size,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd next/src-tauri && cargo test sum_file_sizes && cargo check`
Expected: PASS; clean.

- [ ] **Step 5: Commit**

```bash
git add next/src-tauri/src/commands.rs next/src-tauri/src/lib.rs
git commit -m "feat: get_series_disk_size command"
```

---

## Task B2: `getSeriesDiskSize` wrapper + `formatBytes`

**Files:**
- Modify: `next/src/lib/api.ts` (wrapper)
- Create: `next/src/lib/fileSize.ts`
- Create: `next/src/lib/fileSize.test.ts`
- Modify: `next/src/lib/api.test.ts` (wrapper test)

**Interfaces:**
- Produces: `getSeriesDiskSize(animeId, invokeFn?) -> Promise<number>`; `formatBytes(bytes: number) -> string`.

- [ ] **Step 1: Write the failing tests**

```ts
// next/src/lib/fileSize.test.ts
import { describe, it, expect } from 'vitest';
import { formatBytes } from './fileSize';

describe('formatBytes', () => {
  it('formats across units', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(5 * 1024 * 1024)).toBe('5.0 MB');
    expect(formatBytes(12.4 * 1024 * 1024 * 1024)).toBe('12.4 GB');
  });
  it('rounds large values within a unit to whole numbers', () => {
    expect(formatBytes(250 * 1024 * 1024)).toBe('250 MB');
  });
});
```

Add to `api.test.ts`:

```ts
import { getSeriesDiskSize } from './api';

test('getSeriesDiskSize passes animeId and returns bytes', async () => {
  const fake = vi.fn().mockResolvedValue(3500);
  const res = await getSeriesDiskSize(42, fake);
  expect(fake).toHaveBeenCalledWith('get_series_disk_size', { animeId: 42 });
  expect(res).toBe(3500);
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd next && npx vitest run src/lib/fileSize.test.ts src/lib/api.test.ts -t getSeriesDiskSize`
Expected: FAIL — modules/exports missing.

- [ ] **Step 3: Implement**

```ts
// next/src/lib/fileSize.ts
const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];

/** Human-readable byte size, e.g. 12.4 GB. One decimal below 100 within a unit. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const i = Math.min(UNITS.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const val = bytes / Math.pow(1024, i);
  const text = i === 0 || val >= 100 ? String(Math.round(val)) : val.toFixed(1);
  return `${text} ${UNITS[i]}`;
}
```

Add to `api.ts` (near `getEpisodeFiles`, ~line 728):

```ts
export function getSeriesDiskSize(animeId: number, invokeFn: InvokeFn = tauriInvoke): Promise<number> {
  return invokeFn<number>('get_series_disk_size', { animeId });
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd next && npx vitest run src/lib/fileSize.test.ts src/lib/api.test.ts -t getSeriesDiskSize`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/fileSize.ts next/src/lib/fileSize.test.ts next/src/lib/api.ts next/src/lib/api.test.ts
git commit -m "feat: getSeriesDiskSize wrapper + formatBytes helper"
```

---

## Task B3: Show size on the detail page

**Files:**
- Modify: `next/src/lib/DetailView.svelte`

**Interfaces:**
- Consumes: `getSeriesDiskSize` from `./api`; `formatBytes` from `./fileSize`.

- [ ] **Step 1: Load the size when the detail loads**

In `DetailView.svelte`:
- Add to the imports: `getSeriesDiskSize` (to the `./api` import) and `import { formatBytes } from './fileSize';`.
- Add state near `episodeFiles` (~line 23): `let diskSizeBytes: number | null = null;`.
- Where the detail/episode files are loaded for `animeId` (the block around `episodeFiles = files;`, ~line 237-241), also fetch size:

```ts
      getSeriesDiskSize(animeId).then((b) => { if (requestedId === animeId) diskSizeBytes = b; }).catch(() => {});
```

Reset it alongside the other per-anime state (set `diskSizeBytes = null` when a new `animeId` load starts, matching how `episodeFiles` is reset).

- [ ] **Step 2: Display it**

Near the title/meta area (by the episode-count / status metadata, e.g. around `DetailView.svelte:563`), add:

```svelte
{#if diskSizeBytes !== null && diskSizeBytes > 0}
  <span class="disk-size" title="Total size of downloaded files on disk">{formatBytes(diskSizeBytes)} on disk</span>
{/if}
```

Add a small muted style (mirror an existing meta/`.badge` style):

```css
  .disk-size { font-size: 0.8rem; color: var(--color-muted); font-variant-numeric: tabular-nums; }
```

- [ ] **Step 3: Verify + smoke check**

Run: `cd next && npm run verify`
Expected: passes. Then open a series with files in the app and confirm "12.4 GB on disk" (or similar) appears; a series whose files are missing shows nothing.

- [ ] **Step 4: Commit**

```bash
git add next/src/lib/DetailView.svelte
git commit -m "feat: show series disk size on the detail page"
```

---

# Part C — Up Next prompt (never autoplay)

## Task C1: `get_up_next` + `notify_up_next` backend

**Files:**
- Modify: `next/src-tauri/src/engine/storage.rs` (add `up_next_meta`)
- Modify: `next/src-tauri/src/commands.rs` (struct, pure helper, `_inner`, two commands, tests)
- Modify: `next/src-tauri/src/lib.rs` (register both commands)

**Interfaces:**
- Consumes: `storage.file_index_by_anime`; new `storage.up_next_meta(anime_id) -> Option<(String, Option<String>, i32)>` = `(display_title, image_url, watched_episodes)`; `state.app_handle: Option<AppHandle>` + `NotificationExt` (already imported in `commands.rs:24`).
- Produces: `get_up_next(anime_id) -> Option<UpNext>`; `notify_up_next(title: String, episode: i32) -> ()`; pure `up_next_from(anime_id, title, image_url, watched, &[FileIndexRow]) -> Option<UpNext>`.

- [ ] **Step 1: Write the failing test (pure helper)**

Add to the `tests` module in `commands.rs` (reuse the `file_row` builder from Task A1):

```rust
    #[test]
    fn up_next_picks_first_downloaded_episode_after_watched() {
        let files = vec![
            file_row(1, Some(11), 10, false),
            file_row(1, Some(12), 20, false),
            file_row(1, Some(13), 30, false),
            file_row(1, Some(14), 40, true), // ignored — skip
        ];
        let un = up_next_from(1, "Show".into(), None, 12, &files).expect("has next");
        assert_eq!(un.episode, 13);
        assert!(un.file_path.contains("e13"));
    }

    #[test]
    fn up_next_none_when_no_unwatched_file() {
        let files = vec![file_row(1, Some(1), 10, false), file_row(1, Some(2), 20, false)];
        assert!(up_next_from(1, "Show".into(), None, 5, &files).is_none());
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd next/src-tauri && cargo test up_next`
Expected: FAIL — `cannot find function up_next_from` / type `UpNext`.

- [ ] **Step 3: Add the storage getter**

In `storage.rs`, add (near `get_list_entry`, ~line 461):

```rust
    /// Minimal metadata for the Up Next prompt: display title (English→romaji
    /// fallback), poster, and watched-episode count. `None` if the anime row is
    /// absent.
    pub async fn up_next_meta(
        &self,
        anime_id: i64,
    ) -> anyhow::Result<Option<(String, Option<String>, i32)>> {
        let row = sqlx::query(
            "SELECT COALESCE(NULLIF(json_extract(a.titles_json, '$.english'), ''), \
                    json_extract(a.titles_json, '$.romaji'), 'Unknown') as title, \
                    a.image_url as image_url, \
                    COALESCE(le.watched_episodes, 0) as watched_episodes \
             FROM anime a LEFT JOIN list_entry le ON a.id = le.anime_id \
             WHERE a.id = ?1",
        )
        .bind(anime_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| {
            (
                r.get::<String, _>("title"),
                r.get::<Option<String>, _>("image_url"),
                r.get::<i32, _>("watched_episodes"),
            )
        }))
    }
```

- [ ] **Step 4: Add the struct, pure helper, `_inner`, and commands**

In `commands.rs`:

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct UpNext {
    pub anime_id: i64,
    pub title: String,
    pub image_url: Option<String>,
    pub episode: i32,
    pub file_path: String,
}

/// First downloaded, non-ignored episode strictly greater than `watched`.
pub fn up_next_from(
    anime_id: i64,
    title: String,
    image_url: Option<String>,
    watched: i32,
    files: &[FileIndexRow],
) -> Option<UpNext> {
    let mut cands: Vec<(i32, &str)> = files
        .iter()
        .filter(|f| !f.ignored)
        .filter_map(|f| f.episode.map(|e| (e, f.file_path.as_str())))
        .collect();
    cands.sort_by_key(|(e, _)| *e);
    cands
        .into_iter()
        .find(|(e, _)| *e > watched)
        .map(|(episode, path)| UpNext {
            anime_id,
            title,
            image_url,
            episode,
            file_path: path.to_string(),
        })
}

pub async fn get_up_next_inner(
    state: &EngineState,
    anime_id: i64,
) -> anyhow::Result<Option<UpNext>> {
    let Some((title, image_url, watched)) = state.storage.up_next_meta(anime_id).await? else {
        return Ok(None);
    };
    let files = state.storage.file_index_by_anime(anime_id).await?;
    Ok(up_next_from(anime_id, title, image_url, watched, &files))
}

#[tauri::command]
pub async fn get_up_next(
    anime_id: i64,
    state: tauri::State<'_, EngineState>,
) -> Result<Option<UpNext>, String> {
    get_up_next_inner(&state, anime_id).await.map_err(command_error)
}

#[tauri::command]
pub async fn notify_up_next(
    title: String,
    episode: i32,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    if let Some(ref handle) = state.app_handle {
        let _ = handle
            .notification()
            .builder()
            .title("Up Next")
            .body(format!("{title} — Episode {episode} is ready to play"))
            .show();
    }
    Ok(())
}
```

Register both in `lib.rs` `generate_handler![…]`:

```rust
            commands::get_up_next,
            commands::notify_up_next,
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd next/src-tauri && cargo test up_next && cargo check`
Expected: PASS; clean.

- [ ] **Step 6: Commit**

```bash
git add next/src-tauri/src/engine/storage.rs next/src-tauri/src/commands.rs next/src-tauri/src/lib.rs
git commit -m "feat: get_up_next + notify_up_next backend"
```

---

## Task C2: `getUpNext` + `notifyUpNext` wrappers

**Files:**
- Modify: `next/src/lib/api.ts`
- Modify: `next/src/lib/api.test.ts`

**Interfaces:**
- Produces: `UpNext` interface; `getUpNext(animeId, invokeFn?) -> Promise<UpNext | null>`; `notifyUpNext(title, episode, invokeFn?) -> Promise<void>`.

- [ ] **Step 1: Write the failing test**

```ts
import { getUpNext, notifyUpNext } from './api';

test('getUpNext passes animeId and returns the prompt or null', async () => {
  const fake = vi.fn().mockResolvedValue({ anime_id: 1, title: 'Frieren', image_url: null, episode: 13, file_path: 'C:/x/e13.mkv' });
  const res = await getUpNext(1, fake);
  expect(fake).toHaveBeenCalledWith('get_up_next', { animeId: 1 });
  expect(res?.episode).toBe(13);
});

test('notifyUpNext forwards title and episode', async () => {
  const fake = vi.fn().mockResolvedValue(undefined);
  await notifyUpNext('Frieren', 13, fake);
  expect(fake).toHaveBeenCalledWith('notify_up_next', { title: 'Frieren', episode: 13 });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd next && npx vitest run src/lib/api.test.ts -t UpNext`
Expected: FAIL — exports missing.

- [ ] **Step 3: Implement**

Add to `api.ts`:

```ts
export interface UpNext {
  anime_id: number;
  title: string;
  image_url: string | null;
  episode: number;
  file_path: string;
}

export function getUpNext(animeId: number, invokeFn: InvokeFn = tauriInvoke): Promise<UpNext | null> {
  return invokeFn<UpNext | null>('get_up_next', { animeId });
}

export function notifyUpNext(title: string, episode: number, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('notify_up_next', { title, episode });
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd next && npx vitest run src/lib/api.test.ts -t UpNext`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/api.ts next/src/lib/api.test.ts
git commit -m "feat: getUpNext + notifyUpNext wrappers"
```

---

## Task C3: `upNext.ts` trigger/dedupe logic

**Files:**
- Create: `next/src/lib/upNext.ts`
- Create: `next/src/lib/upNext.test.ts`

**Interfaces:**
- Consumes: `EngineEvent`, `UpNext` from `./api`.
- Produces: `latestProgressAdvance(events) -> { anime_id: number; new_episode: number } | null`; `samePrompt(a, b) -> boolean` where the args are `{ anime_id, episode } | null`.

- [ ] **Step 1: Write the failing test**

```ts
// next/src/lib/upNext.test.ts
import { describe, it, expect } from 'vitest';
import { latestProgressAdvance, samePrompt } from './upNext';
import type { EngineEvent } from './api';

const pa = (anime_id: number, new_episode: number): EngineEvent =>
  ({ ProgressAdvanced: { anime_id, old_episode: new_episode - 1, new_episode, source: 'auto' } } as EngineEvent);

describe('latestProgressAdvance', () => {
  it('returns null when there is no ProgressAdvanced event', () => {
    expect(latestProgressAdvance([])).toBeNull();
    expect(latestProgressAdvance([{ LibraryUpdated: { indexed: 1, removed: 0 } } as EngineEvent])).toBeNull();
  });
  it('returns the last ProgressAdvanced in the batch', () => {
    expect(latestProgressAdvance([pa(1, 3), pa(2, 5)])).toEqual({ anime_id: 2, new_episode: 5 });
  });
});

describe('samePrompt', () => {
  it('treats identical anime+episode as the same prompt', () => {
    expect(samePrompt({ anime_id: 1, episode: 13 }, { anime_id: 1, episode: 13 })).toBe(true);
    expect(samePrompt({ anime_id: 1, episode: 13 }, { anime_id: 1, episode: 14 })).toBe(false);
    expect(samePrompt(null, { anime_id: 1, episode: 13 })).toBe(false);
    expect(samePrompt(null, null)).toBe(false);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd next && npx vitest run src/lib/upNext.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement**

```ts
// next/src/lib/upNext.ts
import type { EngineEvent } from './api';

export interface PromptKey {
  anime_id: number;
  episode: number;
}

/** The most recent ProgressAdvanced in a polled event batch, if any. */
export function latestProgressAdvance(
  events: EngineEvent[],
): { anime_id: number; new_episode: number } | null {
  for (let i = events.length - 1; i >= 0; i--) {
    const ev = events[i];
    if ('ProgressAdvanced' in ev) {
      return { anime_id: ev.ProgressAdvanced.anime_id, new_episode: ev.ProgressAdvanced.new_episode };
    }
  }
  return null;
}

/** Same anime + episode — used to avoid re-prompting for a prompt already shown. */
export function samePrompt(a: PromptKey | null, b: PromptKey | null): boolean {
  if (!a || !b) return false;
  return a.anime_id === b.anime_id && a.episode === b.episode;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd next && npx vitest run src/lib/upNext.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/upNext.ts next/src/lib/upNext.test.ts
git commit -m "feat: up-next trigger/dedupe helpers"
```

---

## Task C4: Up Next settings toggles

**Files:**
- Modify: `next/src/lib/SettingsView.svelte`

**Interfaces:**
- Consumes: `getSetting`, `setSetting` (already imported).
- Produces: persisted settings `up_next_toast_enabled` and `up_next_notification_enabled` (both default `true`).

- [ ] **Step 1: Add state + load/save handlers**

In `SettingsView.svelte` script, mirror the existing `tracking.enabled` pattern (`SettingsView.svelte:182-209`):

```ts
  let upNextToast = true;
  let upNextNotify = true;

  async function loadUpNext() {
    try {
      upNextToast = (await getSetting<boolean>('up_next_toast_enabled')) ?? true;
      upNextNotify = (await getSetting<boolean>('up_next_notification_enabled')) ?? true;
    } catch { /* defaults stand */ }
  }

  async function toggleUpNextToast() {
    upNextToast = !upNextToast;
    try { await setSetting('up_next_toast_enabled', upNextToast); }
    catch { upNextToast = !upNextToast; }
  }

  async function toggleUpNextNotify() {
    upNextNotify = !upNextNotify;
    try { await setSetting('up_next_notification_enabled', upNextNotify); }
    catch { upNextNotify = !upNextNotify; }
  }
```

Call `loadUpNext()` from the existing `onMount` load sequence (where `loadTracking()` is called).

- [ ] **Step 2: Add the toggle rows**

In the tracking/playback settings section, add two `.toggle-row`s using the existing `role="switch"` markup (`SettingsView.svelte:435-446`):

```svelte
<div class="toggle-row">
  <span class="label">Show an in-app "Up Next" prompt when the next episode is ready</span>
  <button type="button" role="switch" aria-checked={upNextToast} class="switch" on:click={toggleUpNextToast}>
    <span class="switch-thumb" />
  </button>
</div>
<div class="toggle-row">
  <span class="label">Also send a Windows notification for "Up Next"</span>
  <button type="button" role="switch" aria-checked={upNextNotify} class="switch" on:click={toggleUpNextNotify}>
    <span class="switch-thumb" />
  </button>
</div>
```

- [ ] **Step 3: Verify + smoke check**

Run: `cd next && npm run verify`
Expected: passes. In the app, toggle both switches and confirm they persist across a reload (Settings re-open reflects saved state).

- [ ] **Step 4: Commit**

```bash
git add next/src/lib/SettingsView.svelte
git commit -m "feat: Up Next settings toggles"
```

---

## Task C5: Up Next prompt wiring + toast in App.svelte

**Files:**
- Modify: `next/src/App.svelte`

**Interfaces:**
- Consumes: `getUpNext`, `notifyUpNext`, `openEpisodeFile`, `getSetting`, `type UpNext` from `./lib/api`; `latestProgressAdvance`, `samePrompt`, `type PromptKey` from `./lib/upNext`.

- [ ] **Step 1: Add prompt state + reaction**

In `App.svelte` script:
- Extend the `./lib/api` import with `getUpNext, notifyUpNext, openEpisodeFile, getSetting, type UpNext`.
- Add `import { latestProgressAdvance, samePrompt, type PromptKey } from './lib/upNext';`.
- Add state:

```ts
  let upNextPrompt: UpNext | null = null;
  let lastPromptKey: PromptKey | null = null;

  async function maybePromptUpNext(events: EngineEvent[]) {
    const adv = latestProgressAdvance(events);
    if (!adv) return;
    const toastOn = (await getSetting<boolean>('up_next_toast_enabled')) ?? true;
    const notifyOn = (await getSetting<boolean>('up_next_notification_enabled')) ?? true;
    if (!toastOn && !notifyOn) return;
    const next = await getUpNext(adv.anime_id);
    if (!next) return;
    const key: PromptKey = { anime_id: next.anime_id, episode: next.episode };
    if (samePrompt(key, lastPromptKey)) return; // already surfaced this one
    lastPromptKey = key;
    if (toastOn) upNextPrompt = next;
    if (notifyOn) void notifyUpNext(next.title, next.episode);
  }

  function playUpNext() {
    if (upNextPrompt) openEpisodeFile(upNextPrompt.file_path);
    upNextPrompt = null;
  }
  function dismissUpNext() { upNextPrompt = null; }
```

- Drive it from the existing event flow. `pollEvents()` already sets `latestEvents = events;` — add right after that assignment: `void maybePromptUpNext(events);`.

- [ ] **Step 2: Render the toast**

Inside the `.content` section (or at the end of `<main class="shell">`), add a fixed-position toast:

```svelte
{#if upNextPrompt}
  <div class="up-next-toast" role="dialog" aria-label="Up next">
    {#if upNextPrompt.image_url}
      <img class="un-thumb" src={upNextPrompt.image_url} alt="" />
    {/if}
    <div class="un-body">
      <span class="un-eyebrow">Up Next</span>
      <span class="un-title">{upNextPrompt.title}</span>
      <span class="un-ep">Episode {upNextPrompt.episode}</span>
    </div>
    <div class="un-actions">
      <button class="un-play" on:click={playUpNext}>▶ Play</button>
      <button class="un-dismiss" aria-label="Dismiss" on:click={dismissUpNext}>×</button>
    </div>
  </div>
{/if}
```

Add styles (reuse tokens; mirror `.update-banner`/`.now-playing-card`):

```css
  .up-next-toast {
    position: fixed; right: 1.25rem; bottom: 1.25rem; z-index: 50;
    display: flex; align-items: center; gap: 0.75rem; max-width: 22rem;
    padding: 0.7rem 0.9rem; border-radius: 12px;
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    background: rgba(16, 21, 32, 0.98); box-shadow: 0 12px 30px rgba(0,0,0,0.5);
  }
  .un-thumb { width: 40px; height: 56px; object-fit: cover; border-radius: 6px; flex: 0 0 auto; }
  .un-body { display: flex; flex-direction: column; min-width: 0; }
  .un-eyebrow { font-size: 0.66rem; font-weight: 800; letter-spacing: 0.12em; text-transform: uppercase; color: var(--color-accent); }
  .un-title { font-size: 0.85rem; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .un-ep { font-size: 0.72rem; color: var(--color-muted); }
  .un-actions { display: flex; align-items: center; gap: 0.4rem; margin-left: auto; }
  .un-play { border: 1px solid rgba(var(--color-accent-rgb), 0.35); border-radius: 999px; padding: 0.35rem 0.75rem; background: rgba(var(--color-accent-rgb), 0.18); color: var(--color-text); font-size: 0.78rem; cursor: pointer; white-space: nowrap; }
  .un-play:hover { background: rgba(var(--color-accent-rgb), 0.3); }
  .un-dismiss { border: none; background: transparent; color: var(--color-muted); font-size: 1.1rem; cursor: pointer; padding: 0 0.25rem; }
  .un-dismiss:hover { color: var(--color-text); }
```

- [ ] **Step 3: Verify + smoke check**

Run: `cd next && npm run verify`
Expected: passes. Then in the app: with a series that has episode N watched and N+1 downloaded, mark N watched (or let auto-detect advance it) and confirm the toast appears once (not re-appearing every 3s poll), **Play** opens the next file, **×** dismisses it, and the Windows notification fires only when its toggle is on. Turning both settings off suppresses everything.

- [ ] **Step 4: Commit**

```bash
git add next/src/App.svelte
git commit -m "feat: Up Next prompt toast + notification wiring"
```

---

## Self-review notes (coverage check)

- **Collection view** → Tasks A1–A4 (backend aggregate, wrapper, filter/sort helpers, poster-wall UI + nav). Filters All/New/Complete/Incomplete and sorts Recently added/Title/Progress are in `collectionUi.ts` (A3) and wired in A4. Play next / context menu / click-to-detail in A4.
- **Series disk size** → Tasks B1–B3 (command, wrapper + `formatBytes`, detail-page display).
- **Up Next prompt (never autoplay)** → Tasks C1–C5. Trigger on `ProgressAdvanced` (C3/C5), `get_up_next` resolves the next downloaded episode (C1), dedupe via `samePrompt`/`lastPromptKey` (C3/C5), two independently-toggleable surfaces — in-app toast + Windows notification (C4/C5). No autoplay anywhere: the toast only ever offers a Play button the user clicks.
- **Reuse:** poster-card/context-menu markup and styles copied from `LibraryView.svelte`; toggle rows from `SettingsView.svelte`; notification via existing `tauri-plugin-notification`. No new dependencies.
- **Type consistency:** `CollectionEntry`, `UpNext` fields match one-for-one between the Rust structs (Tasks A1/C1) and the TS interfaces (A2/C2); `getSeriesDiskSize` returns a plain `number` (u64) matching `formatBytes(bytes: number)`.
```
