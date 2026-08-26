# AniVault Phase A — Core Tracking MVP Design

Date: 2026-06-30

## Goal

Replace Taiga's core watch/update loop: detect what you're watching, recognize the anime, update local progress, and sync to AniList — all silently in the background.

## Scope

### In scope
- Media player detection (process, file system, Windows media session)
- Filename-to-anime recognition with local title index + AniList fallback
- Auto-add new anime to library on first detection
- Silent local progress updates (no notifications, no popups)
- AniList sync with durable queue and retry
- System tray with minimize-to-tray
- Now Playing card on home screen
- Sync status indicator

### Out of scope
- Manual episode editing or progress undo (future phase)
- Low-confidence confirmation UI (future phase)
- MyAnimeList or Kitsu sync (Phase C)
- Library browsing with posters (Phase B)
- Sonarr integration (Phase B)
- Seasonal/calendar views (Phase C)

## Detection

Three parallel Windows watchers run as Tokio tasks:

1. **Process watcher** (poll every 2s): scans for known player processes (mpv, MPC, VLC, PotPlayer, Media Player), reads window title via `GetWindowTextW`, emits candidate on change.

2. **File system watcher** (event-driven via `ReadDirectoryChangesW`): watches configured folders (default `D:\Anime`), emits file path on `.mkv`/`.mp4`/`.avi` modification. Debounce: same file within 10s = same detection.

3. **Media session listener** (Windows Runtime `SystemMediaTransportControls`): emits now-playing metadata for UWP apps and modern players.

**Dedup:** prefer file path over process/window title. Same file within 60s from multiple sources → emit once.

**Config** (SQLite `settings` table): `detection.folders` (JSON array), `detection.poll_interval_ms` (default 2000).

## Recognition

Pure-function filename parser + database-backed title matcher.

**Parser** cleans and extracts from filenames:
- Strip fansub group tags, resolution tags, codec tags
- Extract episode number (trailing integer, E##, Ep ##, ## patterns)
- Extract season (S##, Season ## patterns)
- Remaining text = candidate title

**Local matcher** uses SQLite FTS5:
- Index built from `anime.titles_json` (romaji, english, synonyms)
- Exact romaji match → confidence 100
- Synonym match → confidence 85
- FTS5 fuzzy match → confidence 70-99
- Score ≥ 70 → return match

**AniList fallback** when no local match:
- GraphQL `Media(search:)` query
- Exact title match → auto-insert into local DB, high confidence
- Multiple partial matches → top candidate, lowered confidence 50-60
- Rate limited to 30 searches/min

**Confidence thresholds:**
- ≥ 85: auto-accept, silent tracking
- 50-84: auto-accept, flagged for future review
- < 50: skip, log unrecognized

## Orchestrator

Coordination hub with no domain logic:
1. Receives `MediaDetected` from detection → passes to recognition
2. On `AnimeIdentified` with high confidence → calls storage (update local progress) → queues sync
3. On low confidence → emits event for future UI confirmation

## Sync

AniList adapter over GraphQL. Token stored via DPAPI.

**Sync queue** (reuses existing `sync_queue` table): push progress updates on every `ProgressAdvanced`. Worker picks up pending rows, executes against AniList, deletes on success.

**Backoff:** 30s → 2min → 10min → 1hr → 6hr (permanent retry).

**Rate limiting:** token bucket, max 80 req/min (leaves headroom for recognition searches).

**Tauri commands:** `get_sync_status()`, `retry_failed_sync()`, `clear_sync_errors()`.

## UI

Minimal additions to existing dark shell:

- **System tray:** icon from `icon.ico`, right-click "Show AniVault" / "Quit", minimize-to-tray, engine runs in background when window hidden.
- **Now Playing card:** home screen component showing current anime + episode + confidence. Appears on detection, fades after 5s inactivity.
- **Sync status indicator:** small dot in sidebar rail (green/amber/red). Click for pending/failed counts.

## Storage additions

New columns/app settings needed:
- `settings` entries for detection config (folders, poll interval)
- `settings` entry for AniList OAuth token (DPAPI-encrypted)
- FTS5 virtual table for anime title search index

Existing tables (`anime`, `watch_history`, `sync_queue`, `list_entry`) sufficient — no schema migration needed.

## Verification

- Unit tests: filename parser with 20+ real-world filenames (fixtures from Taiga's recognition tests), FTS5 match accuracy, dedup logic
- Integration tests: mock AniList GraphQL, mock Windows detection APIs, sync retry + backoff
- End-to-end: start AniVault, play an anime file → watch history entry created → sync queued
- `npm run verify` must pass

## Risks

- **Windows Runtime APIs require UI thread:** media session listener may need Tauri window handle. Fall back to process watcher if unavailable.
- **AniList rate limits:** aggressive caching + token bucket prevent bans. FTS5 reduces API calls.
- **FTS5 build on first run:** index population takes ~100ms for typical library size (<5000 titles). Acceptable cold start cost.
- **Existing Taiga recognition regression:** port and test against fixture files from old codebase.

## Files changed

```
next/src-tauri/
  src/
    engine/
      detection/mod.rs      # Process + file + media session watchers
      detection/process.rs   # Process enumeration + window title
      detection/fs.rs        # ReadDirectoryChangesW wrapper
      detection/session.rs   # SystemMediaTransportControls listener
      recognition/mod.rs     # Orchestrates parse → local match → AniList fallback
      recognition/parser.rs  # Filename/title cleanup + extraction
      recognition/matcher.rs # FTS5 + AniList GraphQL search
      orchestrator/mod.rs    # Event routing
      sync/mod.rs            # SyncManager
      sync/anilist.rs        # AniList GraphQL adapter
      sync/queue.rs          # Durable sync queue operations
      sync/worker.rs         # Tokio worker loop
      mod.rs                 # Module exports
    commands.rs              # New Tauri commands
    lib.rs                   # Updated with new commands + engine startup
  migrations/
    0002_fts5_index.sql      # FTS5 virtual table

next/src/
  lib/
    tray.ts                  # Tauri tray setup
    now-playing.svelte       # Now Playing card
    sync-status.svelte       # Sync health indicator
  App.svelte                 # Updated home layout integrating new components
```
