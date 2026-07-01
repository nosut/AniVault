# AniVault

Clean-room Windows-only anime library and tracker.

## Features

- **Local tracking** — detect playback, recognize anime from filenames, update progress silently
- **AniList integration** — OAuth, search, auto-add, sync with backoff
- **Library** — poster grid with status filters and search
- **Calendar** — seasonal anime browser via AniList
- **Watching** — progress tracking with bars, sorted by recent activity
- **Sonarr** — config, series mapping, monitored toggle
- **Tray** — minimize-to-tray, Show/Quit menu

## Phase A2 scope (AniList integration)

- AniList OAuth PKCE flow (token stored via DPAPI)
- AniList GraphQL search + auto-add (confidence ≥85)
- Sync queue worker with exponential backoff (30s → 6hr max)
- Sync status indicator in sidebar (green/amber dot)

## Phase A1 scope (local tracking)

- Filename parser (15 anime release patterns)
- SQLite FTS5 local title matcher (exact, synonym, fuzzy)
- Process + file folder playback detection
- Local tracking orchestrator (watch history + progress)
- Tray icon with Show/Quit menu
- Now Playing status chip (minimal, 5s timeout)

Live AniList OAuth, search, sync, and unknown-anime auto-add are Phase A2.

## Phase 0 scope

- Tauri v2 shell
- Svelte + TypeScript frontend
- Rust engine boundary
- SQLite storage foundation
- DPAPI secret storage
- Migration report skeleton
- Narrow Tauri command API

## Commands

```powershell
npm install
npm run verify
```

Run the desktop shell during development:

```powershell
npm run dev
```

In another terminal:

```powershell
Set-Location -LiteralPath src-tauri
cargo run
```

Build a local unsigned Windows installer:

```powershell
npm run bundle
```

Installer output is written under:

```text
next/src-tauri/target/release/bundle/
```
