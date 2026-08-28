# File Lifecycle, Targeted Rescan & Self-Healing Startup — Design

Date: 2026-07-04
Status: Approved

## Problem

Three related gaps surfaced during a codebase review:

1. **Deleted/moved files linger forever.** `scan_library_folders` only *adds* rows to
   `file_index`; it never removes entries for files that no longer exist on disk. Deleted
   or renamed episodes stay in the DB and keep showing in episode lists / File Manager.
2. **Detail-page "Rescan" doesn't recheck disk.** `DetailView.handleRescanFiles` runs a
   full library scan and reloads, but the scan skips already-indexed files and never
   prunes, so it does not reflect files added to or removed from the show's folders.
3. **Launch-on-startup silently fails.** The `HKCU\...\Run` entry is written with the exe
   path *at toggle time*. After a reinstall, path change, or bundle-identifier change, the
   entry points at a stale/missing exe (or is absent) and nothing repairs it. Diagnosis on
   the dev machine: no `AniVault` Run entry exists and `startup.launch_on_startup` was never
   persisted; the app has no self-check.

## Goals

- Files added, deleted, moved, or renamed are reflected in `file_index` after a scan.
- The detail-page Rescan re-reads the specific show's folders from disk (fast), and falls
  back to a full library scan when the show has no indexed files yet.
- Launch-on-startup is self-healing: it reconciles the registry to the persisted setting on
  every launch and always points at the current exe.

## Design

### 1. Prune deleted files during scan

In `scan_library_folders`, after walking a configured library folder that **exists and was
readable**, delete `file_index` rows whose `file_path` is under that folder but was not seen
in the current walk (i.e. the file is gone from disk).

- **Offline guard:** only prune within folders that currently `exists()` and were scanned
  without a read error. An offline network drive (e.g. `Y:\Anime`) is skipped entirely for
  both indexing and pruning, so it never wipes the index.
- **Scope guard:** only rows whose path starts with a scanned folder are eligible; rows
  belonging to other (offline) folders are untouched.
- **Tombstones:** `ignored` rows for gone files are pruned too — no reason to keep a
  tombstone for a file that no longer exists.
- **Watch history** is keyed by `anime_id`, not file path, so pruning a file row never
  affects history.
- The scan report gains a `removed` count alongside `found` / `indexed` / `skipped`.

Storage gets one new method: given a scanned folder prefix and the set of paths seen this
scan, delete rows under that prefix not in the set.

### 2. Targeted rescan for a single anime

New backend command `rescan_anime_files(anime_id)`:

1. Look up the anime's current `file_index` rows; collect the distinct parent directories.
2. For each directory that still exists, walk it, re-`match_file` its videos, upsert, and
   prune rows under it that vanished — the same add/prune logic as the full scan, scoped to
   those directories.
3. If the anime has **no** indexed files (nothing to derive a folder from), fall back to a
   full `scan_library_folders`.
4. Return the updated episode-file list for the anime.

`DetailView.handleRescanFiles` calls `rescanAnimeFiles(animeId)` instead of
`scanLibraryFolders()`, then refreshes.

### 3. Self-healing launch-on-startup

Extract the registry-value decision into a pure, testable function
`desired_run_value(enabled, start_in_tray, exe_path) -> Option<String>` (`None` = key should
be absent). On app launch (in `setup`, after the engine is up), read the persisted
`startup.launch_on_startup` + `startup.start_in_tray` and reconcile:

- enabled → write/refresh the `AniVault` Run value to the current exe path (with
  `--minimized` when start-in-tray is on).
- disabled → ensure the value is removed.

This repairs stale paths from reinstalls automatically and makes the toggle's effect
durable. `apply_startup_registry` is refactored to build its value via `desired_run_value`
so the toggle path and the launch-time reconcile share one source of truth.

Out of scope: the pre-existing unrelated `Taiga` Run entry (a different application) is left
alone. The orphaned old app-data dir (`%APPDATA%\AniVault\`) is flagged to the user, not
auto-deleted.

## Testing

- **Prune:** scan a temp dir, delete a file, rescan → row removed, others intact; offline
  folder (nonexistent path) → no rows pruned.
- **Targeted rescan:** anime with files in a temp dir picks up a new file and drops a deleted
  one; anime with no files falls back to full scan.
- **Startup:** `desired_run_value` truth table (enabled/disabled × tray on/off) returns the
  expected quoted value or `None`.

## Cleanup (bundled)

- Delete stale `HANDOFF.md`, scratch screenshots `calendar.jpg` / `secret.jpg`.
- Fix two compiler warnings: unused `sqlx::Row` import (`v1_read.rs`), dead-code
  `Storage::pool()`.
- Keep `Banner.png` / `Icon.png` / `Icon.ico` — documented installer source assets.
- Keep unmerged `.worktrees/anivault-installer` (`feat/anivault-installer` has un-merged work).
- **Not pruned:** `.worktrees/m4-library-ui`. Although its branch is merged into
  `develop`, the worktree holds *uncommitted* modifications (commands.rs, storage.rs,
  lib.rs, api.ts) and untracked files that exist nowhere else. Removing it would lose
  that state irrecoverably, so it is preserved and flagged for the user to decide.
