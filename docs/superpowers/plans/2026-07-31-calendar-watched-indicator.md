# Calendar Watched Indicator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show, per episode, whether the user has already watched it in the AniVault calendar's month grid and agenda list.

**Architecture:** The Rust backend gains a `watched: bool` on `CalendarEntry`, computed fresh on every calendar read (never trusted from cache) from the union of the local `watch_history` table and `list_entry.watched_episodes`. The Svelte frontend renders it as a green `✓` alongside the existing download dot, with the row dimmed to `opacity: 0.62`. The existing `episodeMarker()` helper is deliberately left alone so `DashboardView` is unaffected.

**Tech Stack:** Rust (Tauri 2, sqlx/SQLite, tokio), Svelte 5 in legacy mode, TypeScript, Vitest + jsdom, cargo test.

## Global Constraints

- Active codebase is `next/`: Svelte frontend in `next/src`, Rust backend in `next/src-tauri`. Nothing outside `next/` changes.
- Full verification gate: `npm run verify` in `next/` (typecheck + vitest + `cargo check --tests`). Rust suite alone: `cargo test` in `next/src-tauri`.
- `episodeMarker()` in `next/src/lib/calendarUi.ts` MUST NOT change semantics. `DashboardView.svelte:167` shares it and is out of scope.
- Scope is the Calendar view only — month grid and agenda list. Do not touch `DashboardView.svelte`.
- "Watched" is per-episode and means: the episode number appears in `watch_history` for that anime, OR `episode <= list_entry.watched_episodes`. Entries with `next_episode: None` are never watched.
- Local status (`has_file`, `watched`) is recomputed on every `get_calendar_inner` call including cache hits; it is never served from `calendar.cache`.
- A failing storage query degrades that show to "not watched" rather than failing the whole calendar load, matching the existing `attach_download_status` error handling.
- Version bump target: `1.0.12` → `1.0.13` in all four files. **Do not build the installer, push, or create a GitHub release** — those are separate steps the user triggers explicitly.

---

### Task 1: `watched` field and the pure `apply_watched` decision

Adds the data model change and the pure function that decides watched state, with no wiring yet. Everything here is unit-testable without a database.

**Files:**
- Modify: `next/src-tauri/src/commands.rs:1443-1454` (struct `CalendarEntry`)
- Modify: `next/src-tauri/src/commands.rs:1517-1524` (add `apply_watched` next to `apply_has_file`)
- Modify: `next/src-tauri/src/commands.rs:1575`, `:1649`, `:1694` (three `CalendarEntry` literals)
- Test: `next/src-tauri/src/commands.rs:3064` (`mod tests`) — builder at `:3068`, new tests near `:3249`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `CalendarEntry.watched: bool` (serde field name `watched`, `#[serde(default)]`)
  - `fn apply_watched(entries: &mut [CalendarEntry], history: &std::collections::HashSet<(i64, i32)>, progress: &std::collections::HashMap<i64, i32>)`

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `next/src-tauri/src/commands.rs`, next to the existing `apply_has_file_marks_only_indexed_episodes` test (around line 3249):

```rust
    #[test]
    fn apply_watched_marks_history_hits_and_episodes_within_progress() {
        let mut entries = vec![
            calendar_entry(10, Some(3)), // 0: play record exists
            calendar_entry(10, Some(4)), // 1: no play record, no list progress
            calendar_entry(20, Some(5)), // 2: exactly at list progress
            calendar_entry(20, Some(6)), // 3: beyond list progress
            calendar_entry(30, Some(1)), // 4: no history and no list entry
            calendar_entry(40, None),    // 5: no episode number to judge
        ];
        let history: std::collections::HashSet<(i64, i32)> = [(10, 3)].into_iter().collect();
        let progress: std::collections::HashMap<i64, i32> = [(20, 5)].into_iter().collect();

        apply_watched(&mut entries, &history, &progress);

        assert!(entries[0].watched, "anime 10 ep 3 has a play record");
        assert!(!entries[1].watched, "anime 10 ep 4 was never played");
        assert!(entries[2].watched, "ep 5 with progress 5 is watched");
        assert!(!entries[3].watched, "ep 6 is past progress 5");
        assert!(!entries[4].watched, "anime 30 has neither history nor a list entry");
        assert!(!entries[5].watched, "entry without an episode number");
    }

    #[test]
    fn calendar_entry_deserializes_a_cache_written_before_the_watched_field() {
        let json = r#"{"anime_id":1,"title":"T","image_url":null,"episode_count":null,
            "progress":null,"next_episode":3,"airing_at":null,"time_until_airing":null,
            "has_file":false}"#;
        let e: CalendarEntry =
            serde_json::from_str(json).expect("pre-existing calendar.cache must still parse");
        assert!(!e.watched, "missing field defaults to not watched");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd next/src-tauri && cargo test apply_watched calendar_entry_deserializes`

Expected: FAIL — compile error, `cannot find function 'apply_watched' in this scope` and `no field 'watched' on type 'CalendarEntry'`.

- [ ] **Step 3: Add the struct field**

In `next/src-tauri/src/commands.rs`, replace the `CalendarEntry` struct (line 1443):

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CalendarEntry {
    pub anime_id: i64,
    pub title: String,
    pub image_url: Option<String>,
    pub episode_count: Option<i32>,
    pub progress: Option<i32>,
    pub next_episode: Option<i32>,
    pub airing_at: Option<i64>,
    pub time_until_airing: Option<i64>,
    pub has_file: bool,
    /// Whether the user has already watched this specific episode. Like
    /// `has_file`, this is recomputed from the local DB on every read and is
    /// never trusted from `calendar.cache` — `serde(default)` only exists so a
    /// cache written before this field still deserializes instead of forcing a
    /// needless refetch.
    #[serde(default)]
    pub watched: bool,
}
```

- [ ] **Step 4: Add `watched: false` to the three existing struct literals**

All three are constructors for freshly fetched entries, before local status is attached. Add `watched: false,` immediately after each `has_file: false,` line at `commands.rs` lines ~1584 (AniList branch), ~1663 (Sonarr branch), and ~1703 (local watching fallback). The fallback literal ends up as:

```rust
            .map(|w| CalendarEntry {
                anime_id: w.anime_id,
                title: w.anime_title,
                image_url: w.image_url,
                episode_count: w.episode_count,
                progress: Some(w.watched_episodes),
                next_episode: None,
                airing_at: None,
                time_until_airing: None,
                has_file: false,
                watched: false,
            })
```

- [ ] **Step 5: Add `watched: false` to the test builder**

In `mod tests`, update `calendar_entry` (line 3068):

```rust
    fn calendar_entry(anime_id: i64, next_episode: Option<i32>) -> CalendarEntry {
        CalendarEntry {
            anime_id,
            title: "Test".to_string(),
            image_url: None,
            episode_count: None,
            progress: None,
            next_episode,
            airing_at: None,
            time_until_airing: None,
            has_file: false,
            watched: false,
        }
    }
```

- [ ] **Step 6: Write `apply_watched`**

In `next/src-tauri/src/commands.rs`, directly below `apply_has_file` (after line 1524):

```rust
/// Mark entries for episodes the user has already watched. Two independent
/// sources: a local play record (covers in-app and out-of-order plays) or list
/// progress having reached the episode (covers anything watched before or
/// outside AniVault and synced from AniList).
fn apply_watched(
    entries: &mut [CalendarEntry],
    history: &std::collections::HashSet<(i64, i32)>,
    progress: &std::collections::HashMap<i64, i32>,
) {
    for e in entries.iter_mut() {
        e.watched = e.next_episode.is_some_and(|ep| {
            history.contains(&(e.anime_id, ep))
                || progress.get(&e.anime_id).is_some_and(|&p| ep <= p)
        });
    }
}
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cd next/src-tauri && cargo test apply_watched calendar_entry_deserializes`

Expected: PASS, 2 tests.

- [ ] **Step 8: Run the full Rust suite**

Run: `cd next/src-tauri && cargo test`

Expected: PASS. `apply_watched` is currently dead code called only from tests; if the build denies `dead_code` this will surface here — it is resolved in Task 2, so a warning is acceptable but a hard error is not. If it errors, leave `#[allow(dead_code)]` off and proceed straight into Task 2 in the same commit rather than suppressing the lint.

- [ ] **Step 9: Commit**

```bash
git add next/src-tauri/src/commands.rs
git commit -m "feat: add watched flag to CalendarEntry with per-episode decision logic"
```

---

### Task 2: Query the local DB and populate `watched`

Wires `apply_watched` to real data and merges the two per-entry local-status passes into one function so the two call sites cannot drift.

**Files:**
- Modify: `next/src-tauri/src/engine/storage.rs:374-382` (add method after `watch_history_count`)
- Modify: `next/src-tauri/src/commands.rs:1493-1515` (`attach_download_status` → `attach_local_status`)
- Modify: `next/src-tauri/src/commands.rs:1538` and `:1712` (both call sites)

**Interfaces:**
- Consumes: `apply_watched(...)` and `CalendarEntry.watched` from Task 1; existing `Storage::get_list_entry(anime_id) -> anyhow::Result<Option<ListEntryRow>>` where `ListEntryRow { anime_id: i64, status: String, watched_episodes: i32 }`; existing `Storage::file_index_by_anime(anime_id)`.
- Produces:
  - `Storage::watch_history_episodes(&self, anime_id: i64) -> anyhow::Result<Vec<i32>>`
  - `async fn attach_local_status(state: &EngineState, entries: &mut [CalendarEntry])` — replaces `attach_download_status`, which no longer exists.

- [ ] **Step 1: Add the storage query**

In `next/src-tauri/src/engine/storage.rs`, immediately after `watch_history_count` (which ends at line 382):

```rust
    /// Distinct episode numbers with at least one play record for this anime.
    /// `watch_history.episode` is NOT NULL, so no null handling is needed.
    pub async fn watch_history_episodes(&self, anime_id: i64) -> anyhow::Result<Vec<i32>> {
        let rows = sqlx::query("SELECT DISTINCT episode FROM watch_history WHERE anime_id = ?1")
            .bind(anime_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(|r| r.get::<i32, _>("episode")).collect())
    }
```

- [ ] **Step 2: Replace `attach_download_status` with `attach_local_status`**

In `next/src-tauri/src/commands.rs`, replace the whole function at lines 1493-1515:

```rust
/// Fill the per-entry local state that is never cached — download status and
/// watched status. Both reflect the local DB at read time, so they are
/// recomputed on every call, including cache hits. A failing query degrades
/// that one show to "no file" / "not watched" rather than failing the calendar.
async fn attach_local_status(state: &EngineState, entries: &mut [CalendarEntry]) {
    let marker_ids: std::collections::HashSet<i64> = entries
        .iter()
        .filter(|e| e.next_episode.is_some())
        .map(|e| e.anime_id)
        .collect();
    let mut have: std::collections::HashSet<(i64, i32)> = std::collections::HashSet::new();
    let mut history: std::collections::HashSet<(i64, i32)> = std::collections::HashSet::new();
    let mut progress: std::collections::HashMap<i64, i32> = std::collections::HashMap::new();
    for id in marker_ids {
        if let Ok(rows) = state.storage.file_index_by_anime(id).await {
            for row in rows {
                if row.ignored {
                    continue;
                }
                if let Some(ep) = row.episode {
                    have.insert((id, ep));
                }
            }
        }
        if let Ok(eps) = state.storage.watch_history_episodes(id).await {
            for ep in eps {
                history.insert((id, ep));
            }
        }
        if let Ok(Some(entry)) = state.storage.get_list_entry(id).await {
            progress.insert(id, entry.watched_episodes);
        }
    }
    apply_has_file(entries, &have);
    apply_watched(entries, &history, &progress);
}
```

- [ ] **Step 3: Update both call sites**

`next/src-tauri/src/commands.rs:1538` (inside the fresh-cache early return):

```rust
            attach_local_status(state, &mut entries).await;
```

`next/src-tauri/src/commands.rs:1712` (just before `Ok(result)`):

```rust
    attach_local_status(state, &mut result).await;
```

- [ ] **Step 4: Verify no stale references remain**

Run: `cd next/src-tauri && grep -rn "attach_download_status" src/`

Expected: no output. If anything matches, update it to `attach_local_status`.

- [ ] **Step 5: Compile and run the Rust suite**

Run: `cd next/src-tauri && cargo test`

Expected: PASS, including the two tests from Task 1. `apply_watched` is now genuinely called, so any dead-code warning from Task 1 is gone.

- [ ] **Step 6: Commit**

```bash
git add next/src-tauri/src/commands.rs next/src-tauri/src/engine/storage.rs
git commit -m "feat: populate calendar watched status from watch history and list progress"
```

---

### Task 3: Frontend type and the `entryLabel` helper

Mirrors the backend field into TypeScript and moves label composition into the testable pure-helper module.

**Files:**
- Modify: `next/src/lib/api.ts:510-520` (`CalendarEntry` interface)
- Modify: `next/src/lib/calendarUi.ts` (add `markerLabels` and `entryLabel`)
- Test: `next/src/lib/calendarUi.test.ts`

**Interfaces:**
- Consumes: `CalendarEntry.watched: bool` from Task 1.
- Produces:
  - `CalendarEntry.watched: boolean` in `api.ts`
  - `export const markerLabels: { have: 'Downloaded'; missing: 'Not downloaded'; future: 'Upcoming' }`
  - `export function entryLabel(entry: { title: string; next_episode: number | null; watched: boolean }, marker: EpisodeMarker): string`

- [ ] **Step 1: Write the failing tests**

Append to `next/src/lib/calendarUi.test.ts`, and extend the import on line 2 to `import { entryLabel, episodeMarker, markerLabels } from './calendarUi';`:

```ts
describe('entryLabel', () => {
  it('names the download state for an unwatched entry', () => {
    expect(
      entryLabel({ title: 'Frieren', next_episode: 7, watched: false }, 'have'),
    ).toBe('Frieren Ep 7 (Downloaded)');
  });

  it('appends "watched" after the download state', () => {
    expect(
      entryLabel({ title: 'Frieren', next_episode: 6, watched: true }, 'have'),
    ).toBe('Frieren Ep 6 (Downloaded, watched)');
  });

  it('keeps watched independent of the download state', () => {
    expect(
      entryLabel({ title: 'Dandadan', next_episode: 9, watched: true }, 'missing'),
    ).toBe('Dandadan Ep 9 (Not downloaded, watched)');
  });

  it('falls back to "?" when the episode number is unknown', () => {
    expect(
      entryLabel({ title: 'Unknown Show', next_episode: null, watched: false }, 'future'),
    ).toBe('Unknown Show Ep ? (Upcoming)');
  });

  it('exposes the marker labels used in the dot tooltip', () => {
    expect(markerLabels.have).toBe('Downloaded');
    expect(markerLabels.missing).toBe('Not downloaded');
    expect(markerLabels.future).toBe('Upcoming');
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd next && npx vitest run src/lib/calendarUi.test.ts`

Expected: FAIL — `entryLabel` and `markerLabels` are not exported from `./calendarUi`.

- [ ] **Step 3: Add the field to the API type**

In `next/src/lib/api.ts`, add to the `CalendarEntry` interface (after `has_file: boolean;` on line 519):

```ts
  watched: boolean;
```

- [ ] **Step 4: Implement the helpers**

Append to `next/src/lib/calendarUi.ts`:

```ts
/// Human-readable name for each download-status marker, used by both the dot's
/// tooltip and the entry's accessible label.
export const markerLabels = {
  have: 'Downloaded',
  missing: 'Not downloaded',
  future: 'Upcoming',
} as const;

/// Accessible label for one calendar entry: title, episode number, download
/// state, and whether it has already been watched. Download and watched are
/// independent facts, so watched is appended rather than replacing the marker.
export function entryLabel(
  entry: { title: string; next_episode: number | null; watched: boolean },
  marker: EpisodeMarker,
): string {
  const ep = entry.next_episode ?? '?';
  const state = entry.watched ? `${markerLabels[marker]}, watched` : markerLabels[marker];
  return `${entry.title} Ep ${ep} (${state})`;
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd next && npx vitest run src/lib/calendarUi.test.ts`

Expected: PASS — the six original `episodeMarker` tests plus the five new ones.

- [ ] **Step 6: Commit**

```bash
git add next/src/lib/api.ts next/src/lib/calendarUi.ts next/src/lib/calendarUi.test.ts
git commit -m "feat: add entryLabel helper and watched field to the calendar entry type"
```

---

### Task 4: Render the watched indicator

The visible change: `✓` plus a dimmed row in the month grid and agenda, and a "Watched" line in the hover tooltip.

**Files:**
- Modify: `next/src/lib/CalendarView.svelte` — import (line 4), delete local `markerLabels` (line 151), month entry (lines 200-218), agenda row (lines 233-254), tooltip (lines 268-273), styles (after line 313 and after line 332)
- Test: `next/src/lib/CalendarView.test.ts`

**Interfaces:**
- Consumes: `entryLabel`, `markerLabels`, `episodeMarker` from `./calendarUi` (Task 3); `CalendarEntry.watched` (Task 3).
- Produces: DOM contract for tests — `.cal-day-entry.watched`, `.cal-day-entry .ep-check`, `.agenda-row.watched`, `.agenda-row .agenda-check`, `.tip-watched`.

- [ ] **Step 1: Write the failing test**

In `next/src/lib/CalendarView.test.ts`, first extend the `weekly` fixture so some episodes are watched. Replace the `out.push({...})` block (lines 13-23) with:

```ts
    out.push({
      anime_id: animeId,
      title,
      image_url: null,
      episode_count: null,
      progress: null,
      next_episode: firstEp + i,
      airing_at: airing,
      time_until_airing: 0,
      has_file: false,
      // Everything up to and including Ep5 counts as watched.
      watched: firstEp + i <= 5,
    });
```

Then add this test inside the `describe('CalendarView month paging')` block:

```ts
  it('marks watched episodes with a check and dims the row', async () => {
    const app = mount(CalendarView, { target: document.getElementById('app')! });
    await settle();

    expect(headerText()).toBe('July 2026');
    // July renders Ep3..Ep7; the fixture marks Ep3, Ep4 and Ep5 as watched.
    expect(document.querySelectorAll('.cal-day-entry.watched')).toHaveLength(3);
    expect(document.querySelectorAll('.cal-day-entry .ep-check')).toHaveLength(3);

    // Ep3 aired 2026-07-03 with no local file: not downloaded, but watched.
    const first = document.querySelector('.cal-day-entry')!;
    expect(first.getAttribute('aria-label')).toBe('Alpha Show Ep 3 (Not downloaded, watched)');

    await unmount(app);
  });
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd next && npx vitest run src/lib/CalendarView.test.ts`

Expected: FAIL — `expected length 3, received 0`; nothing renders `.watched` or `.ep-check` yet.

- [ ] **Step 3: Update the imports and drop the local `markerLabels`**

In `next/src/lib/CalendarView.svelte`, change line 4 to:

```ts
  import { episodeMarker, entryLabel, markerLabels } from './calendarUi';
```

and delete line 151 entirely (`const markerLabels = { have: 'Downloaded', ... } as const;`) — it now comes from the shared module.

- [ ] **Step 4: Render the month-grid indicator**

Replace the month entry block (lines 200-218) with:

```svelte
            <div
              class="cal-day-entry"
              class:watched={entry.watched}
              tabindex="0"
              role="button"
              aria-label={entryLabel(entry, marker)}
              on:click={() => selectEntry(entry)}
              on:keydown={(e) => e.key === 'Enter' && selectEntry(entry)}
              on:mouseenter={(e) => placeTip(entry, e.clientX, e.clientY)}
              on:mousemove={(e) => placeTip(entry, e.clientX, e.clientY)}
              on:mouseleave={hideTip}
              on:focus={(e) => showTipAt(entry, e.currentTarget)}
              on:blur={hideTip}
            >
              <span class="ep-dot {marker}" title={markerLabels[marker]}></span>
              <span class="cal-entry-title">{entry.title}</span>
              {#if entry.next_episode}
                <span class="cal-entry-ep">Ep{entry.next_episode}</span>
              {/if}
              {#if entry.watched}
                <span class="ep-check" aria-hidden="true">✓</span>
              {/if}
            </div>
```

The `✓` is `aria-hidden` because `entryLabel` already carries the state.

- [ ] **Step 5: Render the agenda indicator**

In the agenda row, add `class:watched={e.watched}` to the opening `<button class="agenda-row"` tag (line 233), and insert the check immediately before the countdown `<span>` (line 251):

```svelte
                {#if e.watched}
                  <span class="agenda-check" aria-hidden="true">✓</span>
                {/if}
                <span class="agenda-countdown" class:soon={isSoon(e)} class:aired={e.airing_at != null && e.airing_at <= now}>
                  {countdownLabel(e)}
                </span>
```

- [ ] **Step 6: Add the tooltip line**

In the tooltip body, immediately after the `<p class="tip-meta">…</p>` block (ends line 273):

```svelte
      {#if tip.entry.watched}
        <p class="tip-watched">✓ Watched</p>
      {/if}
```

- [ ] **Step 7: Add the styles**

In the `<style>` block, after the `.ep-dot.future` rule (line 313):

```css
  .ep-check { flex-shrink: 0; color: var(--color-success); font-size: 0.62rem; line-height: 1; margin-left: 0.15rem; }
  .cal-day-entry.watched { opacity: 0.62; }
  /* A dimmed title must still be readable when the user goes looking for it. */
  .cal-day-entry.watched:hover, .cal-day-entry.watched:focus { opacity: 1; }
```

and after the `.agenda-countdown.aired` rule (line 332):

```css
  .agenda-check { flex-shrink: 0; color: var(--color-success); font-size: 0.85rem; line-height: 1; }
  .agenda-row.watched { opacity: 0.62; }
  .agenda-row.watched:hover, .agenda-row.watched:focus { opacity: 1; }
  .tip-watched { font-size: 0.78rem; color: var(--color-success); font-weight: 600; }
```

Note the `≤900px` breakpoint already hides `.cal-entry-title`; `.ep-check` is intentionally left visible so narrow windows still show dot + `✓` + episode number.

- [ ] **Step 8: Run the component tests to verify they pass**

Run: `cd next && npx vitest run src/lib/CalendarView.test.ts`

Expected: PASS — the two existing paging tests plus the new watched test.

- [ ] **Step 9: Run the full verification gate**

Run: `cd next && npm run verify`

Expected: PASS — typecheck, full vitest suite, and `cargo check --tests` all clean. `svelte-check` will fail if any `CalendarEntry` literal elsewhere in the frontend is missing the new `watched` field; fix any such literal by adding `watched: false`.

- [ ] **Step 10: Commit**

```bash
git add next/src/lib/CalendarView.svelte next/src/lib/CalendarView.test.ts
git commit -m "feat: show a watched check and dimmed row in the calendar"
```

---

### Task 5: Version bump to 1.0.13

Per `CLAUDE.md`, a user-facing change ships with a patch bump. This task bumps versions only — it does not build, push, or release.

**Files:**
- Modify: `next/package.json`, `next/package-lock.json` (via npm)
- Modify: `next/src-tauri/Cargo.toml`
- Modify: `next/src-tauri/tauri.conf.json`
- Modify: `next/src-tauri/Cargo.lock` (regenerated)

**Interfaces:**
- Consumes: a complete, verified feature from Tasks 1-4.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Bump the npm version**

Run: `cd next && npm version 1.0.13 --no-git-tag-version`

Expected: updates `package.json` and `package-lock.json` to `1.0.13`.

- [ ] **Step 2: Bump the Cargo version**

In `next/src-tauri/Cargo.toml`, change the package version line:

```toml
version = "1.0.13"
```

- [ ] **Step 3: Bump the Tauri config version**

In `next/src-tauri/tauri.conf.json`, change:

```json
  "version": "1.0.13",
```

- [ ] **Step 4: Refresh `Cargo.lock`**

Run: `cd next/src-tauri && cargo check --tests`

Expected: PASS, and `Cargo.lock` now records `anivault 1.0.13`.

- [ ] **Step 5: Verify all four versions match**

Run:

```bash
cd next && node -p "require('./package.json').version" \
  && grep -m1 '^version' src-tauri/Cargo.toml \
  && grep -m1 '"version"' src-tauri/tauri.conf.json \
  && grep -m1 -A1 'name = "anivault"' src-tauri/Cargo.lock
```

Expected: `1.0.13` in all four.

- [ ] **Step 6: Commit**

```bash
git add next/package.json next/package-lock.json next/src-tauri/Cargo.toml \
  next/src-tauri/tauri.conf.json next/src-tauri/Cargo.lock
git commit -m "chore: release 1.0.13"
```

- [ ] **Step 7: Stop and report**

Do NOT run `npm run bundle`, do NOT push, do NOT run `gh release create`. Report to the user that the feature and version bump are committed and ask whether they want a build.

---

## Self-Review

**Spec coverage:**

| Spec section | Task |
|---|---|
| Decision 1 — per-episode granularity | Task 1 (`apply_watched` keys on `next_episode`) |
| Decision 2 — history ∪ list progress | Task 1 (logic), Task 2 (both queries) |
| Decision 3 — Calendar only, Dashboard untouched | Task 3/4 (`episodeMarker` unchanged, `DashboardView` not in any file list) |
| Decision 4 — additive `✓` + dim, dot retained | Task 4 |
| `watched` field + `#[serde(default)]` | Task 1 |
| Never trusted from cache | Task 2 (both call sites) |
| `attach_local_status` merge | Task 2 |
| `watch_history_episodes` + `get_list_entry` | Task 2 |
| Error degrades to "not watched" | Task 2 (`if let Ok(...)` guards) |
| `markerLabels` move + `entryLabel` | Task 3 |
| Month grid, agenda, tooltip, styles, hover restore | Task 4 |
| ≤900px breakpoint behaviour | Task 4 Step 7 note |
| Rust tests (5 watched cases + builder) | Task 1 Step 1 |
| Vitest `entryLabel` and component tests | Task 3, Task 4 |
| `npm run verify` gate | Task 4 Step 9 |
| Patch version bump, no build/release | Task 5 |

No gaps.

**Placeholder scan:** No TBD/TODO, no "add error handling" hand-waves, no "similar to Task N" references. Every code step carries literal code.

**Type consistency:** `watched` is the field name in Rust (`bool`) and TypeScript (`boolean`) and the CSS class name. `apply_watched(entries, history, progress)` is declared in Task 1 and called with exactly those three arguments in Task 2. `attach_local_status` is defined and both call sites updated in Task 2, with a grep step confirming `attach_download_status` is fully gone. `entryLabel` and `markerLabels` are exported in Task 3 and imported by that exact name in Task 4. `ListEntryRow.watched_episodes` matches `storage.rs:16`.
