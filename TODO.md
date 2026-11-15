# TODO

Open decisions, none of them bugs.

- **Split `next/src-tauri/src/commands.rs`** (about 4,500 lines) by domain, as its own
  mechanical commit.
- **Watcher rescans.** A filesystem event at a library root makes `scan_specific_dirs`
  walk the whole tree. Should watcher-triggered passes be non-recursive?
- **Quit on close.** Closing the window always goes to the tray; there is no setting to
  quit instead.
- **`--minimized` second launch.** The single-instance callback shows the window even
  when the second launch passed `--minimized`.
- **Relation caching.** Relations are fetched from AniList on every detail-page open.
- **Empty synopsis refetch.** A show with a genuinely empty synopsis calls AniList again
  on every open.
