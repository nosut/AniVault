# Newly Added Shows on the Seasons Page — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Group the shows added to a season since the user last viewed it at the top of the Seasons page.

**Architecture:** A new `season_seen` SQLite table records the anime ids each season was known to contain. One Tauri command diffs a freshly fetched AniList listing against those rows and optionally records the result, in a single transaction. `SeasonView.svelte` renders the returned ids as a banded group above the normal poster grid.

**Tech Stack:** Rust + Tauri v2 + sqlx (SQLite) backend; Svelte 4 + TypeScript frontend; `cargo test` and vitest.

**Spec:** `docs/superpowers/specs/2026-08-08-season-new-releases-design.md`

## Global Constraints

- Windows-only Tauri desktop app. All code lives under `next/`.
- Verify with `npm run verify` in `next/` (tsc + svelte-check + vitest + `cargo check --tests`) and `cargo test` in `next/src-tauri`.
- If `cargo test` hits an internal compiler error, it is a stale incremental cache, not the diff: re-run with `CARGO_INCREMENTAL=0`.
- Do **not** bump the version or build an installer. The user decides when a build cuts, and asks explicitly.
- The Future Seasons page is keyed with the sentinel `season = "__FUTURE__"`, `year = 0`.
- New-item accent is `--color-warning`. `--color-success` already means *In Library* on the same card and must not be reused.
- SQL lives in `storage.rs`, never in `commands.rs`.
- Commands are split `foo_inner(&EngineState, ...) -> anyhow::Result<T>` plus a thin `#[tauri::command] foo(...) -> Result<T, String>` wrapper, matching `get_season_anime_inner` / `get_season_anime`.

---

## File Structure

**Create:**
- `next/src-tauri/migrations/0009_season_seen.sql` — the table.
- `next/src-tauri/tests/season_seen_storage_test.rs` — storage-layer tests.
- `next/src-tauri/tests/season_seen_commands_test.rs` — `diff_season_inner` tests.
- `next/src/lib/seasonNew.ts` — pure partition helper.
- `next/src/lib/seasonNew.test.ts` — its tests.
- `next/src/lib/SeasonPosterCard.svelte` — the poster card, extracted so the band and the main grid share one copy.

**Modify:**
- `next/src-tauri/src/engine/storage.rs` — add `season_seen_ids`, `record_season_seen`.
- `next/src-tauri/src/commands.rs` — add `SeasonDiff`, `diff_season_inner`, `diff_season`.
- `next/src-tauri/src/lib.rs:242` — register `commands::diff_season` in the handler list.
- `next/src/lib/api.ts` — add `SeasonDiff` type and `diffSeason`.
- `next/src/lib/api.test.ts` — cover `diffSeason`.
- `next/src/lib/SeasonView.svelte` — call the command, render the band.
- `next/src/lib/SeasonView.test.ts` — extend the `./api` mock, cover the band.

---

### Task 1: `season_seen` table and storage methods

**Files:**
- Create: `next/src-tauri/migrations/0009_season_seen.sql`
- Modify: `next/src-tauri/src/engine/storage.rs`
- Test: `next/src-tauri/tests/season_seen_storage_test.rs`

**Interfaces:**
- Consumes: `Storage::connect`, `Storage::migrate` (existing).
- Produces:
  - `Storage::season_seen_ids(&self, season: &str, year: i32) -> anyhow::Result<Vec<i64>>`
  - `Storage::record_season_seen(&self, season: &str, year: i32, ids: &[i64], now: i64) -> anyhow::Result<()>`
  - `Storage::season_first_seen_at(&self, season: &str, year: i32, anime_id: i64) -> anyhow::Result<Option<i64>>` — test support only, not called by the app

- [ ] **Step 1: Write the failing test**

Create `next/src-tauri/tests/season_seen_storage_test.rs`:

```rust
use anivault_core::engine::storage::Storage;

async fn new_storage() -> Storage {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage
}

fn sorted(mut ids: Vec<i64>) -> Vec<i64> {
    ids.sort_unstable();
    ids
}

#[tokio::test]
async fn an_unseen_season_has_no_ids() {
    let storage = new_storage().await;
    let ids = storage.season_seen_ids("FALL", 2026).await.unwrap();
    assert!(ids.is_empty());
}

#[tokio::test]
async fn recorded_ids_come_back() {
    let storage = new_storage().await;
    storage
        .record_season_seen("FALL", 2026, &[3, 1, 2], 1000)
        .await
        .unwrap();
    let ids = storage.season_seen_ids("FALL", 2026).await.unwrap();
    assert_eq!(sorted(ids), vec![1, 2, 3]);
}

#[tokio::test]
async fn seasons_are_keyed_independently() {
    let storage = new_storage().await;
    storage.record_season_seen("FALL", 2026, &[1], 1000).await.unwrap();
    storage.record_season_seen("FALL", 2027, &[2], 1000).await.unwrap();
    storage.record_season_seen("SUMMER", 2026, &[3], 1000).await.unwrap();

    assert_eq!(storage.season_seen_ids("FALL", 2026).await.unwrap(), vec![1]);
    assert_eq!(storage.season_seen_ids("FALL", 2027).await.unwrap(), vec![2]);
    assert_eq!(storage.season_seen_ids("SUMMER", 2026).await.unwrap(), vec![3]);
}

#[tokio::test]
async fn the_future_sentinel_is_just_another_key() {
    let storage = new_storage().await;
    storage
        .record_season_seen("__FUTURE__", 0, &[7], 1000)
        .await
        .unwrap();
    assert_eq!(
        storage.season_seen_ids("__FUTURE__", 0).await.unwrap(),
        vec![7]
    );
    assert!(storage.season_seen_ids("FALL", 2026).await.unwrap().is_empty());
}

#[tokio::test]
async fn re_recording_keeps_the_original_first_seen_at() {
    // first_seen_at is the "when did this show up" record. Re-recording an id on
    // every visit must not keep pushing it forward, or it stops meaning anything.
    let storage = new_storage().await;
    storage.record_season_seen("FALL", 2026, &[1], 1000).await.unwrap();
    storage.record_season_seen("FALL", 2026, &[1, 2], 5000).await.unwrap();

    let first = storage.season_first_seen_at("FALL", 2026, 1).await.unwrap();
    let second = storage.season_first_seen_at("FALL", 2026, 2).await.unwrap();
    assert_eq!(first, Some(1000), "an existing row is left alone");
    assert_eq!(second, Some(5000), "a new row gets the current time");
}

#[tokio::test]
async fn recording_nothing_is_a_no_op() {
    let storage = new_storage().await;
    storage.record_season_seen("FALL", 2026, &[], 1000).await.unwrap();
    assert!(storage.season_seen_ids("FALL", 2026).await.unwrap().is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd next/src-tauri && CARGO_INCREMENTAL=0 cargo test --test season_seen_storage_test`
Expected: FAIL — compile error, `no method named season_seen_ids found for struct Storage`.

- [ ] **Step 3: Create the migration**

Create `next/src-tauri/migrations/0009_season_seen.sql`:

```sql
-- Which shows a season was already known to contain the last time it was viewed.
-- The Seasons page fetches from AniList live and keeps no other memory of a
-- season, so diffing a fresh listing against these rows is the only way to know
-- what was added since the last visit.
--
-- Deliberately no foreign key to `anime`: season listings are AniList ids the
-- user has not imported, so an FK would reject exactly the rows this table
-- exists to hold.
--
-- The Future Seasons page has no season of its own and is stored under the
-- sentinel key ('__FUTURE__', 0).
CREATE TABLE IF NOT EXISTS season_seen (
  season        TEXT    NOT NULL,
  year          INTEGER NOT NULL,
  anime_id      INTEGER NOT NULL,
  first_seen_at INTEGER NOT NULL,
  PRIMARY KEY (season, year, anime_id)
);
```

- [ ] **Step 4: Add the storage methods**

In `next/src-tauri/src/engine/storage.rs`, next to `all_library_ids` (around line 1554):

```rust
    /// Anime ids this season was known to contain as of the last recorded view.
    pub async fn season_seen_ids(&self, season: &str, year: i32) -> anyhow::Result<Vec<i64>> {
        let ids: Vec<i64> =
            sqlx::query_scalar("SELECT anime_id FROM season_seen WHERE season = ? AND year = ?")
                .bind(season)
                .bind(year)
                .fetch_all(&self.pool)
                .await?;
        Ok(ids)
    }

    /// Add ids to a season's seen-set. `INSERT OR IGNORE` so an id already
    /// recorded keeps its original `first_seen_at` — that column answers "when
    /// did this show appear", which a re-record must not overwrite.
    pub async fn record_season_seen(
        &self,
        season: &str,
        year: i32,
        ids: &[i64],
        now: i64,
    ) -> anyhow::Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for id in ids {
            sqlx::query(
                "INSERT OR IGNORE INTO season_seen (season, year, anime_id, first_seen_at) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(season)
            .bind(year)
            .bind(id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// When an id was first recorded for a season. Test support for the
    /// `INSERT OR IGNORE` behaviour above; not used by the app.
    pub async fn season_first_seen_at(
        &self,
        season: &str,
        year: i32,
        anime_id: i64,
    ) -> anyhow::Result<Option<i64>> {
        let at: Option<i64> = sqlx::query_scalar(
            "SELECT first_seen_at FROM season_seen \
             WHERE season = ? AND year = ? AND anime_id = ?",
        )
        .bind(season)
        .bind(year)
        .bind(anime_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(at)
    }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd next/src-tauri && CARGO_INCREMENTAL=0 cargo test --test season_seen_storage_test`
Expected: PASS — 6 tests.

- [ ] **Step 6: Confirm the migration test still passes**

Run: `cd next/src-tauri && CARGO_INCREMENTAL=0 cargo test --test migration_test`
Expected: PASS. This suite asserts the schema applies cleanly; a malformed migration shows up here.

- [ ] **Step 7: Commit**

```bash
git add next/src-tauri/migrations/0009_season_seen.sql next/src-tauri/src/engine/storage.rs next/src-tauri/tests/season_seen_storage_test.rs
git commit -m "feat: store which shows each season was known to contain"
```

---

### Task 2: `diff_season` command

**Files:**
- Modify: `next/src-tauri/src/commands.rs` (add after `get_season_anime`, around line 1255)
- Modify: `next/src-tauri/src/lib.rs:242`
- Test: `next/src-tauri/tests/season_seen_commands_test.rs`

**Interfaces:**
- Consumes: `Storage::season_seen_ids`, `Storage::record_season_seen` from Task 1; `fresh_test_state()` from `anivault_core::engine::runtime`.
- Produces:
  - `pub struct SeasonDiff { pub first_visit: bool, pub new_ids: Vec<i64> }` (serde `Serialize`)
  - `diff_season_inner(&EngineState, season: String, year: i32, ids: Vec<i64>, record: bool) -> anyhow::Result<SeasonDiff>`
  - `#[tauri::command] diff_season(...)` registered as `diff_season`

- [ ] **Step 1: Write the failing test**

Create `next/src-tauri/tests/season_seen_commands_test.rs`:

```rust
use anivault_core::commands::diff_season_inner;
use anivault_core::engine::runtime::{fresh_test_state, EngineState};

async fn state() -> EngineState {
    fresh_test_state().await
}

fn sorted(mut ids: Vec<i64>) -> Vec<i64> {
    ids.sort_unstable();
    ids
}

#[tokio::test]
async fn the_first_visit_flags_nothing_and_records_the_baseline() {
    // Every id is trivially "not yet recorded" on a first visit. Reporting them
    // would light up the entire grid, which is the opposite of the point.
    let state = state().await;
    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2, 3], true)
        .await
        .unwrap();

    assert!(diff.first_visit);
    assert!(diff.new_ids.is_empty(), "nothing is new on a first visit");
    assert_eq!(
        sorted(state.storage.season_seen_ids("FALL", 2026).await.unwrap()),
        vec![1, 2, 3],
        "the baseline is still recorded"
    );
}

#[tokio::test]
async fn a_later_visit_reports_only_the_additions() {
    let state = state().await;
    diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2, 3], true)
        .await
        .unwrap();

    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2, 3, 4, 5], true)
        .await
        .unwrap();

    assert!(!diff.first_visit);
    assert_eq!(sorted(diff.new_ids), vec![4, 5]);
    assert_eq!(
        sorted(state.storage.season_seen_ids("FALL", 2026).await.unwrap()),
        vec![1, 2, 3, 4, 5]
    );
}

#[tokio::test]
async fn seeing_the_same_listing_again_reports_nothing() {
    let state = state().await;
    diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2], true).await.unwrap();
    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2], true)
        .await
        .unwrap();
    assert!(!diff.first_visit);
    assert!(diff.new_ids.is_empty());
}

#[tokio::test]
async fn a_filtered_view_reports_without_recording() {
    // A genre-filtered listing must never become the baseline: it contains only
    // that genre, so recording it would mark the rest of the season new.
    let state = state().await;
    diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2], true).await.unwrap();

    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![2, 9], false)
        .await
        .unwrap();

    assert_eq!(diff.new_ids, vec![9]);
    assert_eq!(
        sorted(state.storage.season_seen_ids("FALL", 2026).await.unwrap()),
        vec![1, 2],
        "the filtered view wrote nothing"
    );
}

#[tokio::test]
async fn a_delisted_show_does_not_resurrect_as_new() {
    let state = state().await;
    diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2], true).await.unwrap();
    // AniList drops 2 …
    diff_season_inner(&state, "FALL".into(), 2026, vec![1], true).await.unwrap();
    // … then lists it again.
    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2], true)
        .await
        .unwrap();
    assert!(diff.new_ids.is_empty(), "it was already known; removals are not tracked");
}

#[tokio::test]
async fn an_empty_listing_leaves_the_season_unbaselined() {
    let state = state().await;
    let diff = diff_season_inner(&state, "FALL".into(), 2030, vec![], true)
        .await
        .unwrap();
    assert!(diff.first_visit);
    assert!(diff.new_ids.is_empty());

    // Nothing was recorded, so the next real listing is still a first visit.
    let diff = diff_season_inner(&state, "FALL".into(), 2030, vec![4], true)
        .await
        .unwrap();
    assert!(diff.first_visit);
    assert!(diff.new_ids.is_empty());
}

#[tokio::test]
async fn a_filtered_first_visit_leaves_the_season_unbaselined() {
    // The two rules interact: a first visit flags nothing, and a filtered view
    // records nothing, so the season still has no baseline afterwards. The next
    // unfiltered visit is therefore still the first visit. `first_visit` tracks
    // "has a baseline", not "has ever been opened".
    let state = state().await;
    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![1], false)
        .await
        .unwrap();
    assert!(diff.first_visit);
    assert!(state.storage.season_seen_ids("FALL", 2026).await.unwrap().is_empty());

    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![1, 2, 3], true)
        .await
        .unwrap();
    assert!(diff.first_visit, "the unfiltered visit baselines the full listing");
    assert!(diff.new_ids.is_empty());
    assert_eq!(
        sorted(state.storage.season_seen_ids("FALL", 2026).await.unwrap()),
        vec![1, 2, 3]
    );
}

#[tokio::test]
async fn the_future_page_keys_independently_of_real_seasons() {
    let state = state().await;
    diff_season_inner(&state, "__FUTURE__".into(), 0, vec![1], true).await.unwrap();
    let diff = diff_season_inner(&state, "FALL".into(), 2026, vec![1], true)
        .await
        .unwrap();
    assert!(diff.first_visit, "a real season is untouched by the future page");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd next/src-tauri && CARGO_INCREMENTAL=0 cargo test --test season_seen_commands_test`
Expected: FAIL — compile error, `unresolved import anivault_core::commands::diff_season_inner`.

- [ ] **Step 3: Write the implementation**

In `next/src-tauri/src/commands.rs`, after `get_season_anime` (around line 1255):

```rust
/// Result of comparing a freshly fetched season listing against what the user
/// has already seen.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SeasonDiff {
    /// This season has no recorded baseline yet, so nothing may be flagged —
    /// on a first visit every id is trivially unseen and flagging them would
    /// mark the whole season new.
    pub first_visit: bool,
    /// Ids present in the listing but not previously recorded. Always empty
    /// when `first_visit`.
    pub new_ids: Vec<i64>,
}

/// Compare `ids` against the season's seen-set and, when `record`, extend it.
///
/// `record` must be false whenever the listing was genre-filtered: such a
/// listing holds only part of the season, and baselining it would mark every
/// other show new on the next unfiltered visit.
pub async fn diff_season_inner(
    state: &EngineState,
    season: String,
    year: i32,
    ids: Vec<i64>,
    record: bool,
) -> anyhow::Result<SeasonDiff> {
    let seen: std::collections::HashSet<i64> = state
        .storage
        .season_seen_ids(&season, year)
        .await?
        .into_iter()
        .collect();

    let first_visit = seen.is_empty();
    let new_ids = if first_visit {
        Vec::new()
    } else {
        ids.iter().copied().filter(|id| !seen.contains(id)).collect()
    };

    if record {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        state
            .storage
            .record_season_seen(&season, year, &ids, now)
            .await?;
    }

    Ok(SeasonDiff {
        first_visit,
        new_ids,
    })
}

#[tauri::command]
pub async fn diff_season(
    season: String,
    year: i32,
    ids: Vec<i64>,
    record: bool,
    state: tauri::State<'_, EngineState>,
) -> Result<SeasonDiff, String> {
    diff_season_inner(&state, season, year, ids, record)
        .await
        .map_err(command_error)
}
```

- [ ] **Step 4: Register the command**

In `next/src-tauri/src/lib.rs`, add to the `generate_handler!` list next to `commands::get_season_anime` (line 242):

```rust
            commands::diff_season,
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd next/src-tauri && CARGO_INCREMENTAL=0 cargo test --test season_seen_commands_test`
Expected: PASS — 8 tests.

- [ ] **Step 6: Check the whole backend still builds**

Run: `cd next/src-tauri && CARGO_INCREMENTAL=0 cargo check --tests`
Expected: `Finished`, no errors.

- [ ] **Step 7: Commit**

```bash
git add next/src-tauri/src/commands.rs next/src-tauri/src/lib.rs next/src-tauri/tests/season_seen_commands_test.rs
git commit -m "feat: add diff_season command for new-since-last-visit ids"
```

---

### Task 3: `partitionNew` helper

**Files:**
- Create: `next/src/lib/seasonNew.ts`
- Test: `next/src/lib/seasonNew.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: `partitionNew<T extends { id: number }>(entries: T[], newIds: Set<number>): { fresh: T[]; rest: T[] }`

- [ ] **Step 1: Write the failing test**

Create `next/src/lib/seasonNew.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { partitionNew } from './seasonNew';

const entry = (id: number) => ({ id, title: `Show ${id}` });

describe('partitionNew', () => {
  it('splits entries into new and the rest', () => {
    const entries = [entry(1), entry(2), entry(3)];
    const { fresh, rest } = partitionNew(entries, new Set([2]));
    expect(fresh.map((e) => e.id)).toEqual([2]);
    expect(rest.map((e) => e.id)).toEqual([1, 3]);
  });

  it('preserves the source order within each partition', () => {
    // AniList returns by popularity; the band and the grid must both keep it.
    const entries = [entry(5), entry(4), entry(3), entry(2), entry(1)];
    const { fresh, rest } = partitionNew(entries, new Set([4, 2]));
    expect(fresh.map((e) => e.id)).toEqual([4, 2]);
    expect(rest.map((e) => e.id)).toEqual([5, 3, 1]);
  });

  it('puts everything in rest when nothing is new', () => {
    const entries = [entry(1), entry(2)];
    const { fresh, rest } = partitionNew(entries, new Set());
    expect(fresh).toEqual([]);
    expect(rest.map((e) => e.id)).toEqual([1, 2]);
  });

  it('ignores ids that are not in the listing', () => {
    // A show flagged new can vanish from the next listing; it must not
    // conjure a phantom entry.
    const entries = [entry(1)];
    const { fresh, rest } = partitionNew(entries, new Set([99]));
    expect(fresh).toEqual([]);
    expect(rest.map((e) => e.id)).toEqual([1]);
  });

  it('handles an empty listing', () => {
    const { fresh, rest } = partitionNew([], new Set([1]));
    expect(fresh).toEqual([]);
    expect(rest).toEqual([]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd next && npx vitest run src/lib/seasonNew.test.ts`
Expected: FAIL — "Failed to resolve import './seasonNew'".

- [ ] **Step 3: Write the implementation**

Create `next/src/lib/seasonNew.ts`:

```ts
// Pure helpers for the Seasons page's "new since your last visit" group, kept
// out of the component for testability — same split as seasonUi.ts.

/**
 * Split a season listing into the entries flagged as new and everything else.
 *
 * Flagged entries are held out of `rest` so the page can render them in the
 * band without also repeating them in the main grid. Source order — AniList's
 * popularity sort — is preserved within each partition.
 */
export function partitionNew<T extends { id: number }>(
  entries: T[],
  newIds: Set<number>,
): { fresh: T[]; rest: T[] } {
  const fresh: T[] = [];
  const rest: T[] = [];
  for (const entry of entries) {
    if (newIds.has(entry.id)) fresh.push(entry);
    else rest.push(entry);
  }
  return { fresh, rest };
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd next && npx vitest run src/lib/seasonNew.test.ts`
Expected: PASS — 5 tests.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/seasonNew.ts next/src/lib/seasonNew.test.ts
git commit -m "feat: add partitionNew helper for the season new-releases band"
```

---

### Task 4: `diffSeason` API binding

**Files:**
- Modify: `next/src/lib/api.ts` (add after `getFutureAnime`)
- Test: `next/src/lib/api.test.ts`

**Interfaces:**
- Consumes: `diff_season` command from Task 2; the existing `InvokeFn` type and `tauriInvoke` default.
- Produces:
  - `export interface SeasonDiff { first_visit: boolean; new_ids: number[] }`
  - `diffSeason(season: string, year: number, ids: number[], record: boolean, invokeFn?: InvokeFn): Promise<SeasonDiff>`
  - `export const FUTURE_SEASON_KEY = '__FUTURE__'`

- [ ] **Step 1: Write the failing test**

In `next/src/lib/api.test.ts`, add `diffSeason` and `FUTURE_SEASON_KEY` to the import list at the top of the file (keep the list alphabetical — `diffSeason` goes after `deleteSetting`, `FUTURE_SEASON_KEY` after `fetchAnimeDetail`), then add this test alongside the `getLibraryIds` test (around line 340):

```ts
  it('diffs a season through invoke', async () => {
    const diff = { first_visit: false, new_ids: [7, 9] };
    const invoke = vi.fn(async () => diff);
    await expect(diffSeason('FALL', 2026, [1, 7, 9], true, invoke)).resolves.toEqual(diff);
    expect(invoke).toHaveBeenCalledWith('diff_season', {
      season: 'FALL',
      year: 2026,
      ids: [1, 7, 9],
      record: true,
    });
  });

  it('uses the future sentinel key for the future page', () => {
    expect(FUTURE_SEASON_KEY).toBe('__FUTURE__');
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd next && npx vitest run src/lib/api.test.ts`
Expected: FAIL — `diffSeason` is not exported from `./api`.

- [ ] **Step 3: Write the implementation**

In `next/src/lib/api.ts`, after `getFutureAnime`:

```ts
/**
 * Season key for the Future Seasons page, which has no season/year of its own.
 * Must match the sentinel the backend stores under.
 */
export const FUTURE_SEASON_KEY = '__FUTURE__';

export interface SeasonDiff {
  /** No baseline recorded yet — the caller must flag nothing. */
  first_visit: boolean;
  /** Ids in the listing that were not previously recorded. */
  new_ids: number[];
}

/**
 * Compare a fetched season listing against what has been seen before.
 *
 * `record` must be false when a genre filter is active: that listing holds only
 * part of the season, and baselining it would mark everything else new next time.
 */
export function diffSeason(
  season: string,
  year: number,
  ids: number[],
  record: boolean,
  invokeFn: InvokeFn = tauriInvoke,
): Promise<SeasonDiff> {
  return invokeFn<SeasonDiff>('diff_season', { season, year, ids, record });
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd next && npx vitest run src/lib/api.test.ts`
Expected: PASS — 59 tests.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/api.ts next/src/lib/api.test.ts
git commit -m "feat: add diffSeason API binding"
```

---

### Task 5: Render the band in SeasonView

**Files:**
- Modify: `next/src/lib/SeasonView.svelte`
- Test: `next/src/lib/SeasonView.test.ts`

**Interfaces:**
- Consumes: `partitionNew` (Task 3); `diffSeason`, `SeasonDiff`, `FUTURE_SEASON_KEY` (Task 4).
- Produces: no exports; DOM contract used by tests — `.new-band` container, `.group-count` count text, `.rest-head` divider, `.poster-card.is-new` cards.

- [ ] **Step 1: Write the failing test**

In `next/src/lib/SeasonView.test.ts`, first extend the existing `vi.mock('./api', ...)` factory (around line 15) — it is a full module mock, so any export `SeasonView` imports must be present or the component throws at runtime:

```ts
vi.mock('./api', () => ({
  getSeasonAnime: vi.fn(async () => [seasonEntry]),
  getFutureAnime: vi.fn(async () => [futureEntry]),
  getLibraryIds: vi.fn(async () => []),
  updateListEntry: vi.fn(async () => {}),
  importAnilistAnime: vi.fn(async () => {}),
  diffSeason: vi.fn(async () => ({ first_visit: true, new_ids: [] })),
  FUTURE_SEASON_KEY: '__FUTURE__',
}));
```

Add `diffSeason` to the existing `import { getFutureAnime, getSeasonAnime } from './api';` line, then append this suite at the end of the file:

```ts
describe('SeasonView new-releases band', () => {
  const entries = [
    { ...seasonEntry, id: 1, title: 'Established Show' },
    { ...seasonEntry, id: 7, title: 'Brand New Show' },
  ];

  beforeEach(() => {
    vi.mocked(getSeasonAnime).mockResolvedValue(entries as never);
    vi.mocked(diffSeason).mockResolvedValue({ first_visit: false, new_ids: [7] });
    localStorage.clear();
    document.body.innerHTML = '';
  });

  it('groups new shows in a band and keeps them out of the main grid', async () => {
    const target = document.createElement('div');
    document.body.appendChild(target);
    const component = mount(SeasonView, { target });
    await settle();

    const band = document.querySelector('.new-band');
    expect(band, 'the band renders when something is new').toBeTruthy();
    expect(band?.querySelector('.group-count')?.textContent).toBe('1');

    const bandTitles = [...(band?.querySelectorAll('.poster-title') ?? [])].map((n) => n.textContent);
    expect(bandTitles).toEqual(['Brand New Show']);

    // The flagged show must appear exactly once on the page.
    const allTitles = [...document.querySelectorAll('.poster-title')].map((n) => n.textContent);
    expect(allTitles.filter((t) => t === 'Brand New Show')).toHaveLength(1);
    expect(allTitles).toContain('Established Show');

    unmount(component);
  });

  it('renders no band on a first visit', async () => {
    vi.mocked(diffSeason).mockResolvedValue({ first_visit: true, new_ids: [] });
    const target = document.createElement('div');
    document.body.appendChild(target);
    const component = mount(SeasonView, { target });
    await settle();

    expect(document.querySelector('.new-band')).toBeNull();
    expect(document.querySelector('.rest-head')).toBeNull();
    expect([...document.querySelectorAll('.poster-title')]).toHaveLength(2);

    unmount(component);
  });

  it('still renders the season when the diff call fails', async () => {
    // Newness is a convenience over a live API call and must never be able to
    // block the grid.
    vi.mocked(diffSeason).mockRejectedValue(new Error('db locked'));
    const target = document.createElement('div');
    document.body.appendChild(target);
    const component = mount(SeasonView, { target });
    await settle();

    expect(document.querySelector('.new-band')).toBeNull();
    expect([...document.querySelectorAll('.poster-title')]).toHaveLength(2);
    expect(document.querySelector('.message.error')).toBeNull();

    unmount(component);
  });

  it('does not record a baseline while a genre filter is active', async () => {
    const target = document.createElement('div');
    document.body.appendChild(target);
    const component = mount(SeasonView, { target });
    await settle();
    expect(vi.mocked(diffSeason).mock.calls[0]?.[3]).toBe(true);

    vi.mocked(diffSeason).mockClear();
    const select = document.querySelector('.genre-select') as HTMLSelectElement;
    select.value = 'Mecha';
    select.dispatchEvent(new Event('change'));
    await settle();

    expect(vi.mocked(diffSeason).mock.calls[0]?.[3]).toBe(false);

    unmount(component);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd next && npx vitest run src/lib/SeasonView.test.ts`
Expected: FAIL — `.new-band` is null; the band does not exist yet.

- [ ] **Step 3: Wire the diff call into the script block**

In `next/src/lib/SeasonView.svelte`, extend the api import on line 4 with `diffSeason` and `FUTURE_SEASON_KEY`, and add `import { partitionNew } from './seasonNew';` below the `seasonUi` import.

Add after the `libraryIds` declaration (around line 57):

```ts
  // Ids flagged as added since the last visit. Held in component state and
  // cleared only when the season, year, or genre changes — never on a timer and
  // never after rendering. Season can be the app's start page, so a launch is
  // itself a visit; clearing on render would consume the flag before it is read.
  let newIds = new Set<number>();
```

Replace `load()` (lines 59-68) with:

```ts
  async function load() {
    loading = true; error = null; newIds = new Set();
    try {
      entries = future
        ? await getFutureAnime(genre || undefined)
        : await getSeasonAnime(season, year, genre || undefined);
      await markSeasonSeen();
    }
    catch(e) { error = e instanceof Error ? e.message : String(e); }
    finally { loading = false; }
  }

  // Best-effort: a failed diff means no band, never a failed page. The grid is
  // the feature; this is a convenience layered over it.
  async function markSeasonSeen() {
    try {
      const key = future ? FUTURE_SEASON_KEY : season;
      const keyYear = future ? 0 : year;
      // A genre-filtered listing holds only part of the season. Recording it
      // would baseline that fragment and mark everything else new next visit.
      const diff = await diffSeason(key, keyYear, entries.map((e) => e.id), genre === '');
      newIds = diff.first_visit ? new Set() : new Set(diff.new_ids);
    } catch {
      newIds = new Set();
    }
  }

  $: ({ fresh: freshEntries, rest: restEntries } = partitionNew(entries, newIds));
```

- [ ] **Step 4: Extract the poster card into its own component**

The band and the main grid both render poster cards. Duplicating the markup across
two `{#each}` loops guarantees the copies drift, so extract it once.

The codebase is on Svelte 5 (`svelte: ^5.25.0`) but consistently written in legacy
syntax — `export let`, `createEventDispatcher`, `on:click`. Match that; do not
introduce runes or snippets in this file.

Create `next/src/lib/SeasonPosterCard.svelte` with the existing card markup moved
verbatim, plus the `isNew` flag:

```svelte
<script lang="ts">
  import type { SeasonAnimeEntry, FutureAnimeEntry } from './api';
  import { createEventDispatcher } from 'svelte';

  export let entry: SeasonAnimeEntry | FutureAnimeEntry;
  export let inLibrary = false;
  export let isNew = false;
  export let future = false;
  export let label = '';

  const dispatch = createEventDispatcher<{
    select: { anime_id: number };
    add: { anime_id: number; title: string };
    quickAdd: { anime_id: number };
  }>();

  function scoreColor(score: number | null | undefined): string {
    if (!score) return 'var(--color-muted)';
    if (score >= 80) return 'var(--color-success)';
    if (score >= 60) return 'var(--color-warning)';
    return 'var(--color-error)';
  }
</script>

<div class="poster-card"
  class:in-library={inLibrary}
  class:is-new={isNew}
  tabindex="0"
  role="button"
  aria-label={entry.title}
  on:click={() => dispatch('select', { anime_id: entry.id })}
  on:contextmenu|preventDefault={() => dispatch('quickAdd', { anime_id: entry.id })}
  on:keydown={(e) => e.key === 'Enter' && dispatch('select', { anime_id: entry.id })}
>
  {#if entry.image_url}
    <img class="poster-img" src={entry.image_url} alt={entry.title} loading="lazy" />
  {:else}
    <div class="poster-img placeholder"></div>
  {/if}
  {#if isNew}
    <span class="new-badge">New</span>
  {:else if inLibrary}
    <span class="in-library-badge">In Library</span>
  {/if}
  {#if !inLibrary}
    <button class="add-btn" on:click|stopPropagation={() => dispatch('add', { anime_id: entry.id, title: entry.title })} aria-label="Add {entry.title} to list">+</button>
  {/if}
  <div class="poster-info">
    <p class="poster-title">{entry.title}</p>
    <div class="poster-meta">
      <span class="poster-format">{entry.format ?? 'TV'}</span>
      {#if future}
        <span class="poster-future">{label}</span>
      {:else if entry.average_score}
        <span class="poster-score" style="color: {scoreColor(entry.average_score)}">{entry.average_score}%</span>
      {/if}
    </div>
  </div>
</div>

<style>
  .poster-card { position: relative; border: 1px solid rgba(var(--color-accent-rgb),0.1); border-radius: 10px; overflow: hidden; background: rgba(255,255,255,0.03); cursor: pointer; transition: border-color 0.15s, transform 0.15s; }
  .poster-card:hover { border-color: rgba(var(--color-accent-rgb),0.3); transform: translateY(-2px); }
  .poster-card.in-library { border-color: rgba(var(--color-success-rgb), 0.55); }
  .poster-card.in-library:hover { border-color: var(--color-success); }
  /* Amber, not the success green: green already means "In Library" on this
     exact card and two unrelated states must not look alike. */
  .poster-card.is-new { border-color: rgba(var(--color-warning-rgb), 0.5); }
  .in-library-badge { position: absolute; top: 0.3rem; left: 0.3rem; font-size: 0.65rem; padding: 0.2rem 0.5rem; border-radius: 999px; background: rgba(var(--color-success-rgb),0.25); color: var(--color-success); font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em; z-index: 1; }
  .new-badge { position: absolute; top: 0.3rem; left: 0.3rem; font-size: 0.65rem; padding: 0.2rem 0.5rem; border-radius: 999px; background: rgba(var(--color-warning-rgb),0.28); color: var(--color-warning); font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em; z-index: 1; }
  .add-btn { position: absolute; top: 0.3rem; right: 0.3rem; border: 1px solid rgba(var(--color-accent-rgb),0.3); border-radius: 4px; padding: 0.1rem 0.4rem; background: rgba(var(--color-accent-rgb),0.15); color: var(--color-accent); cursor: pointer; font-size: 0.85rem; line-height: 1.2; z-index: 1; }
  .add-btn:hover { background: rgba(var(--color-accent-rgb),0.3); }
  .poster-img { width: 100%; aspect-ratio: 3/4; object-fit: cover; display: block; }
  .poster-img.placeholder { background: rgba(var(--color-accent-rgb),0.08); }
  .poster-info { padding: 0.5rem 0.6rem; display: flex; flex-direction: column; gap: 0.25rem; }
  .poster-title { font-size: 0.82rem; font-weight: 600; line-height: 1.3; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .poster-meta { display: flex; gap: 0.5rem; font-size: 0.75rem; align-items: center; }
  .poster-format { color: var(--color-muted); }
  .poster-score { font-weight: 600; }
  .poster-future { font-weight: 600; color: var(--color-accent); }
</style>
```

Then replace the `{:else}` branch of `SeasonView.svelte`'s render block (lines
180-215 — the one opening `<div class="poster-grid">` and iterating `entries`) with:

```svelte
  {:else}
    {#if freshEntries.length > 0}
      <section class="new-band" aria-label="New since your last visit">
        <div class="group-head">
          <span class="group-title">New since your last visit</span>
          <span class="group-count">{freshEntries.length}</span>
        </div>
        <div class="poster-grid">
          {#each freshEntries as entry (entry.id)}
            <SeasonPosterCard
              {entry}
              {future}
              isNew
              inLibrary={libraryIds.has(entry.id)}
              label={labelFor(entry)}
              on:select={(e) => dispatch('select', e.detail)}
              on:add={(e) => handleAddToList(e.detail.anime_id, e.detail.title)}
              on:quickAdd={(e) => handleQuickAdd(e.detail.anime_id)}
            />
          {/each}
        </div>
      </section>
      <div class="rest-head">Rest of the season</div>
    {/if}
    <div class="poster-grid">
      {#each restEntries as entry (entry.id)}
        <SeasonPosterCard
          {entry}
          {future}
          inLibrary={libraryIds.has(entry.id)}
          label={labelFor(entry)}
          on:select={(e) => dispatch('select', e.detail)}
          on:add={(e) => handleAddToList(e.detail.anime_id, e.detail.title)}
          on:quickAdd={(e) => handleQuickAdd(e.detail.anime_id)}
        />
      {/each}
    </div>
  {/if}
```

Import it at the top of `SeasonView.svelte`: `import SeasonPosterCard from './SeasonPosterCard.svelte';`

Delete the now-unused poster-card rules from `SeasonView.svelte`'s `<style>` block (`.poster-card`, `.in-library-badge`, `.add-btn`, `.poster-img`, `.poster-info`, `.poster-title`, `.poster-meta`, `.poster-format`, `.poster-score`, `.poster-future`) — Svelte prunes unused selectors with a warning otherwise, and `npm run check:svelte` runs at zero warnings. Keep `.poster-grid`, `.skeleton-poster`, and everything else.

- [ ] **Step 5: Add the band styles**

Add to `SeasonView.svelte`'s `<style>` block:

```css
  .new-band { border: 1px solid rgba(var(--color-warning-rgb),0.22); border-radius: 12px; background: rgba(var(--color-warning-rgb),0.05); padding: 1rem; display: flex; flex-direction: column; gap: 0.85rem; }
  .group-head { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
  .group-title { font-size: 0.74rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.11em; color: var(--color-warning); }
  .group-count { font-size: 0.7rem; font-weight: 700; color: var(--color-warning); background: rgba(var(--color-warning-rgb),0.16); border-radius: 999px; padding: 0.1rem 0.5rem; font-variant-numeric: tabular-nums; }
  .rest-head { display: flex; align-items: center; gap: 0.75rem; color: var(--color-muted); font-size: 0.74rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.11em; }
  .rest-head::after { content: ""; flex: 1; height: 1px; background: rgba(var(--color-accent-rgb),0.14); }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd next && npx vitest run src/lib/SeasonView.test.ts`
Expected: PASS — 7 tests (3 pre-existing + 4 new).

- [ ] **Step 7: Run the full verification**

Run: `cd next && npm run verify`
Expected: tsc clean, `svelte-check` 0 errors 0 warnings, all vitest tests pass, `cargo check --tests` finishes.

Run: `cd next/src-tauri && CARGO_INCREMENTAL=0 cargo test`
Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add next/src/lib/SeasonView.svelte next/src/lib/SeasonPosterCard.svelte next/src/lib/SeasonView.test.ts
git commit -m "feat: group newly added shows at the top of the Seasons page"
```

---

## Notes for the implementer

- **Do not build or release.** The user cuts builds explicitly; stop after Task 5's commit and report.
- **`SeasonView.test.ts` mocks `./api` wholesale.** Any export the component imports must be added to the mock factory or the component throws at runtime with a confusing error. This bites in Task 5.
- The card extraction in Task 5 is not gold-plating: without it the poster markup would be duplicated across two loops, and the two copies would drift.
- Svelte is `^5.25.0`, but every component here is written in legacy syntax (`export let`, `on:click`, `createEventDispatcher`). Match the surrounding code rather than the newest available API.
