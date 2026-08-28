# Calendar watched indicator

**Date:** 2026-07-31
**Status:** Approved, ready for implementation planning

## Problem

The calendar shows every airing episode for followed shows, but nothing
distinguishes an episode you have already watched from one still waiting. The
only per-entry marker today is download status (`episodeMarker()` in
`next/src/lib/calendarUi.ts`): green dot = downloaded, hollow amber = aired but
missing, faint = upcoming.

## Decisions

1. **Granularity: per episode.** The marker applies to the exact episode a
   calendar cell represents, not to the show as a whole. Two cells for the same
   show on different days can differ.
2. **Evidence: union of local history and list progress.** An episode counts as
   watched when it appears in the `watch_history` table *or* its number is at or
   below `list_entry.watched_episodes`. History covers in-app and out-of-order
   plays; list progress covers episodes watched before or outside AniVault and
   synced from AniList.
3. **Scope: Calendar only** — both the month grid and the agenda list.
   `DashboardView` is untouched.
4. **Presentation: additive marker, not a replacement.** The download dot stays;
   watched adds a `✓` and dims the row. Download state and watched state are
   independent facts (watched-and-still-on-disk vs. watched-and-deleted), so
   they get independent affordances.

## Backend

### Data model

`CalendarEntry` (`next/src-tauri/src/commands.rs`) gains:

```rust
#[serde(default)]
pub watched: bool,
```

`#[serde(default)]` lets already-persisted `calendar.cache` JSON deserialize
instead of failing and forcing an unnecessary refetch.

Like `has_file`, `watched` is never trusted from cache. It reflects local DB
state and is recomputed on every `get_calendar_inner` call, including cache
hits.

### Computation

`attach_download_status()` is currently called from two sites — the cache-hit
early return and the tail of `get_calendar_inner`. Fold the new work into a
single `attach_local_status()` that fills `has_file` and `watched` together, so
the two call sites cannot drift apart as they would with a second parallel
function.

Per anime present in the entry set (following the existing per-anime
`file_index_by_anime` loop; the followed-show set is dozens of rows):

- new `Storage::watch_history_episodes(anime_id) -> Vec<i32>`
  (`SELECT DISTINCT episode FROM watch_history WHERE anime_id = ?1`)
- existing `Storage::get_list_entry(anime_id)` for `watched_episodes`

The decision itself lives in a pure function mirroring the existing
`apply_has_file`:

```rust
fn apply_watched(
    entries: &mut [CalendarEntry],
    history: &HashSet<(i64, i32)>,
    progress: &HashMap<i64, i32>,
)
```

An entry is watched when `next_episode` is `Some(ep)` and either
`history.contains(&(anime_id, ep))` or `ep <= progress[anime_id]`. Entries with
`next_episode: None` are never watched.

### Error handling

A failing `watch_history_episodes` or `get_list_entry` query degrades that show
to "not watched" rather than failing the calendar load, matching how
`attach_download_status` already swallows `file_index_by_anime` errors.

## Frontend

### `next/src/lib/calendarUi.ts`

- `episodeMarker()` is **unchanged**. It means download status and only download
  status. `DashboardView.svelte:167` shares it, and leaving it alone is what
  keeps the Dashboard out of scope.
- `markerLabels` moves here from its component-local const in `CalendarView`.
- New pure `entryLabel(entry, marker)` composes the accessible label, e.g.
  `"Frieren Ep 6 (Downloaded, watched)"`, so label text is assertable without
  mounting a component.

### `next/src/lib/CalendarView.svelte`

Month grid entry (`.cal-day-entry`):

- `class:watched={entry.watched}`
- `{#if entry.watched}<span class="ep-check" aria-hidden="true">✓</span>{/if}`
  after the episode number
- `aria-label={entryLabel(entry, marker)}`

Agenda row (`.agenda-row`): same `class:watched`, with the `✓` placed before the
countdown pill. Note the agenda only lists entries airing from the start of
today onward, so a watched row there is the narrower case of an episode that
aired earlier today and has already been watched — real, but uncommon.

Hover tooltip: a `✓ Watched` line when `tip.entry.watched`.

Styles:

- `.cal-day-entry.watched, .agenda-row.watched { opacity: 0.62; }`
- `.cal-day-entry.watched:hover, .cal-day-entry.watched:focus,
   .agenda-row.watched:hover { opacity: 1; }` — a dimmed title stays readable
  when the user goes looking for it.
- `.ep-check { flex-shrink: 0; color: var(--color-success); font-size: 0.62rem;
   line-height: 1; margin-left: 0.15rem; }`

The `✓` is `aria-hidden` because the state is already carried by the label.

At the existing ≤900px breakpoint `.cal-entry-title` is hidden; the `✓` stays,
so narrow windows still render dot + `✓` + episode number.

## Testing

**Rust** (`next/src-tauri`, `cargo test`):

- `apply_watched` unit tests: episode in history; `ep <= progress`;
  `ep > progress` and absent from history; `next_episode: None`; no list entry
  for the anime.
- The existing `calendar_entry` test builder needs the new field.

**Vitest** (`next/`):

- `calendarUi.test.ts`: `entryLabel` cases across the marker states, watched and
  unwatched.
- `CalendarView.test.ts`: add `watched` to the `weekly()` fixture with a couple
  of episodes marked; assert `.ep-check` and `.cal-day-entry.watched` counts in
  the month grid.

**Full gate:** `npm run verify` in `next/` (typecheck + vitest +
`cargo check --tests`).

## Release

User-facing change, so it ships with a patch version bump across
`next/package.json`, `next/package-lock.json`, `next/src-tauri/Cargo.toml` and
`next/src-tauri/tauri.conf.json`. No build, push, or GitHub release happens
without the user explicitly asking.
