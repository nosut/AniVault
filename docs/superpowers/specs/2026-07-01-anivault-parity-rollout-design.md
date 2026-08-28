# AniVault Parity Rollout Design

## Purpose

AniVault will reach parity as a Windows desktop anime tracker without MAL or Kitsu support. Parity means local playback detection, anime recognition, AniList sync, Sonarr integration, daily-use library UI, tray/background behavior, old Taiga data migration, installer/rebrand, and release hardening.

This design replaces any earlier assumption that MAL or Kitsu tracking are required. AniList is the only tracker integration in scope.

## Current Baseline

The codebase is at Phase 0 foundation:

- Tauri/Svelte shell exists in `next/`.
- Rust modules exist for storage, secrets, migration, events, and models.
- SQLite schema includes future-facing tables.
- Tauri command surface has only stub health and migration commands.
- Storage, event bus, workers, and frontend event delivery are not wired into runtime.
- UI is a static shell with inert navigation.
- Rebrand and installer work are not implemented.

## Scope

### In scope

- Windows desktop app runtime using Tauri, Svelte, Rust, SQLite.
- Local media detection and playback tracking.
- Filename/title recognition with manual correction.
- AniList authentication, list import, and progress sync.
- Sonarr connection, series mapping, and availability/file linking.
- Library, details, settings, sync status, and diagnostics UI.
- Tray/background operation.
- Taiga v1 migration and data-safety flows.
- AniVault branding and installer.

### Out of scope

- MAL tracking.
- Kitsu tracking.
- Generic multi-tracker conflict UI.
- Service adapter abstractions unless they directly simplify AniList or Sonarr.

Existing `tracker_mapping.service` schema may remain for future-proofing, but implementation treats AniList as the only tracker.

## Rollout Strategy

Use milestone-gated vertical delivery. Each milestone must produce a runnable app state with tests and acceptance criteria met before the next milestone starts.

### M0 — Runtime Foundation

Goal: turn the scaffold into a real runtime.

Deliverables:

- Initialize SQLite on app startup.
- Run migrations automatically.
- Manage `Storage` as Tauri state.
- Replace hardcoded command stubs with real state-backed commands.
- Add settings read/write commands.
- Wire event bus into backend state.
- Provide backend-to-frontend event delivery path.
- Add command integration tests.

Acceptance criteria:

- App boot creates or opens the database.
- `get_engine_status` reports real DB and migration state.
- Frontend displays real engine status.
- Settings can be read and written through commands.
- Tests prove commands use real runtime state.

### M1 — Local Tracking MVP

Goal: detect playback and record watch history locally.

Deliverables:

- Windows process/window scanner.
- Supported media-player registry.
- Active file/title detection.
- Watch-session lifecycle.
- Playback threshold rules.
- Local progress and watch-history writes.
- Now Playing UI.
- Manual mark-watched fallback.

Acceptance criteria:

- Playing an episode creates local watch history.
- Progress persists across restart.
- User can override or cancel a detection.

### M2 — Recognition Engine

Goal: match detected files or titles to anime and episodes.

Deliverables:

- Filename parser.
- Release-group, quality, codec, and tag cleanup.
- Episode number extraction.
- Candidate search against local library/list.
- Confidence scoring.
- Manual correction UI.
- Persisted file/anime mappings.
- Parser test corpus.

Acceptance criteria:

- Common anime filenames are recognized.
- Low-confidence matches require confirmation.
- User corrections persist and improve future matches.

### M3 — AniList Integration

Goal: make AniList the single tracker sync target.

Deliverables:

- AniList authentication.
- DPAPI token storage.
- AniList GraphQL client.
- List import.
- Progress sync.
- Sync queue worker.
- Retry and backoff behavior.
- Local-vs-AniList conflict handling.
- Sync status UI.

Acceptance criteria:

- User connects AniList.
- App imports AniList library.
- Local tracked progress syncs to AniList.
- Failed syncs are visible and retryable.

### M4 — Library and App UI

Goal: make AniVault usable as a daily app.

Deliverables:

- Dashboard.
- Library view.
- Anime detail view.
- Progress editing.
- Search and filtering.
- Settings UI.
- Logs/errors UI.
- Empty, loading, and error states.

Acceptance criteria:

- All navigation items open real views.
- User can browse and edit local library without leaving the app.
- User can inspect sync and tracking status.

### M5 — Tray and Background Behavior

Goal: match expected desktop tracker behavior.

Deliverables:

- Tray icon and menu.
- Minimize-to-tray.
- Background tracking.
- Launch-on-startup setting.
- Notifications.
- Pause/resume tracking.
- Clear quit semantics.

Acceptance criteria:

- App tracks while hidden.
- User can control tracking from tray and settings.
- Quit/minimize behavior is predictable.

### M6 — Sonarr Integration

Goal: connect AniVault to the user's media management workflow.

Deliverables:

- Sonarr server settings.
- API key validation.
- Series import.
- Anime-to-Sonarr mapping.
- Episode availability display.
- File path linking.
- Manual remap UI.

Acceptance criteria:

- User connects Sonarr.
- App maps Sonarr series to anime entries.
- Library shows availability and local file/source data.

### M7 — Migration and Data Safety

Goal: support old data and prevent silent data loss.

Deliverables:

- Taiga v1 import dry run.
- Import warnings and summary.
- Duplicate handling.
- Backup/export/import.
- Migration tests.

Acceptance criteria:

- User can preview import before applying.
- Import does not silently drop meaningful data.
- User can recover from bad import or DB failure using backup/export flows.

### M8 — Rebrand and Installer

Goal: ship as AniVault.

Deliverables:

- Rename package, Cargo crate, Tauri product name, and identifiers.
- Integrate icon and banner assets.
- Enable Tauri bundling.
- Configure NSIS installer.
- Add bundle script.
- Add install/uninstall smoke tests.

Acceptance criteria:

- Installer builds.
- Installed app launches.
- Branding is consistent.
- Installed app can track and sync.

### M9 — Release Hardening

Goal: release-candidate quality.

Deliverables:

- Expanded parser corpus.
- Player support matrix.
- Sync failure tests.
- DB corruption/recovery handling.
- Diagnostic log export.
- Accessibility pass.
- Performance pass.
- User docs.

Acceptance criteria:

- Critical workflows pass end-to-end.
- Failures surface clearly to the user.
- No known data-loss paths remain unguarded.
- Release checklist passes.

## Dependencies

- M0 blocks all feature milestones.
- M1 and M2 are tightly coupled; M1 can start with manual matching, then M2 improves automation.
- M3 depends on M0 and benefits from M2 for accurate progress updates.
- M4 depends on enough runtime state from M0-M3 to avoid placeholder UI.
- M5 depends on tracking lifecycle from M1.
- M6 depends on library entities from M4.
- M7 can start after M0 but should finish before release.
- M8 should happen after core value exists to avoid packaging an empty shell.
- M9 closes release risk after feature work stabilizes.

## Testing Strategy

- Rust unit tests for parsers, storage, settings, sync queue, and integration clients.
- Rust integration tests for Tauri command handlers where practical.
- Frontend unit tests for API wrappers and state stores.
- Frontend component tests for core views and error states.
- End-to-end smoke tests for app boot, DB status, settings, tracking flow, and sync flow.
- Fixture-based tests for filenames, AniList responses, Sonarr responses, migration snapshots, and sync failures.

## Risk Controls

- Manual correction is required for recognition because filename auto-match will fail in real libraries.
- Sync queue must be offline-first and idempotent to avoid duplicate or corrupt tracker updates.
- AniList writes must show pending/error status and never silently overwrite user data.
- Migration import must be previewable and reversible.
- Runtime foundation must be finished before feature UI to avoid placeholder screens masking missing backend behavior.

## First Implementation Plan Target

The first implementation plan should cover only M0 Runtime Foundation. It should include file-level tasks, tests, and acceptance gates for real database startup, state-backed commands, settings CRUD, and event delivery scaffolding.
