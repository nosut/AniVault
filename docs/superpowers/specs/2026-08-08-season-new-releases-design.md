# Newly Added Shows on the Seasons Page — Design

**Date:** 2026-08-08
**Status:** Approved

## Problem

`get_season_anime` (`commands.rs:1212`) queries AniList live on every view and stores
nothing. The Seasons page therefore has no memory of what a season contained: a show
announced last week is rendered identically to one that has been listed for months.
Anyone tracking an upcoming season has to remember the previous listing themselves and
eyeball the grid for differences.

## Goal

When you revisit a season, show the shows added since you last looked, grouped at the
top of the page, so new announcements are obvious without a manual comparison.

## Approach

Record, per season, the set of anime ids that season was known to contain, and diff a
fresh AniList listing against it.

Two alternatives were considered and rejected:

- **Keep the seen-set in `localStorage`**, alongside `anivault-season-state` and the
  start-page preference. No backend work and no migration, but those keys hold view
  state that is cheap to lose. This is a record whose loss silently produces a false
  "everything is new" page. It belongs in SQLite with `file_index` and the rest of the
  real data.
- **Ask AniList when each show was added to the season.** This would mean genuinely
  "newly announced" rather than "new to you", at near-zero storage cost. Rejected: the
  season query exposes no reliable added-to-season timestamp, so it would need a
  different query and still would not be trustworthy.

## Behaviour

A show is new until the season has been viewed once. Concretely:

| Visit | AniList returns | Flagged | Recorded |
|---|---|---|---|
| First ever | 12 shows | nothing | all 12, silently |
| Next day | 12 shows | nothing | no change |
| Friday | 14 shows | the 2 additions | the 2 additions |
| Friday, a minute later | 14 shows | nothing | no change |
| A show is delisted | 13 shows | nothing | no change |

Removals are never announced. Nothing expires: a season's rows persist until the
season itself is irrelevant, which costs about 50 rows per season visited.

### The first visit must flag nothing

On a season with no stored rows, every id is trivially "not yet recorded". Reporting
those as new would light up the entire grid on first view — the exact opposite of the
feature's purpose. The backend therefore reports `first_visit` separately, and the
frontend renders no band when it is set.

### The genre filter must not corrupt the baseline

The genre dropdown filters server-side, so a listing fetched under "Mecha" contains
only mecha shows. Recording that as the season's seen-set would mark every non-mecha
show in the season new on the next unfiltered visit.

Detection and recording are therefore separated. A filtered view still reports what is
new — any returned id absent from the stored set is genuinely new, filter or not — but
writes nothing. Only an unfiltered view extends the baseline.

### A filtered first visit leaves the season unbaselined

The two rules above interact: visiting a season for the first time *with a genre
selected* reports nothing as new (`first_visit`) and records nothing (`record: false`),
so the season still has no baseline. The first unfiltered visit is then treated as the
first visit and baselines the full listing. This is the correct outcome — a baseline
built from a filtered listing is exactly what the `record` flag exists to prevent — and
it means `first_visit` tracks "has a baseline", not "has ever been opened".

The same follows for a season AniList returns as empty: nothing is recorded, so it
stays unbaselined until it has shows to record.

### The flag must not clear out from under you

`Season` is one of the `START_PAGE_OPTIONS` (`startPage.ts`), so launching the app can
itself be the visit that consumes the flag. The new-id set therefore lives in component
state and is cleared only when the season, year, or genre changes — not on a timer and
not after rendering. The band stays for as long as you remain on that season; it is
gone on the next visit, which is the agreed behaviour.

## Design

### Migration

`next/src-tauri/migrations/0009_season_seen.sql`:

```sql
-- Which shows a season was already known to contain the last time it was viewed.
-- The Seasons page fetches from AniList live and keeps no other memory of a
-- season, so diffing a fresh listing against these rows is the only way to know
-- what was added since the last visit.
--
-- Deliberately no foreign key to `anime`: season listings are AniList ids the
-- user has not imported, so an FK would reject exactly the rows this table
-- exists to hold.
CREATE TABLE IF NOT EXISTS season_seen (
  season        TEXT    NOT NULL,
  year          INTEGER NOT NULL,
  anime_id      INTEGER NOT NULL,
  first_seen_at INTEGER NOT NULL,
  PRIMARY KEY (season, year, anime_id)
);
```

Picked up automatically by `sqlx::migrate!("./migrations")` (`storage.rs:286`).

The Future Seasons page has no season/year of its own and is stored under the sentinel
key `('__FUTURE__', 0)`. It shares the whole code path; far-future and TBA
announcements are where new listings most often appear, so excluding it would miss the
common case.

### Storage layer

Two methods on `Storage`, keeping SQL out of the command layer as elsewhere:

```rust
pub async fn season_seen_ids(&self, season: &str, year: i32) -> anyhow::Result<Vec<i64>>;
pub async fn record_season_seen(
    &self,
    season: &str,
    year: i32,
    ids: &[i64],
    now: i64,
) -> anyhow::Result<()>;
```

`record_season_seen` uses `INSERT OR IGNORE` so re-recording an existing id preserves
its original `first_seen_at`.

### Command

One command, so the compare and the record are a single round trip and cannot tear:

```rust
pub struct SeasonDiff {
    /// No rows stored for this season yet. The caller must flag nothing.
    pub first_visit: bool,
    /// Ids present in `ids` but not previously recorded. Empty when `first_visit`.
    pub new_ids: Vec<i64>,
}

pub async fn diff_season(
    season: String,
    year: i32,
    ids: Vec<i64>,
    record: bool,
    state: tauri::State<'_, EngineState>,
) -> Result<SeasonDiff, String>;
```

Logic, in one transaction:

1. Read the stored set for `(season, year)`.
2. `first_visit = stored.is_empty()`.
3. `new_ids = if first_visit { vec![] } else { ids not in stored }`.
4. If `record`, insert every id in `ids`.

Split into a `diff_season_inner(&EngineState, ...)` taking `&EngineState` plus a thin
`#[tauri::command]` wrapper, matching `get_season_anime_inner` / `get_season_anime`.
The inner function is what the tests drive.

### Frontend

**`next/src/lib/seasonNew.ts`** — pure, unit-tested, following the `seasonUi.ts` and
`homeUi.ts` convention of keeping logic out of the component:

```ts
export function partitionNew<T extends { id: number }>(
  entries: T[],
  newIds: Set<number>,
): { fresh: T[]; rest: T[] };
```

Preserves AniList's ordering within each partition.

**`next/src/lib/api.ts`** — `diffSeason(season, year, ids, record, invokeFn?)`,
following the existing injectable-`InvokeFn` signature so it is testable.

**`next/src/lib/SeasonView.svelte`**:

- After `load()` resolves, call `diffSeason` with `record: genre === ''`, passing the
  fetched ids and the sentinel key when `future` is set.
- Store the result in `newIds: Set<number>`, cleared whenever season, year, or genre
  changes.
- Render the band above the grid when `newIds` is non-empty, using `partitionNew` to
  hold flagged shows out of the lower grid so nothing appears twice.

Band styling reuses the existing token set. The new-item accent is
`--color-warning` (amber): `--color-success` already means *In Library* on this exact
card, and reusing it would make two unrelated states look alike.

The lower grid gets a `Rest of the season` divider only while the band is showing, so
an ordinary visit renders exactly the page that exists today.

### Error handling

The diff call is wrapped so any failure leaves `newIds` empty and the page renders as
it does now. This is a convenience layer over a live API call; it must never block the
season grid or raise an error over it. The existing "not connected" path for
`get_season_anime` is untouched.

## Testing

**Rust** (`cargo test`, in-memory storage via `fresh_test_state`):

- First visit records the full baseline and reports `first_visit: true` with no new ids.
- A second visit with two extra ids reports exactly those two.
- `record: false` reports new ids and leaves the table unchanged.
- A delisted show does not resurrect as new when it reappears.
- `record_season_seen` preserves the original `first_seen_at` on re-record.
- Future-page rows key independently of any real season.

**TypeScript** (`vitest`):

- `partitionNew` splits correctly, preserves order, and handles an empty id set.
- `api.diffSeason` passes the expected command name and arguments.
- `SeasonView.test.ts`: mounting with a mocked API reporting new ids renders the band
  with the correct count, and the flagged shows do not also appear in the lower grid.

## Out of scope

- Announcing removals from a season.
- Any expiry or "seen N days ago" window; the flag clears after one visit.
- Surfacing the new count outside the Seasons page (e.g. a badge on the sidebar item).
- Backfilling a baseline for seasons never visited.
