# AniVault

Clean-room Windows-only anime library and tracker.

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
