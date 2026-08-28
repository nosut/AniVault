# Automatic Mapping, Library Watcher & Startup Switches — Design

Date: 2026-07-08
Status: Approved

## Problem

1. **Startup toggles are ambiguous.** The Settings → Startup section renders
   "Launch AniVault when Windows starts" and "Start minimized to the system tray" as
   text-pill buttons (`Enabled`/`Disabled`). Whether the current state is on or off is not
   obvious at a glance, unlike the Tracking tab's slider switch.
2. **New episodes require manual work — twice.** Library scans only run when the user
   clicks "Scan Folders" (Settings) or "Rescan" (detail page). Nothing scans on startup, on
   a timer, or on filesystem changes, so a freshly downloaded episode is invisible until a
   manual scan. Worse, a manual mapping teaches the matcher nothing: when episode 2 of a
   manually-mapped series lands in the same folder, `match_file` re-runs the same title
   match that already failed for episode 1, fails again, and the file lands in Unmapped.
   The user has to visit File Management for every new episode of that series.

Goal: a user should hardly ever need the File Management screen. Map a series once (often
not even that); after that, new episodes appear and self-map automatically.

## Goals

- Startup toggles use the same `role="switch"` slider style as "Enable tracking".
- One manual mapping fixes the whole series: subsequent files in the same folder inherit
  the mapping automatically, including files already on disk at mapping time.
- New files are picked up without user action: in near-real-time while the app runs
  (filesystem watcher) and via a startup + hourly scan as catch-up.
- The Library/File Management UI reflects automatic changes without a manual refresh.

## Design

### 1. Startup toggles → switch style

In `SettingsView.svelte`, convert the two Startup `toggle-btn` buttons to the exact
pattern the Tracking tab uses: `role="switch"`, `aria-checked`, existing `.switch` /
`.switch-thumb` classes. Handlers (`handleStartupToggle`, `handleStartInTrayToggle`) are
unchanged. Remove the `.toggle-btn` styles if no other element uses them.

### 2. Folder inheritance in `match_file` (the "episode 2" fix)

In `library_scanner::match_file`, when the best title-match score is **below**
`MATCH_THRESHOLD` (today's failure path), consult the file's directory:

- Query `file_index` for non-ignored rows in the same directory with a non-null
  `anime_id` and `confidence > 0`.
- If **all** such rows point to one single anime, and the filename parsed to an episode
  number (> 0), return that anime at a fixed inherited confidence of **85**.
- Otherwise (no mapped siblings, disagreeing siblings, or no episode number), keep
  today's behavior: leave unmatched at confidence 0.

Safety properties:

- **Unanimity rule:** a mixed downloads folder has files mapped to different anime, so
  nothing inherits there — behavior unchanged.
- **Title match wins:** inheritance only runs when title matching fails, so a movie file
  inside a TV-series folder that confidently matches its own entry keeps that match.
- Since `match_file` is shared by the full scan, targeted rescan, and Re-match command,
  all paths rank identically.

Storage gets one new method: distinct mapped `anime_id`s among non-ignored rows whose
path is directly under a given directory prefix (reusing the `dir_prefix` trailing-
separator guard).

### 3. Manual mapping immediately re-matches siblings

After `set_known_file_mapping` / `set_known_file_mappings` commit, collect the distinct
parent directories of the mapped files and re-run `match_file` + upsert for the
**unmatched** (confidence 0, non-ignored) rows in those directories. With §2 in place,
mapping episode 1 instantly sweeps up episode 2+ already on disk — no scan button.

This runs inline in the command (bounded: only unmatched rows of a few directories) so
the File Management screen reflects the sweep when it reloads after saving.

### 4. Startup + hourly scan

A spawned background task (`spawn_library_scan_worker`, same pattern as
`spawn_sync_worker`) runs `scan_library_folders`:

- once shortly after launch (small delay so startup stays snappy), then
- every 60 minutes.

Skipped entirely when no library folders are configured. Errors are logged, never fatal;
the loop continues.

### 5. Real-time filesystem watcher

Add the `notify` crate (with debouncing). A watcher task:

- Watches each configured library folder recursively.
- Filters events to video extensions (`VIDEO_EXTENSIONS`) plus rename/remove events.
- Debounces per directory (~5 s of quiet) to ride out multi-file moves and in-progress
  downloads; a rename-on-completion (e.g. `.part` → `.mkv`) triggers a fresh event.
- On a debounced batch, runs a **targeted scan of just the affected directories**:
  `index_new_files_in_dir` + prune-under-dir (same offline/root guards as existing scan
  paths — an unreachable folder is skipped, never pruned under).
- Rebuilds watchers when library folders change: `set_library_folders` signals the task
  via a `tokio::sync::watch` channel.
- A folder that fails to watch (offline network drive) is logged and left to the hourly
  scan as fallback; the watcher retries on the next folders-changed signal.

Event→directories mapping lives in a pure function for unit testing; the watcher task
itself stays a thin shell.

### 6. UI stays in sync

When any *automatic* scan (watcher or timer) changes something (`indexed > 0` or
`removed > 0`), publish a new `EngineEvent::LibraryUpdated { indexed, removed }` on the
event bus. The frontend listens and refreshes the Library view and File Management lists
when open. Manual scans keep their existing report-based flow.

## Testing

- **Inheritance rule** (unit, `library_scanner`): unanimous folder inherits at 85; mixed
  folder does not; unmapped-siblings-only does not; missing episode number does not;
  title match ≥ threshold ignores inheritance.
- **Sibling re-match** (unit, command layer): manual map of one file re-matches unmatched
  rows in the same directory; ignored rows untouched; other directories untouched.
- **Watcher logic** (unit): event batch → affected-directories set; extension filter;
  debounce grouping (pure function).
- **Storage** (unit): distinct-mapped-anime-in-dir query respects `ignored`, prefix
  boundaries (`Anime` vs `Anime2`).
- **Toggles**: existing SettingsView behavior unchanged; manual visual check that both
  Startup switches render and flip like the Tracking switch.

## Out of scope

- Title-matcher tuning for the specific series that failed to match (the failing
  filenames weren't available at design time). If inheritance doesn't cover a case,
  revisit with concrete filenames.
- Cross-folder learning (e.g. alias table keyed on parsed title). Folder unanimity is
  deliberately conservative; revisit only if real-world gaps show up.
