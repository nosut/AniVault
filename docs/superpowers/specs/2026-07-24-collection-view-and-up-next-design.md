# Collection view, series size & Up Next prompt — design

**Date:** 2026-07-24
**Status:** Approved, ready for implementation planning

## Summary

Three related additions to AniVault, centered on the library the user has
actually downloaded to disk:

1. **Collection view** — a new Plex/Jellyfin-style poster wall that browses only
   the series with episode files on disk, distinct from the full-list Library.
2. **Series disk size** — total on-disk size of a series shown on its detail
   page.
3. **Up Next prompt** — when the user finishes an episode and the next one is on
   disk, prompt them to play it. Prompt only; never autoplay.

These are independent enough to build and review as separate units but share one
theme and are specced together.

---

## 1. Collection view

### Purpose

A screen to browse the physical collection — decide what to watch and get quick
access to play it — the way Plex/Jellyfin present a library. The existing
**Library** view (full tracked list, table/grid, management-oriented) stays
unchanged. Collection is image-first and scoped to what is on disk.

### Navigation

- New rail item labeled **Collection**, placed immediately **after Library** in
  `App.svelte`'s `navItems` / `navIcons`.
- Suggested icon: a Lucide icon distinct from Library (e.g. `HardDrive` or
  `Clapperboard`); pick during implementation.
- New `View` value `'collection'` wired through `App.svelte` (render block +
  `handleCollectionSelect` reusing the existing detail-navigation pattern so a
  poster click opens `DetailView`, mirroring `handleLibrarySelect`).

### What appears

Only anime that have **at least one downloaded episode file** mapped in the file
index (`file_index` rows with a non-null `anime_id`, not ignored, path present).

### Backend

New Tauri command **`get_collection() -> Vec<CollectionEntry>`**, aggregating per
anime from the file index joined to anime metadata + list status.

```
CollectionEntry {
  anime_id: i64,
  title: String,
  image_url: Option<String>,
  status: String,              // list status (watching/completed/…/unlisted)
  watched_episodes: i64,
  episode_count: Option<i64>,  // known total, if any
  downloaded_count: i64,       // distinct episodes present on disk
  max_downloaded_episode: i64, // highest episode number on disk
  next_unwatched_episode: Option<i64>, // first downloaded ep > watched; drives Play next + badge
  new_count: i64,              // downloaded episodes beyond watched_episodes
  last_indexed_at: i64,        // most recent files.indexed_at; drives "Recently added" sort
}
```

- **Complete** = `episode_count` is known and `max_downloaded_episode >=
  episode_count` (all episodes present). Otherwise **Incomplete**.
- Query stays cheap: it does **not** stat files or compute byte size (see §2).
- Add a matching TypeScript `CollectionEntry` interface and `getCollection()`
  wrapper in `next/src/lib/api.ts`.

### Frontend — `CollectionView.svelte`

- Image-first **poster wall**: `grid-template-columns: repeat(auto-fill,
  minmax(~10rem, 1fr))`, reusing the poster-card styling already present in
  `LibraryView`'s grid mode and the shared `tokens.css`.
- **Toolbar:** search input, filter chips, sort control.
  - **Filter chips:** All · New · Complete · Incomplete.
    - *New* = `new_count > 0`.
    - *Complete* / *Incomplete* per the definition above.
  - **Sort:** Recently added (default), Title, Progress. "Recently added" uses the
    most recent `indexed_at` among the anime's files (include a
    `last_indexed_at` field on `CollectionEntry` to support this sort).
  - Persist filter/sort/search to `localStorage` under `anivault-collection-*`
    keys, mirroring `LibraryView`'s persistence approach.
- **Each poster:** cover image (fallback placeholder), title, a thin completeness
  bar (green when complete, amber when partial), and an "N new" badge when
  `new_count > 0`.
- **Hover:** an instant **▶ Play next** control that opens the file for
  `next_unwatched_episode` (via `getEpisodeFiles` + `openEpisodeFile`, or a
  path returned from the aggregate — see below).
- **Right-click / ⋯ context menu** (reuse the menu pattern already in
  `LibraryView.svelte`): Play next · Play previous · Open folder · Full details ·
  Set status ▸ · Remove from library. No "reclaim space" entry.
- **Click a poster** → dispatch `select` → `App.svelte` opens `DetailView`.
- Live updates: subscribe to the app-wide `events` prop (already passed to
  Library) and refresh on `LibraryUpdated` / `ProgressAdvanced`, matching
  `LibraryView`'s event handling.

To resolve concrete file paths for Play next/previous the view can call the
existing `getEpisodeFiles(anime_id)` on demand (as `LibraryView` does) rather
than embedding paths in the aggregate. Keep the aggregate metadata-only.

### Out of scope

- Reclaim-space / delete-files-from-disk (explicitly dropped).
- Showing size per card on the wall (size is detail-page only, §2).

---

## 2. Series disk size on the detail page

### Purpose

See at a glance how much disk space a series occupies.

### Backend

New Tauri command **`get_series_disk_size(anime_id) -> u64`**:

- Loads the anime's mapped files (`file_index_by_anime`).
- Stats each existing path; sums byte sizes. Missing/moved files contribute 0.
- Computed on demand — no schema migration, always reflects current disk state.

Rationale for on-demand vs. a stored `size_bytes` column: avoids a migration +
backfill, and a detail page opens rarely enough that stat-ing a few dozen files
is negligible. (A stored column remains a future option if library-wide size
totals are ever wanted.)

### Frontend — `DetailView.svelte`

- Call `getSeriesDiskSize(animeId)` when the detail loads (alongside the existing
  `getEpisodeFiles`).
- Display near the title/meta area, e.g. **"12.4 GB on disk"**.
- Add a bytes → human-readable helper (`formatBytes`) in a small testable module
  (e.g. extend an existing `*Ui.ts` or add `fileSize.ts`) with unit tests.
- Add the `getSeriesDiskSize` wrapper to `api.ts`.

---

## 3. Up Next prompt

### Purpose

When the user finishes an episode and the next episode's file is on disk, prompt
them to play it. **Prompt only — never autoplay.**

### Trigger

- The app already polls `drainEngineEvents()` every 3s in `App.svelte` and holds
  `latestEvents`. When a `ProgressAdvanced` event marks anime X at episode N:
  - Query **`get_up_next(anime_id) -> Option<UpNext>`**, which returns the next
    unwatched **downloaded** episode's info, or null.
  - If non-null, raise the Up Next prompt for `{ anime_id, episode }`.
- **Deduplicate:** prompt at most once per `(anime_id, episode)`. Dismissing
  suppresses that pair; playing clears it. Track the last prompted pair so the
  3s poll can't re-raise it.

```
UpNext {
  anime_id: i64,
  title: String,
  image_url: Option<String>,
  episode: i64,       // the next episode to play (N+1 or first unwatched on disk)
  file_path: String,  // resolved path to play
}
```

`get_up_next` returns the first downloaded episode `> watched_episodes` for that
anime (normally N+1) together with its file path, so the frontend has everything
needed without a second call.

### Behavior

- The prompt offers **Play** (calls `openEpisodeFile(file_path)`) and
  **Dismiss**. No countdown, no automatic launch under any circumstance.

### Surfaces — both, independently toggleable

Two settings (persisted via existing `getSetting`/`setSetting`, exposed in
`SettingsView.svelte`), both **default on**:

- `up_next_toast_enabled`
- `up_next_notification_enabled`

1. **In-app toast** — a floating card (bottom corner of the app window) showing
   the poster thumb, "Up Next — {title} · Ep {episode}", with Play / Dismiss.
   Rendered by `App.svelte`.
2. **Windows notification** — a native OS toast via the **Tauri notification
   plugin** (add `@tauri-apps/plugin-notification` + the Rust plugin and request
   permission if not already present). Works while AniVault is minimized / in
   tray. Clicking it plays the episode; if OS-level action handling proves too
   limited, fall back to focusing the app and showing the in-app toast. Document
   whichever behavior ships.

### Logic module

- Put the pure decision logic in a small testable module **`upNext.ts`**
  (matching the `homeUi.ts` / `updateUi.ts` pattern): given the latest events,
  the last-prompted pair, and an `UpNext` lookup, decide whether to raise a new
  prompt and what it contains. Unit-test the dedupe and trigger rules.
- `App.svelte` owns the prompt state, renders the toast, fires the notification
  (respecting the two setting toggles), and clears state on play/dismiss.

---

## Verification

`npm run verify` in `next/` (typecheck + vitest + `cargo check --tests`), plus
`cargo test` in `next/src-tauri` for the new commands. New pure modules
(`upNext.ts`, `formatBytes`) get unit tests. Manual check in the running app for
the poster wall, size display, and the Up Next prompt on both surfaces.

## Release

Per project convention, once the work is confirmed and a build is requested: bump
the patch version in all four places, build the installer, commit feature work
then a `chore: release` commit, tag, push, and cut the GitHub release. Do not
build/release unprompted.
