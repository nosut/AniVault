<p align="center">
  <img src="Banner.png" alt="AniVault" width="100%" />
</p>

<h1 align="center">
  <img src="Icon.png" alt="" width="28" align="top" />
  AniVault
</h1>

<p align="center">
  <b>A modern Windows desktop anime library &amp; tracker.</b><br />
  Watches what you play, keeps your library organized, and syncs it all to AniList — automatically.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-Windows-0078D6?logo=windows" alt="Windows" />
  <img src="https://img.shields.io/badge/backend-Rust%20%2B%20Tauri%202-orange?logo=rust" alt="Rust + Tauri" />
  <img src="https://img.shields.io/badge/frontend-Svelte%205-ff3e00?logo=svelte" alt="Svelte 5" />
  <img src="https://img.shields.io/badge/storage-SQLite-003B57?logo=sqlite" alt="SQLite" />
  <img src="https://img.shields.io/badge/license-GPLv3-blue" alt="GPLv3" />
  <img src="https://img.shields.io/badge/built%20with-AI-8A63D2" alt="Built with AI" />
</p>

---

> [!IMPORTANT]
> **This is an AI-generated project.** AniVault was designed, written, and is maintained with
> the assistance of AI (Anthropic's Claude). Treat it accordingly — review the code before
> relying on it, and expect the rough edges that come with automated development.

## What is AniVault?

AniVault is a lightweight, native Windows app that tracks the anime you watch and manages your
local collection. Leave it running in the tray and it **detects playback in your media player**,
recognizes the episode, advances your progress, and pushes the update to **AniList** — no manual
logging required. It also indexes your local video folders, maps files to their shows, and gives
you a calendar of upcoming episodes.

It's a clean-room reimagining of the classic [Taiga](https://github.com/erengy/taiga), rebuilt on a
modern stack (Rust + Tauri 2 + Svelte 5 + SQLite).

## Features

### 🎬 Automatic playback tracking
- Detects supported media players (mpv, VLC, MPC-HC/BE, PotPlayer, and more) and recognizes the
  playing episode from its filename or window title.
- Auto-advances progress, auto-completes a series at its final episode, and shows a desktop
  notification — then queues the change for AniList.
- Pause tracking any time from the tray.

### 📚 Local library management
- Scan your anime folders; AniVault parses filenames (season/episode, release group, `SxxExx` and
  `1x01` formats) and matches each file to its show with a confidence score.
- **File Manager** for bulk mapping, ignoring, and removing indexed files, with a deep AniList
  match to resolve tricky titles.
- Keeps the index honest — files deleted from disk are pruned on rescan (guarded so an offline
  drive never wipes your data).
<img width="1282" height="852" alt="image" src="https://github.com/user-attachments/assets/01f2b77b-29c9-4b03-8e95-593b914434ce" />

### 🔗 AniList integration
- OAuth sign-in, one-click import of your existing list, and background two-way sync with retry
  and backoff.
- Rich detail pages: cover art, synopsis, progress/score editing, watch history, related entries,
  and next-airing countdowns.
<img width="1282" height="852" alt="image" src="https://github.com/user-attachments/assets/090d18a8-25cb-44a9-96c9-88153ab2dde6" />

### 📅 Airing calendar
- Month grid **and** agenda views of upcoming episodes for the shows you follow.
- Sourced primarily from AniList's airing schedule, with **Sonarr** as a fallback.
- Live countdowns to each release and hover cards with full titles + posters.

### 📺 Sonarr integration
- Connect your Sonarr instance to import series, auto-match them to your library, and see episode
  availability and next-airing info right on a show's detail page.

### 🖥️ Library &amp; dashboard
- Searchable, sortable library in table or poster-grid layout, with per-category sort memory and
  a search that spans every status.
- Dashboard with stats, "continue watching," seasonal browsing, and watch history.

### 🗄️ Data safety
- Import from a legacy Taiga v1 installation.
- Backup, export, and import your data.
- Secrets (AniList/Sonarr credentials) are encrypted at rest with Windows DPAPI.

### 🪟 Native desktop behavior
- System-tray icon, minimize-to-tray, and quit confirmation.
- Optional launch-on-startup that self-heals its registry entry across reinstalls.

## Tech stack

| Layer | Technology |
|-------|-----------|
| Backend / engine | Rust, [Tauri 2](https://tauri.app/), `sqlx` (SQLite), `reqwest`, `tracing` |
| Frontend | Svelte 5, TypeScript, Vite |
| Storage | SQLite with versioned migrations |
| Secrets | Windows DPAPI |
| Packaging | NSIS installer |

## Getting started

Prerequisites: **Windows**, [Node.js](https://nodejs.org/), and the
[Rust toolchain](https://rustup.rs/) with the MSVC build tools.

```powershell
cd next
npm install

npm run dev      # run the app in development
npm run test     # frontend tests (Vitest)
npm run check    # type-check
npm run bundle   # build the Windows installer (NSIS)
```

Rust engine checks:

```powershell
cd next/src-tauri
cargo check --tests
cargo test --test <name>   # individual integration tests
```

## Project structure

```
next/
├── src/            # Svelte frontend (TypeScript, Vitest)
├── src-tauri/      # Rust backend (Tauri 2)
│   ├── src/engine/ # scanner, parser, matcher, anilist, sonarr, migration, storage
│   ├── migrations/ # SQLite schema
│   └── tests/      # Rust integration tests
└── scripts/        # verify.ps1, bundle.ps1

docs/               # design specs and implementation plans
```

## Credits

Inspired by and based on [Taiga](https://github.com/erengy/taiga) by
[Eren Okka](https://github.com/erengy). AniVault is an independent reimplementation and is not
affiliated with the original project.

## License

[GNU General Public License v3.0](LICENSE) — same license as the original Taiga.
