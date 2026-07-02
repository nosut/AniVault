# AniVault

Windows desktop anime library and tracker. Rebuilt in Rust + Tauri 2 + Svelte 5 + SQLite. AniList-only tracker integration.

Based on [Taiga](https://github.com/erengy/taiga) by Eren Okka. Licensed under GPLv3.

## Quick Start

```powershell
cd next
npm install
npm run dev      # development
npm run test     # 45 tests
npm run bundle   # Windows installer (NSIS)
```

## Structure

```
next/
├── src/            # Svelte frontend (TypeScript, Vitest)
├── src-tauri/      # Rust backend (Tauri 2.4, sqlx, reqwest, tracing)
│   ├── src/engine/  # Core: scanner, parser, matcher, anilist, sonarr, migration
│   ├── migrations/  # SQLite schema
│   └── tests/       # Rust integration tests
└── scripts/        # verify.ps1, bundle.ps1

docs/               # Design specs and implementation plans
```

## License

[GNU General Public License v3](https://www.gnu.org/licenses/gpl-3.0.html)
