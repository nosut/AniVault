# M10 — Taiga Feature Parity Plan

> **Goal:** Bring AniVault to feature parity with Taiga for the scoped-in features. Focus on daily-use workflows: library management, discovery, local file handling.

> **Excluded by design:** MAL, Kitsu, torrents/RSS, HTTP/IRC sharing, proxy, external links, social media, multiple instances, keyboard shortcuts, Discord, browser detection.

---

## Feature List (15 features, 5 phases)

### Phase 1 — Library UX Polish (4 features)

| # | Feature | Description | Files |
|---|---------|-------------|-------|
| 1 | **Now Playing confirmation** | Add confirm button on detected candidates in NowPlaying widget. User clicks a candidate → auto-maps that anime. | `NowPlaying.svelte`, `commands.rs` |
| 2 | **Episode progress bars** | Visual progress bar in library rows showing `watched / total` as a filled bar, not just text. | `LibraryView.svelte` |
| 3 | **Auto-complete on last episode** | When user marks last episode watched, auto-set list status to "Completed". | `commands.rs` (`mark_episode_watched_inner`) |
| 4 | **Image/poster library view** | Toggle between table (current) and poster grid view. Grid shows cover images with title overlay and quick progress. | `LibraryView.svelte` |

### Phase 2 — History & Statistics (2 features)

| # | Feature | Description | Files |
|---|---------|-------------|-------|
| 5 | **Watch history view** | New full-page history log showing all watched episodes with timestamps, anime title, player used. Searchable/filterable. | New `HistoryView.svelte`, `commands.rs` |
| 6 | **Statistics page** | Score distribution chart (bar or histogram), total episodes watched, total watch time (estimates), breakdown by status. | New `StatsView.svelte`, `commands.rs` |

### Phase 3 — Local File Management (3 features)

| # | Feature | Description | Files |
|---|---------|-------------|-------|
| 7 | **Directory monitoring** | User configures library folders. App scans for video files (mkv, mp4, avi), parses filenames via existing M2 parser, shows available episodes per anime. | `engine/library_folders.rs`, `commands.rs`, `SettingsView.svelte` |
| 8 | **Episode file browser** | In Detail view: list of available episode files with ability to click to open/play in default media player. Shows file path, size, format. | `DetailView.svelte`, new `EpisodeFiles.svelte` |
| 9 | **File import on scan** | When directory is monitored and new file detected, auto-create file_index entry and trigger recognition. If confidence high enough, auto-confirm. | `scanner.rs` (extend), `engine/file_watcher.rs` (new) |

### Phase 4 — Discovery & Navigation (3 features)

| # | Feature | Description | Files |
|---|---------|-------------|-------|
| 10 | **Season browser** | Browse AniList seasons (Winter/Spring/Summer/Fall) by year. Grid of anime posters for that season. Filter by genre. Sort by score/popularity. AniList API: `Page(query: "season: SUMMER seasonYear: 2024", type: ANIME) { media {} }` | New `SeasonView.svelte`, `anilist/client.rs` (new query), `commands.rs` |
| 11 | **Drag-and-drop status change** | Drag library row to a status tab pill to change status. Native HTML5 drag events or simple click handling. | `LibraryView.svelte` |
| 12 | **Continue watching section** | On Dashboard: show recently watched anime with progress, one-click resume. Sorts by most recent watch. | `DashboardView.svelte`, `commands.rs` |

### Phase 5 — Power User (3 features)

| # | Feature | Description | Files |
|---|---------|-------------|-------|
| 13 | **Batch editing** | Checkbox selection in library rows. Batch actions: change status, set score, increment progress. | `LibraryView.svelte`, `commands.rs` |
| 14 | **More media players** | Add SMPlayer, Kodi, mpv.net, GOM Player, KMPlayer to player registry. | `player_registry.rs` |
| 15 | **Anime relations / episode redirection** | Query anime-relations data to handle split-cour, OVAs, movies that redirect episode counts. AniList has `relations` field on Media. Use it to detect sequels/prequels and redirect episode tracking. | `anilist/client.rs` (add relations to query), `engine/matcher.rs` |

---

## Implementation Order

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5
```

Each phase produces a testable installer. Phase 1 is highest impact for daily use.

---

## Technical Notes

### Directory Monitoring (Feature 7)
- Use `notify` crate for filesystem events on Windows
- Configurable folders stored in `settings` table as JSON array
- Scan on startup + watch for changes
- Parse filenames via existing `parser.rs` (M2)
- Store in `file_index` table with `anime_id` resolved by matcher
- UI: Settings → Library tab → "Library Folders" section

### Season Browser (Feature 10)
- AniList GraphQL query:
```graphql
query ($season: MediaSeason, $year: Int) {
  Page(page: 1, perPage: 50) {
    media(season: $season, seasonYear: $year, type: ANIME, sort: POPULARITY_DESC) {
      id title { romaji english } coverImage { large } episodes status format
      averageScore popularity
    }
  }
}
```
- MediaSeason enum: WINTER, SPRING, SUMMER, FALL

### Continue Watching (Feature 12)
- Query `watch_history` joined with `list_entry` where `status = 'watching'`
- Order by `watched_at DESC`
- Group by `anime_id`, show latest watched episode
- Click → open Detail view at that anime

---

## Verification

Each phase:
```bash
cd next/src-tauri && cargo check --tests
cd next && npm run check && npm run test
npm run bundle
```
