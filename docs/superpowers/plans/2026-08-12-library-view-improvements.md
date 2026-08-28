# Library View Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the bogus "Unlisted" category, group Plan to Watch by season in both views, fix a row cap that hides two thirds of the library, and add a next-episode countdown column to Watching.

**Architecture:** All four changes live in the Svelte frontend. Every pure decision (filter normalisation, season grouping, airing lookup, countdown formatting) is extracted into a new `next/src/lib/libraryUi.ts` and unit-tested there; `LibraryView.svelte` keeps only rendering and wiring. The Rust backend is not modified.

**Tech Stack:** Svelte 4 (Vite), TypeScript, Vitest, Tauri 2 (Rust backend, untouched).

Spec: `docs/superpowers/specs/2026-08-12-library-view-improvements-design.md`
Mockup: https://claude.ai/code/artifact/f16b4199-9299-443b-9f1d-9e4a670a2f49

## Global Constraints

- **Windows-only Tauri desktop app.** Active codebase is `next/`. Run commands from `next/` unless stated otherwise.
- **`pwsh` is NOT installed.** npm scripts run via plain `powershell`. Use `npm.cmd` / `npx.cmd` from a Bash shell.
- **No Rust changes.** `next/src-tauri/` is not touched by any task. `storage.rs`'s `COALESCE(le.status, 'unlisted')` stays exactly as it is — the marker is honest internally and `library_storage_test.rs` covers it.
- **Verification command:** `npm run verify` in `next/` — runs `tsc`, `svelte-check`, Vitest, and `cargo check --tests`. Must be clean before any commit that ends a task.
- **Do not build an installer, bump the version, push, or create a release.** CLAUDE.md requires the user to ask first. This plan stops at committed, verified code on `develop`.
- **Existing suites that must stay green:** `src/lib/api.test.ts` (asserts `searchLibrary`'s argument shape), `src/App.test.ts` (mocks `searchLibrary`), and `src/lib/SeasonView.test.ts` (Task 3 moves a helper out of that component).
- **Svelte 4 orders reactive statements by dependency.** A `$:` that reads a value declared in a later `$:` fails svelte-check with a use-before-define. Where a task says to *move* a reactive line, the move is load-bearing.
- **Existing pref convention:** `loadPref(key, fallback)` / `persistPref(key, value)` in `LibraryView.svelte`, both `try/catch`-wrapped against `localStorage` throwing.

---

### Task 1: Fix the 200-row cap

Smallest and fully independent — land it first so the rest of the work is done against a Library that shows every row.

**Files:**
- Modify: `next/src/lib/LibraryView.svelte:200`

**Interfaces:**
- Consumes: nothing
- Produces: `LIBRARY_FETCH_LIMIT` (module-local constant; no other task imports it)

- [ ] **Step 1: Read the current call site**

Run: `sed -n '193,210p' next/src/lib/LibraryView.svelte`

Confirm it reads `const results = await searchLibrary(query, searchFilter, 200, 0);`

- [ ] **Step 2: Add the constant**

In `LibraryView.svelte`, immediately above `async function load() {`, add:

```ts
  // The Library is local SQLite over IPC, so fetching every row for the active
  // tab costs nothing meaningful — and it is the only way the client-side sort
  // in `sortedEntries` can be correct. A page size instead truncates by
  // `ORDER BY a.id` and then sorts the survivors, which silently shows the
  // lowest-id subset re-sorted to look complete.
  const LIBRARY_FETCH_LIMIT = 10000;
```

- [ ] **Step 3: Use it**

Replace line 200:

```ts
      const results = await searchLibrary(query, searchFilter, LIBRARY_FETCH_LIMIT, 0);
```

- [ ] **Step 4: Verify**

Run: `cd next && npm.cmd run check && npm.cmd run check:svelte`
Expected: tsc clean, svelte-check 0 errors 0 warnings.

Run: `cd next && npx.cmd vitest run src/lib/api.test.ts src/App.test.ts`
Expected: PASS — neither asserts the literal `200`.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/LibraryView.svelte
git commit -m "fix: stop truncating the Library list at 200 rows

The list was fetched with a hard-coded LIMIT 200 and an offset that was
never advanced, while the tab counts came from a COUNT over the whole
table -- so All read 627 and showed 200.

Worse than missing rows: search_library applies ORDER BY a.id before the
limit and the UI then sorts client-side, so the view was the 200 lowest
AniList ids re-sorted to look complete."
```

---

### Task 2: Create `libraryUi.ts` and remove the "Unlisted" category

**Files:**
- Create: `next/src/lib/libraryUi.ts`
- Create: `next/src/lib/libraryUi.test.ts`
- Modify: `next/src/lib/LibraryView.svelte` (lines ~11–14, 120, 172, 179–187, 631, 821)

**Interfaces:**
- Consumes: nothing
- Produces: `normalizeStatusFilter(value: string | null, known: (string | null)[]): string | null`. Tasks 3 and 6 add further exports to the same file.

- [ ] **Step 1: Write the failing test**

Create `next/src/lib/libraryUi.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { normalizeStatusFilter } from './libraryUi';

const KNOWN = [null, 'watching', 'completed', 'on_hold', 'dropped', 'plan_to_watch'];

describe('normalizeStatusFilter', () => {
  it('passes a known status through', () => {
    expect(normalizeStatusFilter('watching', KNOWN)).toBe('watching');
  });

  it('maps a removed status to All', () => {
    expect(normalizeStatusFilter('unlisted', KNOWN)).toBeNull();
  });

  it('maps an empty string to All', () => {
    expect(normalizeStatusFilter('', KNOWN)).toBeNull();
  });

  it('maps null to All', () => {
    expect(normalizeStatusFilter(null, KNOWN)).toBeNull();
  });

  it('maps any unrecognised value to All', () => {
    expect(normalizeStatusFilter('nonsense', KNOWN)).toBeNull();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd next && npx.cmd vitest run src/lib/libraryUi.test.ts`
Expected: FAIL — cannot resolve `./libraryUi`.

- [ ] **Step 3: Write minimal implementation**

Create `next/src/lib/libraryUi.ts`:

```ts
// Pure helpers for LibraryView, kept out of the component for testability.

/// A persisted status filter is only trusted if it still corresponds to a tab.
/// Anything else -- an empty string, or a category that has since been removed
/// -- falls back to All, so a stale localStorage value cannot leave the view
/// with no tab selected and an empty-looking list.
export function normalizeStatusFilter(
  value: string | null,
  known: (string | null)[],
): string | null {
  if (!value) return null;
  return known.includes(value) ? value : null;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd next && npx.cmd vitest run src/lib/libraryUi.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Remove the Unlisted tab and its count**

In `LibraryView.svelte`, delete the `unlisted` entry from `statusOptions` so it reads:

```ts
  const statusOptions = [
    { value: null, label: 'All' },
    { value: 'watching', label: 'Watching' },
    { value: 'completed', label: 'Completed' },
    { value: 'on_hold', label: 'On Hold' },
    { value: 'dropped', label: 'Dropped' },
    { value: 'plan_to_watch', label: 'Plan to Watch' },
  ];
```

In `tabCount`, delete this line entirely:

```ts
      case 'unlisted': return Math.max(0, stats.total - (stats.watching + stats.completed + stats.on_hold + stats.dropped + stats.plan_to_watch));
```

In `matchesActiveFilter`, delete this line entirely:

```ts
    if (statusFilter === 'unlisted') return !e.status || e.status === 'unlisted';
```

- [ ] **Step 6: Harden the persisted filter**

`statusOptions` is declared around line 179, but `loadPersistedFilter` runs at line ~30, so it cannot reference `statusOptions`. Declare the known values above `loadPersistedFilter` instead.

Add to the import block at the top of the `<script>`:

```ts
  import { normalizeStatusFilter } from './libraryUi';
```

Then replace `loadPersistedFilter` with:

```ts
  // Every value a tab can select. Declared here rather than derived from
  // `statusOptions` because that constant is defined further down the file,
  // after this function runs.
  const KNOWN_STATUS_FILTERS: (string | null)[] = [
    null, 'watching', 'completed', 'on_hold', 'dropped', 'plan_to_watch',
  ];

  function loadPersistedFilter(): string | null {
    try {
      return normalizeStatusFilter(
        localStorage.getItem('anivault-library-filter'),
        KNOWN_STATUS_FILTERS,
      );
    } catch { return null; }
  }
```

- [ ] **Step 7: Quiet the badge**

In the grid card (was line 631), replace:

```svelte
            <span class="badge">{formatStatus(entry.status)}</span>
```

with:

```svelte
            {#if entry.status === 'unlisted'}
              <span class="no-status" aria-label="No list status">—</span>
            {:else}
              <span class="badge">{formatStatus(entry.status)}</span>
            {/if}
```

In the table row (was line 821), replace:

```svelte
                  <span class="badge">{formatStatus(entry.status)}</span>
```

with:

```svelte
                  {#if entry.status === 'unlisted'}
                    <span class="no-status" aria-label="No list status">—</span>
                  {:else}
                    <span class="badge">{formatStatus(entry.status)}</span>
                  {/if}
```

- [ ] **Step 8: Style the placeholder**

In the `<style>` block, immediately after the `.badge { … }` rule, add:

```css
  .no-status {
    color: var(--color-muted);
    opacity: 0.5;
  }
```

- [ ] **Step 9: Verify**

Run: `cd next && npm.cmd run verify`
Expected: tsc clean, svelte-check 0/0, all Vitest files pass, `cargo check --tests` clean.

- [ ] **Step 10: Commit**

```bash
git add next/src/lib/libraryUi.ts next/src/lib/libraryUi.test.ts next/src/lib/LibraryView.svelte
git commit -m "feat: drop the Unlisted category from the Library view

'unlisted' is not a status -- search_library synthesises it via
COALESCE for anime with local files but no list entry. It was also a
drag-drop target, so dropping a row on it wrote the literal string
'unlisted' into list_entry.status, a value nothing else produces.

Rows are kept and stay playable; the status cell now shows a muted
dash. A persisted filter of 'unlisted' now falls back to All instead of
loading an empty-looking list."
```

---

### Task 3: Season grouping helpers

Pure logic only. The component is wired up in Tasks 4 and 5.

**Files:**
- Modify: `next/src/lib/seasonUi.ts` (export `getCurrentSeason`)
- Modify: `next/src/lib/SeasonView.svelte` (use the moved helper)
- Modify: `next/src/lib/libraryUi.ts`
- Modify: `next/src/lib/libraryUi.test.ts`

**Interfaces:**
- Consumes: `normalizeStatusFilter` (Task 2), already in `libraryUi.ts`
- Produces:
  - `getCurrentSeason(): { season: string; year: number }` from `seasonUi.ts`
  - `SeasonGrouped`: `{ season: string | null; season_year: number | null }`
  - `SEASON_ORDER: Record<string, number>`
  - `seasonSortVal(e: SeasonGrouped): number`
  - `seasonGroupKey(e: SeasonGrouped): string`
  - `seasonGroupLabel(e: SeasonGrouped): string`
  - `SeasonGroup<T>`: `{ key: string; label: string; chip: string | null; entries: T[] }`
  - `groupBySeason<T extends SeasonGrouped>(entries: T[], current: { season: string; year: number }): SeasonGroup<T>[]`
  - `DisplayRow<T>`: `{ kind: 'group'; group: SeasonGroup<T> } | { kind: 'entry'; entry: T }`
  - `flattenGroups<T>(groups: SeasonGroup<T>[], collapsed: Record<string, boolean>): DisplayRow<T>[]`
  - `asDisplayRows<T>(entries: T[]): DisplayRow<T>[]`

**Why `DisplayRow` exists:** it lets the template render grouped and ungrouped modes through a *single* `{#each}`. Without it, both branches would need their own copy of the ~80-line row markup, which drifts the moment either is edited. Tasks 4, 5 and 7 all depend on there being exactly one copy.

- [ ] **Step 1: Write the failing test**

Append to `next/src/lib/libraryUi.test.ts`:

```ts
import {
  asDisplayRows, flattenGroups, groupBySeason,
  seasonGroupKey, seasonGroupLabel, seasonSortVal,
} from './libraryUi';

const CURRENT = { season: 'SUMMER', year: 2026 };
const show = (title: string, season: string | null, season_year: number | null) =>
  ({ title, season, season_year });

describe('seasonGroupKey', () => {
  it('combines season and year', () => {
    expect(seasonGroupKey(show('a', 'FALL', 2026))).toBe('fall2026');
  });

  it('is tba when the year is missing', () => {
    expect(seasonGroupKey(show('a', 'FALL', null))).toBe('tba');
  });

  it('is tba when the season is missing', () => {
    expect(seasonGroupKey(show('a', null, 2026))).toBe('tba');
  });
});

describe('seasonGroupLabel', () => {
  it('renders a readable season', () => {
    expect(seasonGroupLabel(show('a', 'FALL', 2026))).toBe('Fall 2026');
  });

  it('renders TBA when undated', () => {
    expect(seasonGroupLabel(show('a', null, null))).toBe('TBA');
  });
});

describe('groupBySeason', () => {
  it('returns nothing for an empty list', () => {
    expect(groupBySeason([], CURRENT)).toEqual([]);
  });

  it('orders groups ascending by season', () => {
    const groups = groupBySeason([
      show('c', 'SPRING', 2027),
      show('a', 'FALL', 2026),
      show('b', 'WINTER', 2027),
    ], CURRENT);
    expect(groups.map((g) => g.label)).toEqual(['Fall 2026', 'Winter 2027', 'Spring 2027']);
  });

  it('collects every show for a season into one group', () => {
    const groups = groupBySeason([
      show('a', 'FALL', 2026),
      show('b', 'FALL', 2026),
    ], CURRENT);
    expect(groups).toHaveLength(1);
    expect(groups[0].entries.map((e) => e.title)).toEqual(['a', 'b']);
  });

  it('puts TBA last regardless of input order', () => {
    const groups = groupBySeason([
      show('a', null, null),
      show('b', 'FALL', 2026),
    ], CURRENT);
    expect(groups.map((g) => g.key)).toEqual(['fall2026', 'tba']);
  });

  it('marks the soonest future season as next, and only that one', () => {
    const groups = groupBySeason([
      show('a', 'FALL', 2026),
      show('b', 'WINTER', 2027),
      show('c', null, null),
    ], CURRENT);
    expect(groups.map((g) => g.chip)).toEqual(['Next season', null, null]);
  });

  it('says This season when the soonest group is the current one', () => {
    const groups = groupBySeason([
      show('a', 'SUMMER', 2026),
      show('b', 'FALL', 2026),
    ], CURRENT);
    expect(groups.map((g) => g.chip)).toEqual(['This season', null]);
  });

  it('never marks a past season', () => {
    const groups = groupBySeason([
      show('a', 'WINTER', 2026),
      show('b', 'FALL', 2026),
    ], CURRENT);
    expect(groups.map((g) => g.chip)).toEqual([null, 'Next season']);
  });

  it('marks nothing when every group is in the past', () => {
    expect(groupBySeason([show('a', 'WINTER', 2026)], CURRENT).map((g) => g.chip)).toEqual([null]);
  });

  it('marks nothing when the only group is TBA', () => {
    expect(groupBySeason([show('a', null, null)], CURRENT).map((g) => g.chip)).toEqual([null]);
  });
});

describe('seasonSortVal', () => {
  it('orders within a year by season', () => {
    expect(seasonSortVal(show('a', 'WINTER', 2026)))
      .toBeLessThan(seasonSortVal(show('b', 'FALL', 2026)));
  });

  it('sorts undated shows last', () => {
    expect(seasonSortVal(show('a', null, null))).toBe(Number.POSITIVE_INFINITY);
  });
});

describe('flattenGroups', () => {
  const groups = groupBySeason([
    show('a', 'FALL', 2026),
    show('b', 'FALL', 2026),
    show('c', 'WINTER', 2027),
  ], CURRENT);

  it('emits a header before each group and its entries after', () => {
    const rows = flattenGroups(groups, {});
    expect(rows.map((r) => r.kind)).toEqual(['group', 'entry', 'entry', 'group', 'entry']);
  });

  it('omits the entries of a collapsed group but keeps its header', () => {
    const rows = flattenGroups(groups, { fall2026: true });
    expect(rows.map((r) => r.kind)).toEqual(['group', 'group', 'entry']);
  });

  it('treats a season absent from the map as open', () => {
    expect(flattenGroups(groups, { winter2027: false })).toHaveLength(5);
  });

  it('returns nothing for no groups', () => {
    expect(flattenGroups([], {})).toEqual([]);
  });
});

describe('asDisplayRows', () => {
  it('wraps a flat list as entry rows', () => {
    expect(asDisplayRows([show('a', 'FALL', 2026)]))
      .toEqual([{ kind: 'entry', entry: show('a', 'FALL', 2026) }]);
  });

  it('returns nothing for an empty list', () => {
    expect(asDisplayRows([])).toEqual([]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd next && npx.cmd vitest run src/lib/libraryUi.test.ts`
Expected: FAIL — `groupBySeason` and friends are not exported.

- [ ] **Step 3: Export `getCurrentSeason` from `seasonUi.ts`**

Append to `next/src/lib/seasonUi.ts`:

```ts
/// The anime season containing today's date. Months 0-2 Winter, 3-5 Spring,
/// 6-8 Summer, 9-11 Fall.
export function getCurrentSeason(): { season: string; year: number } {
  const now = new Date();
  const m = now.getMonth();
  const s = m < 3 ? 'WINTER' : m < 6 ? 'SPRING' : m < 9 ? 'SUMMER' : 'FALL';
  return { season: s, year: now.getFullYear() };
}
```

In `next/src/lib/SeasonView.svelte`, delete the private copy (lines 16–21):

```ts
  function getCurrentSeason(): { season: string; year: number } {
    const now = new Date();
    const m = now.getMonth();
    const s = m < 3 ? 'WINTER' : m < 6 ? 'SPRING' : m < 9 ? 'SUMMER' : 'FALL';
    return { season: s, year: now.getFullYear() };
  }
```

and add `getCurrentSeason` to the existing `./seasonUi` import in that file.

- [ ] **Step 4: Write the implementation**

Append to `next/src/lib/libraryUi.ts`:

```ts
import { getCurrentSeason } from './seasonUi';

export { getCurrentSeason };

/// The season fields every grouped row carries. Structural, so both
/// LibraryEntry and test fixtures satisfy it.
export interface SeasonGrouped {
  season: string | null;
  season_year: number | null;
}

export const SEASON_ORDER: Record<string, number> = {
  WINTER: 0, SPRING: 1, SUMMER: 2, FALL: 3,
};

const SEASON_LABELS: Record<string, string> = {
  WINTER: 'Winter', SPRING: 'Spring', SUMMER: 'Summer', FALL: 'Fall',
};

/// Sortable position of a season. Undated shows sort last.
export function seasonSortVal(e: SeasonGrouped): number {
  if (!e.season_year) return Number.POSITIVE_INFINITY;
  return e.season_year * 10 + (SEASON_ORDER[e.season ?? ''] ?? 0);
}

/// Stable identity for a season group, used as the localStorage collapse key.
export function seasonGroupKey(e: SeasonGrouped): string {
  if (!e.season || e.season_year == null) return 'tba';
  return `${e.season.toLowerCase()}${e.season_year}`;
}

/// Display name for a season group.
export function seasonGroupLabel(e: SeasonGrouped): string {
  if (!e.season || e.season_year == null) return 'TBA';
  return `${SEASON_LABELS[e.season] ?? e.season} ${e.season_year}`;
}

export interface SeasonGroup<T> {
  key: string;
  label: string;
  /// 'This season' / 'Next season' on the soonest group that has not already
  /// passed; null on every other group.
  chip: string | null;
  entries: T[];
}

/// Absolute season index, so seasons compare across year boundaries.
function absIndex(season: string, year: number): number {
  return year * 4 + (SEASON_ORDER[season] ?? 0);
}

/// Group rows into seasons, nearest first, TBA last.
///
/// The marker is computed against `current` (today's season) rather than
/// against list position, so it stays correct even when the list happens to
/// contain a season that has already started.
export function groupBySeason<T extends SeasonGrouped>(
  entries: T[],
  current: { season: string; year: number },
): SeasonGroup<T>[] {
  const byKey = new Map<string, SeasonGroup<T>>();
  for (const e of entries) {
    const key = seasonGroupKey(e);
    let g = byKey.get(key);
    if (!g) {
      g = { key, label: seasonGroupLabel(e), chip: null, entries: [] };
      byKey.set(key, g);
    }
    g.entries.push(e);
  }

  const groups = [...byKey.values()];
  // TBA has no position on the calendar, so it is pinned last rather than
  // sorted; every dated group orders ascending.
  groups.sort((a, b) => {
    if (a.key === 'tba') return b.key === 'tba' ? 0 : 1;
    if (b.key === 'tba') return -1;
    return seasonSortVal(a.entries[0]) - seasonSortVal(b.entries[0]);
  });

  const currentAbs = absIndex(current.season, current.year);
  const soonest = groups.find((g) => {
    if (g.key === 'tba') return false;
    const e = g.entries[0];
    return absIndex(e.season as string, e.season_year as number) >= currentAbs;
  });
  if (soonest) {
    const e = soonest.entries[0];
    const isCurrent = absIndex(e.season as string, e.season_year as number) === currentAbs;
    soonest.chip = isCurrent ? 'This season' : 'Next season';
  }

  return groups;
}

/// One rendered row: either a season header or an anime.
///
/// Grouped and ungrouped modes both reduce to a list of these, so the template
/// needs a single `{#each}` and the row markup exists in exactly one place.
export type DisplayRow<T> =
  | { kind: 'group'; group: SeasonGroup<T> }
  | { kind: 'entry'; entry: T };

/// Interleave group headers with their entries, skipping collapsed bodies.
/// A season absent from `collapsed` is open.
export function flattenGroups<T>(
  groups: SeasonGroup<T>[],
  collapsed: Record<string, boolean>,
): DisplayRow<T>[] {
  const out: DisplayRow<T>[] = [];
  for (const group of groups) {
    out.push({ kind: 'group', group });
    if (!collapsed[group.key]) {
      for (const entry of group.entries) out.push({ kind: 'entry', entry });
    }
  }
  return out;
}

/// The ungrouped equivalent: every row is an anime.
export function asDisplayRows<T>(entries: T[]): DisplayRow<T>[] {
  return entries.map((entry) => ({ kind: 'entry', entry }));
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd next && npx.cmd vitest run src/lib/libraryUi.test.ts src/lib/seasonUi.test.ts src/lib/SeasonView.test.ts`
Expected: PASS — the new cases, plus the existing seasonUi and SeasonView suites still green after moving `getCurrentSeason`.

- [ ] **Step 6: Verify the whole suite**

Run: `cd next && npm.cmd run verify`
Expected: all clean.

- [ ] **Step 7: Commit**

```bash
git add next/src/lib/libraryUi.ts next/src/lib/libraryUi.test.ts next/src/lib/seasonUi.ts next/src/lib/SeasonView.svelte
git commit -m "feat: season-grouping helpers for the Library view

Pure logic only -- the component is wired up next. Grouped and
ungrouped modes both reduce to a DisplayRow list so the template can
keep one copy of the row markup. getCurrentSeason moves out of
SeasonView into seasonUi so the month-to-season maths is not
duplicated."
```

---

### Task 4: Grouping in the table view

**Files:**
- Modify: `next/src/lib/LibraryView.svelte`

**Interfaces:**
- Consumes: `groupBySeason`, `flattenGroups`, `asDisplayRows`, `seasonSortVal`, `getCurrentSeason` (Task 3)
- Produces: `groupingActive`, `displayRows`, `collapsedSeasons`, `toggleGroup(key: string)`, `columnCount` — reused by Tasks 5 and 7

- [ ] **Step 1: Move `seasonSortVal` and `SEASON_ORDER` to the import**

In `LibraryView.svelte`, delete both local declarations:

```ts
  const SEASON_ORDER: Record<string, number> = { WINTER: 0, SPRING: 1, SUMMER: 2, FALL: 3 };
```

```ts
  function seasonSortVal(e: LibraryEntry): number {
    if (!e.season_year) return Number.POSITIVE_INFINITY;
    return e.season_year * 10 + (SEASON_ORDER[e.season ?? ''] ?? 0);
  }
```

Extend the `./libraryUi` import (added in Task 2) to:

```ts
  import {
    normalizeStatusFilter, groupBySeason, flattenGroups, asDisplayRows,
    seasonSortVal, getCurrentSeason,
  } from './libraryUi';
```

Leave `formatSeason` in the component — it is still used by the flat-mode Season cell.

- [ ] **Step 2: Add the toggle and collapse state**

Below the existing `compact` declaration, add:

```ts
  const GROUP_PREF_KEY = 'anivault-library-group-by-season';
  const COLLAPSE_KEY = 'anivault-library-season-collapsed';

  let groupBySeasonPref = loadPref(GROUP_PREF_KEY, 'true') === 'true';
  $: persistPref(GROUP_PREF_KEY, groupBySeasonPref ? 'true' : 'false');

  // Season key -> collapsed. A season absent from the map is open, so a newly
  // announced season never arrives hidden.
  function loadCollapsed(): Record<string, boolean> {
    try {
      const raw = localStorage.getItem(COLLAPSE_KEY);
      return raw ? (JSON.parse(raw) as Record<string, boolean>) : {};
    } catch { return {}; }
  }
  let collapsedSeasons: Record<string, boolean> = loadCollapsed();

  function toggleGroup(key: string) {
    collapsedSeasons = { ...collapsedSeasons, [key]: !collapsedSeasons[key] };
    try { localStorage.setItem(COLLAPSE_KEY, JSON.stringify(collapsedSeasons)); } catch { /* ignore */ }
  }
```

- [ ] **Step 3: Derive the display rows**

Immediately *after* the existing `$: sortedEntries = (…)()` block, add:

```ts
  // Grouping is a Plan to Watch affordance only, and a search spans every
  // category, so it switches off for the duration of a query.
  $: groupingActive = groupBySeasonPref && statusFilter === 'plan_to_watch' && !query.trim();
  $: displayRows = groupingActive
    ? flattenGroups(groupBySeason(sortedEntries, getCurrentSeason()), collapsedSeasons)
    : asDisplayRows(sortedEntries);
```

Then **move** the existing `$: showSeason = …` line (currently around line 73) down to sit directly beneath those, and change it so the column hides while grouped:

```ts
  // The group header carries the season, so the column is redundant there.
  $: showSeason = statusFilter === 'plan_to_watch' && !groupingActive;
  // check + thumb + title + status + progress + files, plus optional columns.
  $: columnCount = 6 + (showSeason ? 1 : 0);
```

The move is load-bearing: leaving `showSeason` above `groupingActive` fails svelte-check with a use-before-define.

- [ ] **Step 4: Render the toggle**

In the controls row, directly after the existing Compact button's `{/if}`, add:

```svelte
    {#if statusFilter === 'plan_to_watch'}
      <button
        class="view-toggle"
        on:click={() => groupBySeasonPref = !groupBySeasonPref}
        aria-pressed={groupBySeasonPref}
        title="Group Plan to Watch by season"
      >
        ⊞ Group by season
      </button>
    {/if}
```

- [ ] **Step 5: Switch the table body to one loop over `displayRows`**

The `<tbody>`'s final branch currently reads:

```svelte
          {:else}
            {#each sortedEntries as entry (entry.anime_id)}
              <tr class="data-row" … >
                …
              </tr>
            {/each}
          {/if}
```

Change **only** the loop wrapper — the `<tr class="data-row">` markup inside stays exactly as it is:

```svelte
          {:else}
            {#each displayRows as row (row.kind === 'group' ? `g:${row.group.key}` : `e:${row.entry.anime_id}`)}
              {#if row.kind === 'group'}
                <tr class="group-row" class:is-marked={row.group.chip !== null}>
                  <td colspan={columnCount}>
                    <button
                      type="button"
                      class="group-btn"
                      aria-expanded={!collapsedSeasons[row.group.key]}
                      on:click={() => toggleGroup(row.group.key)}
                    >
                      <span class="chev" class:collapsed={collapsedSeasons[row.group.key]} aria-hidden="true">
                        <ChevronDown size={13} />
                      </span>
                      <span class="group-name">{row.group.label}</span>
                      <span class="group-count">{row.group.entries.length}</span>
                      {#if row.group.chip}<span class="next-chip">{row.group.chip}</span>{/if}
                    </button>
                  </td>
                </tr>
              {:else}
                {@const entry = row.entry}
                <tr class="data-row" … >
                  …  <!-- unchanged -->
                </tr>
              {/if}
            {/each}
          {/if}
```

`{@const entry = row.entry}` is what lets the existing markup keep referring to `entry`. It must be the first thing inside the `{:else}` — Svelte 4 requires `{@const}` to be an immediate child of a block.

Also change the empty-row colspan from `colspan={showSeason ? 7 : 6}` to `colspan={columnCount}`.

- [ ] **Step 6: Style the group header**

Add to the `<style>` block, after the `.data-row` rules:

```css
  .group-row td {
    padding: 0;
    background: var(--color-surface-raised);
    border-bottom: 1px solid rgba(var(--color-accent-rgb), 0.14);
  }

  .group-btn {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    width: 100%;
    padding: 0.5rem 0.7rem;
    background: transparent;
    border: 0;
    border-left: 3px solid transparent;
    color: var(--color-text);
    font-family: var(--font-ui);
    font-size: 0.85rem;
    text-align: left;
    cursor: pointer;
  }

  .group-btn:hover { background: rgba(var(--color-accent-rgb), 0.07); }
  .group-btn:focus-visible { outline: 2px solid var(--color-accent); outline-offset: -2px; }
  .group-row.is-marked .group-btn { border-left-color: var(--color-accent); }

  .chev {
    display: inline-flex;
    color: var(--color-muted);
    transition: transform 0.16s ease;
  }
  .chev.collapsed { transform: rotate(-90deg); }

  .group-name { font-weight: 650; }
  .group-count { color: var(--color-muted); font-size: 0.78rem; font-variant-numeric: tabular-nums; }

  .next-chip {
    margin-left: auto;
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--color-accent);
    background: rgba(var(--color-accent-rgb), 0.14);
    border: 1px solid rgba(var(--color-accent-rgb), 0.3);
    border-radius: 999px;
    padding: 0.1rem 0.5rem;
  }

  @media (prefers-reduced-motion: reduce) {
    .chev { transition: none; }
  }
```

- [ ] **Step 7: Leave the selection helpers alone**

`toggleSelectAll` and `allSelected` iterate `sortedEntries`, not `displayRows`. That is correct — select-all should cover every entry, including those inside collapsed groups. Confirm both still reference `sortedEntries` and do not change them.

- [ ] **Step 8: Verify**

Run: `cd next && npm.cmd run verify`
Expected: all clean. svelte-check is what catches an unclosed `{#if}` or a misplaced `{@const}`.

- [ ] **Step 9: Commit**

```bash
git add next/src/lib/LibraryView.svelte
git commit -m "feat: group Plan to Watch by season in the table view

Plan to Watch is a schedule of unaired shows, not a backlog, so groups
run nearest-season-first and the soonest is marked against today's
date. Collapse state persists per season; unseen seasons default to
open. The Season column hides while grouped since the header carries
it.

Both modes render through one loop over DisplayRow, so the row markup
stays in a single place."
```

---

### Task 5: Grouping in the grid view

**Files:**
- Modify: `next/src/lib/LibraryView.svelte`

**Interfaces:**
- Consumes: `displayRows`, `collapsedSeasons`, `toggleGroup` (Task 4) — no new exports

- [ ] **Step 1: Switch the poster grid to the same loop**

The grid currently reads:

```svelte
  {#if viewMode === 'grid'}
    <div class="poster-grid">
      {#each sortedEntries as entry (entry.anime_id)}
        <div class="poster-card" … >
          …
        </div>
      {/each}
    </div>
```

Change **only** the loop wrapper — the `<div class="poster-card">` markup inside stays exactly as it is:

```svelte
  {#if viewMode === 'grid'}
    <div class="poster-grid">
      {#each displayRows as row (row.kind === 'group' ? `g:${row.group.key}` : `e:${row.entry.anime_id}`)}
        {#if row.kind === 'group'}
          <button
            type="button"
            class="group-band"
            class:is-marked={row.group.chip !== null}
            aria-expanded={!collapsedSeasons[row.group.key]}
            on:click={() => toggleGroup(row.group.key)}
          >
            <span class="chev" class:collapsed={collapsedSeasons[row.group.key]} aria-hidden="true">
              <ChevronDown size={13} />
            </span>
            <span class="group-name">{row.group.label}</span>
            <span class="group-count">{row.group.entries.length}</span>
            {#if row.group.chip}<span class="next-chip">{row.group.chip}</span>{/if}
          </button>
        {:else}
          {@const entry = row.entry}
          <div class="poster-card" … >
            …  <!-- unchanged -->
          </div>
        {/if}
      {/each}
    </div>
```

No wrapper element is needed around each section: the band is a direct child of `.poster-grid` and spans it with `grid-column: 1 / -1`, so the cards keep flowing as one continuous grid and ungrouping needs no re-flow.

- [ ] **Step 2: Style the band**

Add to the `<style>` block, after the `.poster-card` rules:

```css
  .group-band {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 0.55rem;
    width: 100%;
    padding: 0.45rem 0.7rem;
    margin-top: 0.35rem;
    background: var(--color-surface-raised);
    border: 1px solid rgba(var(--color-accent-rgb), 0.14);
    border-left: 3px solid transparent;
    border-radius: 8px;
    color: var(--color-text);
    font-family: var(--font-ui);
    font-size: 0.85rem;
    text-align: left;
    cursor: pointer;
  }

  .group-band:first-child { margin-top: 0; }
  .group-band:hover { background: rgba(var(--color-accent-rgb), 0.1); }
  .group-band:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }
  .group-band.is-marked { border-left-color: var(--color-accent); }
```

- [ ] **Step 3: Verify**

Run: `cd next && npm.cmd run verify`
Expected: all clean.

- [ ] **Step 4: Manual check**

Run the app and confirm, on Plan to Watch:
- Grid view shows full-width season bands with posters flowing beneath.
- Collapsing a band in grid view keeps it collapsed after switching to table view (shared state).
- Turning the toggle off closes the wall up with no gaps.

Note: `cargo tauri dev` rewrites the launch-on-startup Run key to the debug binary — restore it afterwards if you use it.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/LibraryView.svelte
git commit -m "feat: season grouping in the Library grid view

The band is a direct grid child spanning the full row, so cards keep
flowing as one continuous grid and ungrouping needs no re-flow. Shares
the toggle, collapse state and DisplayRow list with the table view."
```

---

### Task 6: Next-airing helpers

Pure logic only. The column is wired up in Task 7.

**Files:**
- Modify: `next/src/lib/libraryUi.ts`
- Modify: `next/src/lib/libraryUi.test.ts`

**Interfaces:**
- Consumes: nothing from earlier tasks
- Produces:
  - `AiringLike`: `{ anime_id: number; next_episode: number | null; airing_at: number | null }`
  - `nextAiringByAnime<T extends AiringLike>(entries: T[], nowSec: number): Map<number, T>`
  - `formatAiringCountdown(secs: number): string`
  - `nextAiringSortVal(animeId: number, map: Map<number, AiringLike>): number`

- [ ] **Step 1: Write the failing test**

Append to `next/src/lib/libraryUi.test.ts`:

```ts
import { formatAiringCountdown, nextAiringByAnime, nextAiringSortVal } from './libraryUi';

const NOW = 1_760_000_000;
const air = (anime_id: number, next_episode: number | null, airing_at: number | null) =>
  ({ anime_id, next_episode, airing_at });

describe('nextAiringByAnime', () => {
  it('picks the earliest future entry for a show', () => {
    const map = nextAiringByAnime([
      air(1, 9, NOW + 8 * 86400),
      air(1, 8, NOW + 86400),
      air(1, 10, NOW + 15 * 86400),
    ], NOW);
    expect(map.get(1)?.next_episode).toBe(8);
  });

  it('ignores entries that have already aired', () => {
    const map = nextAiringByAnime([
      air(1, 7, NOW - 86400),
      air(1, 8, NOW + 86400),
    ], NOW);
    expect(map.get(1)?.next_episode).toBe(8);
  });

  it('omits a show whose entries are all in the past', () => {
    expect(nextAiringByAnime([air(1, 7, NOW - 86400)], NOW).has(1)).toBe(false);
  });

  it('omits an entry with no airing time', () => {
    expect(nextAiringByAnime([air(1, 7, null)], NOW).has(1)).toBe(false);
  });

  it('keeps shows separate', () => {
    const map = nextAiringByAnime([
      air(1, 8, NOW + 86400),
      air(2, 3, NOW + 2 * 86400),
    ], NOW);
    expect(map.get(1)?.next_episode).toBe(8);
    expect(map.get(2)?.next_episode).toBe(3);
  });

  it('is empty for no entries', () => {
    expect(nextAiringByAnime([], NOW).size).toBe(0);
  });
});

describe('formatAiringCountdown', () => {
  it('renders days and hours', () => {
    expect(formatAiringCountdown(6 * 86400 + 14 * 3600)).toBe('6d 14h');
  });

  it('renders hours and minutes under a day', () => {
    expect(formatAiringCountdown(14 * 3600 + 3 * 60)).toBe('14h 3m');
  });

  it('renders minutes under an hour', () => {
    expect(formatAiringCountdown(3 * 60)).toBe('3m');
  });

  it('renders airing now at or past zero', () => {
    expect(formatAiringCountdown(0)).toBe('airing now');
    expect(formatAiringCountdown(-60)).toBe('airing now');
  });
});

describe('nextAiringSortVal', () => {
  it('returns the airing time when there is one', () => {
    const map = nextAiringByAnime([air(1, 8, NOW + 86400)], NOW);
    expect(nextAiringSortVal(1, map)).toBe(NOW + 86400);
  });

  it('returns Infinity for a show with no airing, so it sorts last', () => {
    expect(nextAiringSortVal(1, nextAiringByAnime([], NOW))).toBe(Number.POSITIVE_INFINITY);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd next && npx.cmd vitest run src/lib/libraryUi.test.ts`
Expected: FAIL — the three new helpers are not exported.

- [ ] **Step 3: Write the implementation**

Append to `next/src/lib/libraryUi.ts`:

```ts
/// The airing fields the Watching column needs. Structural, so CalendarEntry
/// and test fixtures both satisfy it.
export interface AiringLike {
  anime_id: number;
  next_episode: number | null;
  airing_at: number | null;
}

/// Earliest still-future airing per anime.
///
/// `get_calendar` returns one entry per airing episode across a window that
/// reaches ~a month into the past, so past entries are filtered out rather
/// than assumed absent. A show with nothing upcoming is simply missing from
/// the map, which the column renders as a dash.
export function nextAiringByAnime<T extends AiringLike>(
  entries: T[],
  nowSec: number,
): Map<number, T> {
  const out = new Map<number, T>();
  for (const e of entries) {
    if (e.airing_at == null || e.airing_at <= nowSec) continue;
    const existing = out.get(e.anime_id);
    if (!existing || (existing.airing_at as number) > e.airing_at) {
      out.set(e.anime_id, e);
    }
  }
  return out;
}

/// Countdown at day/hour granularity: "6d 14h" / "14h 3m" / "3m".
///
/// Deliberately separate from DetailView's formatCountdown and CalendarView's
/// countdown: those two already disagree (seconds tier, "Aired" vs "airing
/// now"), so unifying them would change what those views display.
export function formatAiringCountdown(secs: number): string {
  if (secs <= 0) return 'airing now';
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

/// Sort position by next airing. Shows with nothing upcoming sort last.
export function nextAiringSortVal(
  animeId: number,
  map: Map<number, AiringLike>,
): number {
  const hit = map.get(animeId);
  return hit?.airing_at ?? Number.POSITIVE_INFINITY;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd next && npx.cmd vitest run src/lib/libraryUi.test.ts`
Expected: PASS — every case from Tasks 2, 3 and 6.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/libraryUi.ts next/src/lib/libraryUi.test.ts
git commit -m "feat: next-airing helpers for the Library view

Reduces the calendar's per-episode entries to the earliest future
airing per show. The countdown formatter is deliberately a third one:
DetailView's and CalendarView's already disagree, and merging them
would change what those views display."
```

---

### Task 7: Next-episode column on Watching

**Files:**
- Modify: `next/src/lib/LibraryView.svelte`

**Interfaces:**
- Consumes: `nextAiringByAnime`, `formatAiringCountdown`, `nextAiringSortVal` (Task 6); `getCalendar` and `type CalendarEntry` from `./api`; `columnCount` (Task 4)
- Produces: nothing consumed downstream

- [ ] **Step 1: Extend the imports**

Add `getCalendar` and `type CalendarEntry` to the existing `./api` import, and extend the `./libraryUi` import with `nextAiringByAnime`, `formatAiringCountdown`, `nextAiringSortVal`.

- [ ] **Step 2: Load the calendar and tick the clock**

Below the `stats` declaration, add:

```ts
  let calendar: CalendarEntry[] = [];
  let nowSec = Math.floor(Date.now() / 1000);
  let clockTimer: ReturnType<typeof setInterval> | undefined;

  // Cached behind CALENDAR_CACHE_TTL_SECS in the backend and already scoped to
  // watching + plan-to-watch shows, so this is free on a warm cache. Failure is
  // non-fatal: the column falls back to a dash.
  async function loadCalendar() {
    try { calendar = await getCalendar(); } catch { calendar = []; }
  }
```

In `onMount`, add the load and start the timer:

```ts
  onMount(() => {
    void load();
    void loadStats();
    void loadCalendar();
    // The column shows days and hours, so a minute is fine-grained enough.
    clockTimer = setInterval(() => { nowSec = Math.floor(Date.now() / 1000); }, 60_000);
  });
```

In `onDestroy`, clear it:

```ts
  onDestroy(() => {
    if (confirmDeleteTimer) clearTimeout(confirmDeleteTimer);
    if (clockTimer) clearInterval(clockTimer);
  });
```

- [ ] **Step 3: Derive the lookup and widen `columnCount`**

Task 4 placed `showSeason` and `columnCount` together after `displayRows`. Add `nextAiring` and `showNextEpisode` in that same block, and extend `columnCount`:

```ts
  $: nextAiring = nextAiringByAnime(calendar, nowSec);
  $: showNextEpisode = statusFilter === 'watching';
  // check + thumb + title + status + progress + files, plus optional columns.
  $: columnCount = 6 + (showSeason ? 1 : 0) + (showNextEpisode ? 1 : 0);
```

`showNextEpisode` must be declared above `columnCount`, or svelte-check reports a use-before-define. Both `colspan` sites already read `columnCount`, so no other changes are needed there.

- [ ] **Step 4: Add the sort key**

Change the `SortKey` type:

```ts
  type SortKey = 'title' | 'status' | 'progress' | 'season' | 'next_airing';
```

Change Watching's default in `DEFAULT_SORT`:

```ts
  const DEFAULT_SORT: Record<string, Sort> = {
    watching: { key: 'next_airing', dir: 'asc' },
    on_hold: { key: 'progress', dir: 'asc' },
    plan_to_watch: { key: 'season', dir: 'asc' },
  };
```

Add the case inside the `sortedEntries` switch, after `case 'season':`:

```ts
        case 'next_airing': {
          const va = nextAiringSortVal(a.anime_id, nextAiring);
          const vb = nextAiringSortVal(b.anime_id, nextAiring);
          // Both missing: fall back to title so the tail has a stable order.
          if (va === vb) { cmp = a.title.localeCompare(b.title); break; }
          // Shows with nothing upcoming sort last in both directions. These
          // early returns bypass the `cmp * dir` at the end of the comparator,
          // which is exactly what pins the tail.
          if (va === Number.POSITIVE_INFINITY) return 1;
          if (vb === Number.POSITIVE_INFINITY) return -1;
          cmp = va - vb;
          break;
        }
```

- [ ] **Step 5: Add the column header**

In `<thead>`, directly after the `{/if}` closing the Season `<th>`, add:

```svelte
            {#if showNextEpisode}
              <th
                class="col-airing"
                scope="col"
                aria-sort={sortKey === 'next_airing' ? (sortDir === 'asc' ? 'ascending' : 'descending') : 'none'}
              >
                <button
                  type="button"
                  class="sort-btn"
                  aria-label="Sort by next episode"
                  on:click={() => setSort('next_airing')}
                >
                  Next Episode
                  {#if sortKey === 'next_airing'}
                    <span aria-hidden="true" class="sort-arrow">
                      {#if sortDir === 'asc'}<ChevronUp size={13} />{:else}<ChevronDown size={13} />{/if}
                    </span>
                  {/if}
                </button>
              </th>
            {/if}
```

- [ ] **Step 6: Add the cell**

In the single `<tr class="data-row">` block (Task 4 collapsed both modes into one loop), directly after the `{#if showSeason}` season cell block, add:

```svelte
                {#if showNextEpisode}
                  <td class="col-airing airing-cell">
                    {@const na = nextAiring.get(entry.anime_id)}
                    {#if na}
                      <span class="airing-in" class:soon={(na.airing_at ?? 0) - nowSec < 86400}>
                        in {formatAiringCountdown((na.airing_at ?? 0) - nowSec)}
                      </span>
                      <span class="airing-sub">
                        Ep {na.next_episode} · {new Date((na.airing_at ?? 0) * 1000).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}
                      </span>
                    {:else}
                      <span class="no-status">—</span>
                    {/if}
                  </td>
                {/if}
```

**Do not** write `{#if nextAiring.has(…)}` and then `.get(…)` inside it: TypeScript does not narrow a `Map.get()` result through a `.has()` check, so `na` stays `CalendarEntry | undefined` and `npm run check` fails. Bind with `{@const}` first, then `{#if na}` — that narrows.

- [ ] **Step 7: Add the grid-card line**

In the single poster-card block, directly after the `.progress-wrap` div, add:

```svelte
            {#if showNextEpisode}
              {@const na = nextAiring.get(entry.anime_id)}
              {#if na}
                <span class="airing-in" class:soon={(na.airing_at ?? 0) - nowSec < 86400}>
                  in {formatAiringCountdown((na.airing_at ?? 0) - nowSec)}
                </span>
              {/if}
            {/if}
```

Same narrowing rule as Step 6 — `{@const}` then `{#if na}`, never `.has()` as the guard. The card shows nothing at all when there is no upcoming episode (unlike the table, which needs the dash to hold the column).

- [ ] **Step 8: Style the cell**

Add to the `<style>` block, after the `.season-cell` rule:

```css
  .airing-cell { white-space: nowrap; line-height: 1.25; }

  .airing-in {
    display: block;
    color: var(--color-text);
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
  }

  .airing-in.soon { color: var(--color-accent); font-weight: 650; }

  .airing-sub {
    display: block;
    color: var(--color-muted);
    font-size: 0.7rem;
    font-variant-numeric: tabular-nums;
  }

  table.compact .airing-in { font-size: 0.72rem; }
  table.compact .airing-sub { font-size: 0.64rem; }
```

- [ ] **Step 9: Verify**

Run: `cd next && npm.cmd run verify`
Expected: all clean.

- [ ] **Step 10: Manual check**

Run the app and confirm, on Watching:
- Each currently-airing show shows a countdown with `Ep N · Mon D` beneath.
- A finished show shows a dash.
- Anything within 24h is accented.
- Clicking the column header sorts, with dashed rows staying at the bottom in both directions.
- The column is absent on every other tab.

- [ ] **Step 11: Commit**

```bash
git add next/src/lib/LibraryView.svelte
git commit -m "feat: next-episode countdown column on Watching

Sourced from get_calendar rather than get_next_airing: the calendar is
already cached, already scoped to watching + plan-to-watch shows, and
batches into one request, where get_next_airing is one live GraphQL
query per anime and would mean a request per row on every load.

Sortable, and now Watching's default sort. Shows with nothing upcoming
sort last in both directions."
```

---

## Post-Implementation

- [ ] Run the full suite once more: `cd next && npm.cmd run verify`
- [ ] Run `cd next/src-tauri && cargo test` (should be untouched; use `CARGO_INCREMENTAL=0` if it ICEs)
- [ ] Update `SESSION_STATE.md` with what shipped and what was deferred
- [ ] **Ask the user before building.** Per CLAUDE.md: bump to 1.0.20 in all four places, `npm run bundle`, `chore: release 1.0.20` + tag, push, `gh release create` — **only once they say go.**

## Deferred (explicitly not in this plan)

- **`loadEpisodeFiles` caps at 50 entries** with a sequential `await` per anime. Independent of the row-cap bug, but with 627 rows on screen it shows as "only the top rows have download bars". Proper fix is one batched command taking a list of ids.
- **No grid virtualization.** Pre-existing; 627 cards in one flat grid is in the range the Seasons page already tolerates.
- **A Plan to Watch show whose season has already started** groups under that past season and sorts above the next one. Accepted; fix is to fold pre-current seasons into one group pinned to the bottom.
