# Taiga Next Modernization Design

Date: 2026-06-29

## Goal

Build a clean-room successor to Taiga for personal anime management on Windows. The new app should keep Taiga's core value—automatic watch detection, anime recognition, progress updates, and tracker sync—while replacing the dated Qt Widgets UI with a premium dark immersive media-library experience.

Taiga is reference material only. The new app should not refactor the existing UI or preserve legacy architecture.

## Decisions

- Product direction: clean-room rewrite, not in-place refactor.
- Target platform: Windows only.
- User model: single local profile for one Windows user.
- Source of truth: local-first SQLite database.
- UX style: premium dark media app with poster art, restrained depth, and smooth interactions.
- MVP scope: include tracking, library, and tracker flows, but deliver watch detection → auto-match → progress update → sync first.
- Old torrent/RSS section: not carried forward.
- Sonarr: action-capable integration first, designed for fuller coordination later.

## Technology Stack

- App shell: Tauri v2.
- Frontend: Svelte + TypeScript.
- Engine: Rust.
- Storage: SQLite with WAL mode.
- Secrets: Windows DPAPI for tracker and Sonarr credentials.
- Windows integration: Rust Windows APIs for media session, process, file, tray, and notification behavior.

This stack is chosen because it gives modern web-quality UI/theming without Electron's footprint, while Rust keeps the always-running background engine efficient and reliable.

## Core Architecture

The engine must be headless and event-driven. UI consumes state and events through a narrow Tauri IPC API. Tracking, sync, recognition, storage, and migration must not depend on UI code.

### Modules

1. **Detection**
   - Watches active media players, Windows media sessions, process/window state, and file paths.
   - Emits `MediaDetected` events.

2. **Recognition**
   - Parses filenames and titles.
   - Matches against the local title index.
   - Emits `AnimeIdentified` with confidence score and evidence.

3. **Library**
   - Owns anime metadata, user list entries, watch history, file index, and local settings.
   - Writes local progress before any remote sync attempt.

4. **Sync**
   - Owns tracker auth, remote adapters, service rate limits, retry policy, and conflicts.
   - Starts with AniList, then adds MyAnimeList and Kitsu.

5. **Integrations**
   - Owns external tools such as Sonarr.
   - Sonarr adapter can link anime to Sonarr series, read monitored/download status, and request searches/downloads.

6. **Orchestrator**
   - Routes events between modules.
   - Contains coordination logic only; no heavy domain rules.

7. **Migration**
   - Imports current Taiga data without mutating the old installation.
   - Produces a migration report for skipped or ambiguous data.

8. **UI**
   - Svelte views over engine commands/events.
   - Never writes SQLite directly.

## Event Flow

Primary MVP flow:

1. Detection observes media activity.
2. Detection emits `MediaDetected`.
3. Recognition parses and matches the item.
4. Recognition emits `AnimeIdentified`.
5. Orchestrator validates confidence threshold.
6. Library updates progress and appends watch history.
7. Sync appends remote update to `sync_queue`.
8. Sync worker pushes to enabled tracker services.
9. UI shows success, pending retry, or conflict state.

Low-confidence matches ask for confirmation before changing progress. Every automatic progress change must be visible in recent activity and undoable.

## Storage Design

SQLite is the local source of truth. WAL mode is enabled. Schema changes are versioned through migrations.

Core tables:

- `anime` — local anime metadata and title variants.
- `list_entry` — local user state: status, progress, score, dates, notes.
- `watch_history` — append-only watched episode history.
- `tracker_mapping` — local anime ID to AniList/MAL/Kitsu IDs.
- `file_index` — scanned local media files and recognition results.
- `sync_queue` — durable pending remote operations.
- `settings` — local app preferences and integration configuration references.
- `migration_log` — imported Taiga records, skipped records, and warnings.
- `sonarr_mapping` — local anime ID to Sonarr series/season metadata.
- `integration_queue` — durable Sonarr and future integration actions.

Remote IDs should be stored as text so integer IDs and UUIDs can share one mapping model.

## Sync Reliability

Progress changes are local-first. Remote sync is asynchronous and durable.

- Local DB update happens first.
- Sync operation is appended to `sync_queue`.
- Worker retries with exponential backoff.
- Per-service throttling prevents rate-limit issues.
- Network failures do not roll back local progress.
- OAuth tokens are encrypted with DPAPI.
- Sync errors surface in the UI with retry and dismiss controls.

Conflict handling:

1. Local-only change: push local.
2. Remote-only change: pull remote.
3. Both changed: show conflict UI with local, remote, and merged choices.

## Sonarr Integration

The old Taiga torrent/RSS workflow is not part of the rewrite. Sonarr replaces that role.

Initial Sonarr scope:

- Connect using Sonarr URL and API key.
- Store credentials securely.
- Search and link local anime to Sonarr series.
- Display monitored status, download status, and missing episode state.
- Trigger Sonarr search/download actions from Taiga Next.
- Queue Sonarr actions with retries and visible error states.

Future full integration:

- Coordinate library folders.
- Coordinate monitored state.
- Understand Sonarr download pipeline status.
- Power “next episode missing” and “download next” workflows.
- Avoid duplicate work when Sonarr already manages files.

## UI/UX Design

Dark mode is the default and foundational, not a later stylesheet patch. The UI uses design tokens for color, spacing, radius, shadows, typography, and motion.

Visual direction:

- Premium dark media app.
- Deep dark background.
- Poster-rich cards and detail pages.
- Soft depth and restrained glass/card effects.
- Smooth but not excessive animation.
- High readability for long lists and status-heavy screens.

Main navigation:

- Home
- Library
- Watching
- Calendar / Seasonal
- Sync
- Integrations
- Settings

MVP screens:

- Home dashboard with continue watching, now detected, sync health, recently added, and upcoming episodes.
- Now Playing confirmation card.
- Anime detail page.
- Library grid/list.
- Sync queue/status page.
- Sonarr integration page.
- Migration report.
- Settings for folders, trackers, detection behavior, and integrations.

## Migration

Migration is required. The importer must read existing Taiga data and write the new schema without changing the old data.

Import targets:

- User list entries.
- Known tracker IDs.
- Watch history.
- Library folders.
- Relevant preferences.
- Local anime metadata where useful.

Ambiguous or failed records are skipped with a clear report. Users should be able to rerun migration on a fresh new database.

## Phased Delivery

### Phase 0: Foundation and Migration

- Tauri/Svelte/Rust scaffold.
- SQLite schema and migrations.
- Event bus and command API.
- DPAPI secret storage.
- Taiga importer.
- Test harness for storage, migration, and event flow.

Deliverable: non-user-facing foundation that can import data and pass tests.

### Phase A: Core Tracking MVP

- Media/player detection.
- Filename/title recognition.
- Confidence and confirmation flow.
- Local progress and history updates.
- AniList sync first.
- Sync queue and retry UI.
- Tray behavior.
- Now Playing card.

Deliverable: replacement for Taiga's core watch/update loop.

### Phase B: Immersive Library and Sonarr Actions

- Library scan and file index.
- Poster-rich home, library, and anime detail pages.
- Sonarr connect/link/status/search/download.
- Play-next and missing-episode UX.

Deliverable: modern local library experience plus action-capable Sonarr integration.

### Phase C: Tracker Parity and Full Sonarr Planning

- MyAnimeList adapter.
- Kitsu adapter.
- Seasonal/search/discovery views.
- Conflict resolution UI.
- Design full Sonarr coordination workflows.

Deliverable: tracker parity and prepared plan for deeper Sonarr coordination.

### Phase D: Polish and Expansion

- Analytics and watching insights.
- Richer motion and visual polish.
- Optional light mode.
- Additional integrations or adapter/plugin model after APIs stabilize.

## Testing Strategy

- Unit tests for recognition parsing, storage migrations, sync queue behavior, and conflict logic.
- Integration tests for migration from sample Taiga databases.
- Mock tracker services for sync retry and conflict scenarios.
- Mock Sonarr API for link/search/download actions.
- UI tests for critical flows: migration, now playing confirmation, progress undo, sync errors, Sonarr action errors.

## Risks and Mitigations

- **Data loss during migration**: importer never mutates old Taiga data; migration report required.
- **Recognition regression**: preserve behavior with fixture tests from real filenames.
- **Tracker rate limits**: durable queue with per-service throttling.
- **Sync conflicts**: explicit conflict UI instead of silent overwrite.
- **WebView2 issues**: installer should ensure WebView2 runtime exists and provide clear launch diagnostics.
- **Sonarr actions causing surprise downloads**: action confirmation, queue visibility, and clear error/status feedback.
- **UI/engine coupling**: enforce command/event boundary; UI cannot access SQLite directly.

## Out of Scope Initially

- Built-in torrent/RSS client.
- Multi-user local profiles.
- Cross-platform support.
- Public plugin ecosystem before core APIs stabilize.
- Full Sonarr folder/monitored-state coordination before Phase C planning.
