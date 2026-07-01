# AniVault

Clean-room Windows-only anime library and tracker.

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
