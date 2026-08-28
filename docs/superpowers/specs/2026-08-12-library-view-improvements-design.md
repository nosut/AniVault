# Library view improvements — design

_Date: 2026-08-12. Baseline: v1.0.19 (`6afd239f`)._

Mockup: https://claude.ai/code/artifact/f16b4199-9299-443b-9f1d-9e4a670a2f49

Four independent changes to the Library view. Three are user-visible features;
one is a bug fix that has been silently hiding two thirds of the library.

All four are frontend-only except where noted. `storage.rs` is not modified.

---

## 1. Remove the "Unlisted" category

### Why

`unlisted` is not a status. `search_library` synthesises it with
`COALESCE(le.status, 'unlisted')` (`storage.rs:1485`) for any anime that has
mapped local files but no `list_entry` row. Nothing ever writes it.

It leaks into the UI in three places, and one is a latent data bug: **every
status tab is a drag-and-drop target**. `handleDrop` writes the tab's `value`
as the new status, so dragging a row onto *Unlisted* writes the literal string
`"unlisted"` into `list_entry.status` — a value nothing else in the app
produces, and which would sync to AniList as a real status. Removing the tab
closes that path.

### What changes

| Location | Change |
|---|---|
| `statusOptions` (L179–187) | Delete the `unlisted` entry. Removes tab + drop target. |
| `tabCount()` (L172) | Delete the `'unlisted'` case (the subtraction that derived its count). |
| `matchesActiveFilter()` (L120) | Delete the `unlisted` branch. |
| Badge, grid card (L631) | Render a muted `—` when `entry.status === 'unlisted'`. |
| Badge, table row (L821) | Same. |
| `storage.rs` | **Unchanged.** The COALESCE and its filter branch stay — the marker is honest internally and `library_storage_test.rs` covers it. |

### Decisions

- **Rows are kept.** Those anime are real and file-backed; they stay visible
  under All and in search, and stay playable. Only the label goes.
- **The badge goes with the tab.** Removing only the tab would leave those
  shows still reading "UNLISTED" under All — the exact thing being removed.
- **A muted em-dash**, not an empty cell, so the Status column stays aligned.

### Migration hazard

Anyone whose last-selected tab was Unlisted has `"unlisted"` persisted in
`localStorage` under `anivault-library-filter`. After this change that value
matches no tab, so the view loads with no tab highlighted and an empty-looking
list. **`loadPersistedFilter()` must map an unrecognised value to `null`
(All).** Validate against `statusOptions` rather than special-casing the string,
so any future removal is covered too.

---

## 2. Group Plan to Watch by season

### Why

Plan to Watch holds shows that **have not aired yet** — it is a schedule, not a
backlog. (This was explicitly confirmed by the user; do not re-introduce
backlog handling.) It already defaults to sorting by season ascending, so the
nearest season is at the top, but 127 rows run together as one undifferentiated
list. Finding "what's on next season" means reading down the Season column
until the text changes.

### What changes

A **Group by season** toggle appears next to the existing Grid/Table and
Compact controls, gated to the Plan to Watch tab by the same condition that
already governs `showSeason` (`statusFilter === 'plan_to_watch'`).

When on, entries are grouped into collapsible sections, one per season,
**ascending — nearest future season first**.

**Table view.** A full-width header row spanning all columns, containing a
button with: chevron, season name, count. The soonest group additionally gets
an accent left rail and a "Next season" chip.

**Grid view.** The same header as a full-width band. The poster grid is one
grid container; each section is `display: contents` so every card remains a
direct grid item and the band spans `grid-column: 1 / -1`. This means
ungrouping requires no re-flow — the bands hide and the wall closes up.

### Decisions

| Decision | Choice |
|---|---|
| Order | Ascending, nearest season first — matches the existing default sort. |
| Next-season mark | Accent rail + "Next season" chip on the soonest group, computed **against today's date**, not list position. |
| Season column | Hidden while grouped (the header carries it); restored with its sort arrow when grouping is off. |
| TBA | Undated shows collect in a final `TBA` group, **expanded**, keeping the position they already sort to. |
| Scope | Both table and grid, sharing one toggle and one collapse state. |
| Default | Grouping on. |

### State

- `anivault-library-group-by-season` — `'true'`/`'false'`, default `'true'`,
  following the existing `loadPref`/`persistPref` convention.
- Collapse state: one key holding a map of season key → collapsed. Seasons not
  present in the map default to **open**, so a newly announced season never
  arrives hidden.
- Season key is derived from `season` + `season_year` (e.g. `fall2026`,
  `tba` for null). Reuse `SEASON_ORDER` and `seasonSortVal` for ordering and
  `formatSeason` for the display name — do not duplicate that logic.

### Search interaction

Typing a query already spans every status and ignores the tab
(`searchFilter = query.trim() ? null : statusFilter`). Grouping therefore
switches off for the duration of a search, rather than grouping results that
are not all Plan to Watch.

### Known limitation (accepted)

A show in Plan to Watch whose season has **already started** groups under that
past season, which sorts above the next one and pushes it down. Assumed rare
enough to ignore. If it turns out to matter, the fix is to fold everything
older than the current season into a single group pinned to the bottom.

---

## 3. Fix the 200-row cap (bug)

### The bug

`LibraryView.svelte:200`:

```ts
const results = await searchLibrary(query, searchFilter, 200, 0);
```

A hard-coded page size of 200 and an offset that is never advanced. There is no
second call, no "load more", and no infinite scroll. Tab counts come from
`getLibraryStats()` — a SQL `COUNT` over the whole table — so the count is
correct and the list is what is wrong.

Observed: All reads 627 and shows 200; Completed reads 463 and shows 200. Every
category under 200 is unaffected, which is why it reads as an oddity rather
than a cap.

### Why it is worse than "fewer rows"

`search_library` applies `ORDER BY a.id` **before** `LIMIT`, and the UI then
sorts the returned array client-side (`sortedEntries`). So the visible list is
the **200 lowest AniList ids, re-sorted to look complete**:

- Sorting by title Z→A shows the end of a *subset*, not of the library.
- Which shows are missing correlates with AniList id, i.e. roughly with how
  recent they are.
- Nothing in the UI indicates truncation.

### The fix

Fetch the whole filtered set — pass a limit far above any plausible library
size rather than introducing pagination. Concretely: a named constant
`LIBRARY_FETCH_LIMIT = 10000`, so the number is greppable and its intent is
documented at the definition rather than inferred from a magic argument.

Rationale: this is local SQLite over Tauri IPC, not a network API. 627 narrow
rows is negligible to query and transfer, and the Seasons page already renders
several hundred cards in one flat grid. A "load more" control would add UI and
state to solve a problem the data size does not have, and would leave the
client-side sort just as wrong on a partial set — the sort is only correct once
the client holds every row for the active tab.

**Correct sorting falls out of this fix**; no sorting code changes.

### Explicitly out of scope

`loadEpisodeFiles` walks `entries.slice(0, 50)` with a sequential `await` per
anime — 50 IPC round trips, and only the first 50 rows get download bars. That
cap is independent of this bug but becomes visible with 627 rows on screen. The
proper fix is one batched command taking a list of ids. **Not in this work.**

---

## 4. Next-episode column on Watching

### Why

The detail page already surfaces "Next episode · Ep 7 · Aug 19 · in 6d 14h",
but one show at a time. As a column it answers the question Watching is opened
to answer — which of these is on next — and it sorts.

### Data source

**Use `get_calendar`, not `get_next_airing`.**

`get_next_airing` (`commands.rs:752`) fires **one live GraphQL query per
anime**. Acceptable for a single opened show; wrong for a list, where it means
one request per row on every Library load against an AniList budget of roughly
30/min — the budget season pagination already strains.

`get_calendar` costs nothing extra:

- Already cached behind `CALENDAR_CACHE_TTL_SECS`; returns from cache with no
  network at all.
- Its universe is already exactly the watching + plan-to-watch shows
  (`calendar_anime_ids()`).
- Fetches in one batched request (`airingSchedules(mediaId_in: […])`).
- Calendar and Dashboard already call it, so on a warm cache the Library adds
  zero requests.

It returns one `CalendarEntry` per airing episode across a ±window
(now−31d → now+60d). The column takes, per `anime_id`, the **earliest entry
whose `airing_at` is still in the future**. A show with no such entry —
finished, or airing beyond the window — renders `—`.

### Presentation

Two lines in one cell:

- **Primary:** `in 6d 14h` — the countdown, in body text colour, tabular nums.
  Accent + semibold when under 24 hours.
- **Secondary:** `Ep 7 · Aug 19` — muted, small, matching the Season cell tone.
- **No upcoming episode:** a muted `—`.

Grid view gets the same as a line under the progress bar on Watching cards.

### Decisions

| Decision | Choice |
|---|---|
| Gating | Watching only, mirroring how the Season column is gated to Plan to Watch. Plan to Watch shows have not aired, so a countdown there duplicates the season header. |
| Sorting | New `next_airing` sort key. Rows with no airing sort **last regardless of direction**, the same treatment `seasonSortVal` gives undated shows. |
| Default sort | Change Watching's default from `progress`/`asc` to `next_airing`/`asc`. Per-category sort memory means an existing preference is respected. |
| Ticking | A 60s interval re-renders the countdown, matching its day/hour granularity. Cleared in `onDestroy` with the existing timers. |
| Scope | Table and grid, consistent with change 2. |

**Do not merge the two existing countdown formatters.**
`DetailView.formatCountdown` returns `airing now` at <= 0 and stops at minute
granularity; `CalendarView.countdown` returns `Aired` and adds a seconds tier.
They are not the same function, and unifying them would change what Calendar
and the detail page display. Add a third formatter in the new `libraryUi.ts`
(below) scoped to this column, and leave both existing call sites untouched.

### Trade-off (accepted)

Because the calendar is cached, a countdown can be up to one TTL stale. It is
computed from a cached `airing_at` against the live clock, so it stays
*arithmetically* correct as it ticks down; but a schedule change AniList
published within the TTL may not have landed. Correct trade against a live
request per row, and the detail page still shows the fresh value.

---

## Testing

**There is no `LibraryView.test.ts`, and this work does not add one.** The
established pattern in this codebase is pure logic extracted into a `*Ui.ts`
module with a `*Ui.test.ts` beside it (`seasonUi`, `calendarUi`, `collectionUi`,
`homeUi`, `fileMappingUi`). Component tests exist only where the behaviour is
genuinely interactive (`SeasonView`, `CalendarView`, `DashboardView`).

So: **create `next/src/lib/libraryUi.ts`** holding every pure decision this work
introduces, and unit-test it in `next/src/lib/libraryUi.test.ts`. The component
keeps only rendering and wiring.

`libraryUi.ts` exports:

| Export | Purpose |
|---|---|
| `normalizeStatusFilter(v, known)` | Maps an unrecognised persisted filter to `null`. Change 1's migration hazard. |
| `SEASON_ORDER`, `seasonSortVal(e)` | Moved verbatim out of the component. |
| `seasonGroupKey(e)` | `fall2026` / `tba`. |
| `seasonGroupLabel(e)` | `Fall 2026` / `TBA`. |
| `groupBySeason(entries, current)` | Ordered `SeasonGroup[]` with the next-season marker resolved. |
| `nextAiringByAnime(calendar, nowSec)` | `Map<anime_id, CalendarEntry>` of the earliest future airing per show. |
| `formatAiringCountdown(secs)` | `6d 14h` / `14h 3m` / `3m`. |
| `nextAiringSortVal(e, map)` | Airing time, `Infinity` when there is none. |

`getCurrentSeason()` currently lives as a private function inside
`SeasonView.svelte`. Move it to `seasonUi.ts` as an export and have both
SeasonView and `libraryUi` use it, rather than duplicating the month-to-season
maths a second time.

Cases to cover in `libraryUi.test.ts`:

1. `normalizeStatusFilter('unlisted', …)` → `null`; a known value passes through.
2. `seasonGroupKey` for a dated show, and `tba` when season or year is null.
3. `seasonGroupLabel` renders `Fall 2026`, and `TBA` for undated.
4. `groupBySeason` orders groups ascending by season.
5. `groupBySeason` puts the TBA group last regardless of input order.
6. The first group at or after the current season is marked; earlier and later
   ones are not; the chip reads `This season` on an exact match and
   `Next season` when it is genuinely ahead.
7. `groupBySeason` on an empty list returns an empty array.
8. `nextAiringByAnime` picks the earliest *future* entry per anime and ignores
   past ones.
9. `nextAiringByAnime` omits a show whose only entries are in the past.
10. `formatAiringCountdown` renders days+hours, hours+minutes, and minutes.
11. `nextAiringSortVal` returns `Infinity` for a show with no airing, so it
    sorts last in both directions.

Existing suites that must stay green: `api.test.ts` (asserts `searchLibrary`'s
argument shape) and `App.test.ts` (mocks `searchLibrary`).

## Verification

```
cd next && npm run verify      # tsc, svelte-check, vitest, cargo check --tests
```

## Release

Per CLAUDE.md, this ships as a patch bump (1.0.20) — but **only when the user
says to build**. Order once they do: build → git push → GitHub release.
