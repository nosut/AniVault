<p align="center">
  <img src="Banner.png" alt="AniVault" width="100%" />
</p>

<h1 align="center">
  <img src="Icon.png" alt="" width="28" align="top" />
  AniVault
</h1>

<p align="center">
  <b>A modern Windows anime library and tracker.</b><br />
  Tracks what you play, keeps your library organized, and syncs it to AniList and Sonarr.
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
> **This is an AI-generated project.** AniVault is designed, written and maintained with
> Claude (Anthropic). Review the code before relying on it, and expect some rough edges.

## What is AniVault?

AniVault is a native Windows app that tracks the anime you watch and manages your local
collection. Leave it in the tray and it **detects the episode playing in your media player**,
advances your progress and pushes the update to **AniList**, with no manual logging. It also
indexes your anime folders, keeps a calendar of upcoming episodes, and works with **Sonarr** to
fill the gaps in your collection.

It's a clean-room reimagining of [Taiga](https://github.com/erengy/taiga) on a modern stack
(Rust + Tauri 2 + Svelte 5 + SQLite).

## Download

Get `AniVault_<version>_x64-setup.exe` from the
[latest release](https://github.com/nosut/AniVault/releases/latest) and run it. To upgrade,
install over the previous version; your library is kept. AniVault shows a notice when a new
release is out.

## Features

### ▶️ Watching
- **Automatic tracking** in mpv, VLC, MPC-HC, PotPlayer, Kodi and most other common players.
  The episode is recognized from the filename or window title.
- Progress, watch history and AniList update on their own, and a series completes itself at
  its final episode. Low-confidence matches ask you to confirm first.
- **Continue** on any series page plays the next episode, or picks a rewatch up where you
  left off.
- **Now Playing** shows every episode as a tile you can click to play, with
  Previous / Next / Replay. It can open by itself when an episode starts.
- **Up Next** offers the next episode when one ends.
- Pause tracking any time from the tray.

<p align="center">
  <a href="https://github.com/user-attachments/assets/2f7790ea-fb69-4414-b9af-95d1f354a8e9"><img src="https://github.com/user-attachments/assets/2f7790ea-fb69-4414-b9af-95d1f354a8e9" alt="Now playing" width="60%" /></a>
</p>

### 🏠 Home
- **Airing today**, with download status for each episode.
- **Ready to watch**: shows whose next episode is already on disk.
- **Missing downloads**, with a one-click Sonarr search.
- **Not in Sonarr**: Watching and Planning shows Sonarr doesn't have, ready to add or link.

<p align="center">
  <a href="https://github.com/user-attachments/assets/2fef399a-02e8-4694-9d76-b8861a985510"><img src="https://github.com/user-attachments/assets/2fef399a-02e8-4694-9d76-b8861a985510" alt="Dashboard" width="60%" /></a>
</p>

### 📚 Local library
- Scans your anime folders and matches each file to its show, including season markers
  and 2nd cours that Sonarr keeps inside season 1.
- **Library** in table or poster view, grouped by season, with countdowns to the next episode.
  Plan to Watch marks shows that have started airing or premiere within a week.
- **Collection**: the series you have on disk, with disk usage per series.
- **File Manager** for fixing, ignoring and removing file matches.
- Files deleted from disk are pruned on rescan, but an offline drive never wipes your data.
- Readable English titles for entries AniList leaves untranslated (never machine-translated).

<p align="center">
  <a href="https://github.com/user-attachments/assets/01f2b77b-29c9-4b03-8e95-593b914434ce"><img src="https://github.com/user-attachments/assets/01f2b77b-29c9-4b03-8e95-593b914434ce" alt="Library" width="49%" /></a>
  <a href="https://github.com/user-attachments/assets/090d18a8-25cb-44a9-96c9-88153ab2dde6"><img src="https://github.com/user-attachments/assets/090d18a8-25cb-44a9-96c9-88153ab2dde6" alt="Collection" width="49%" /></a>
</p>

### 🔗 AniList
- Sign in and import your existing list in one click.
- Progress, status and score changes sync in the background and retry through outages. If
  your login expires, AniVault asks you to reconnect and then sends the queued changes.
- Detail pages with cover art, synopsis, progress and score editing, watch history and
  related entries.
- **Seasons** page to browse any season, with shows added since your last visit at the top.

<p align="center">
  <a href="https://github.com/user-attachments/assets/7294d339-38f7-4d7e-8d35-bd408ceafffc"><img src="https://github.com/user-attachments/assets/7294d339-38f7-4d7e-8d35-bd408ceafffc" alt="Seasons" width="60%" /></a>
</p>

### 📺 Sonarr
- Imports your series, filtered by the Sonarr tags you choose, and re-syncs every 30 minutes.
- **Add to Sonarr** from search results, or by pasting a TVDB ID or link when search misses.
- Episode availability on each show's page, and missing episodes searchable from Home.
- **Clean up** when you drop or remove a show: unmonitor or remove it in Sonarr, and send its
  files to the Recycle Bin. Pick it back up later from the **Unmonitored** tab.

### 📅 Airing calendar
- Month and agenda views of upcoming episodes for the shows you follow.
- Watched episodes are checked off. Uses AniList's schedule, with Sonarr as a fallback, and
  works offline.

### 🛡️ Data and reliability
- Watch history (searchable) and stats, including your score distribution.
- Backup and restore, JSON export and import, and import from Taiga v1.
- AniList and Sonarr credentials are encrypted with Windows DPAPI.
- Errors explain what happened and what to do, with **Try again**. Background problems
  (a stalled sync, an offline library folder) show up in the app and clear once fixed.

### 🪟 Desktop
- Tray icon, close to tray, and optional launch on startup.
- Remembers window size and position. The sidebar can be collapsed and reordered, and you
  can choose the start page.

## Tech stack

| Layer | Technology |
|-------|-----------|
| Backend | Rust, [Tauri 2](https://tauri.app/), `sqlx` (SQLite), `reqwest` |
| Frontend | Svelte 5, TypeScript, Vite |
| Packaging | NSIS installer |

## Building from source

Needs **Windows**, [Node.js](https://nodejs.org/), the [Rust toolchain](https://rustup.rs/)
with the MSVC build tools, and the Tauri CLI (`cargo install tauri-cli --version "^2"`).

```powershell
cd next
npm install
cargo tauri dev   # run the app in development
npm run verify    # typecheck, svelte-check, Vitest, cargo check --tests
npm run bundle    # build the installer
```

Rust tests: `cargo test` in `next/src-tauri`.

## Credits

Based on [Taiga](https://github.com/erengy/taiga) by [Eren Okka](https://github.com/erengy).
AniVault is an independent reimplementation, not affiliated with the original.

## License

[GPL v3](LICENSE), the same as Taiga.
