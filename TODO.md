# TODO

Open items left over from the September 2026 code review. Everything else from that
review shipped in 1.0.23.

## Decisions (not bugs)

- **Split `next/src-tauri/src/commands.rs`** (about 3,500 lines) by domain. A mechanical
  refactor, best done as its own commit.
- **Watcher rescans.** A filesystem event at a library root makes `scan_specific_dirs`
  walk the whole tree recursively. Decide whether watcher-triggered passes should be
  non-recursive.
- **Quit on close.** Closing the window always goes to the tray; there is no setting to
  quit instead.
- **`--minimized` second launch.** The single-instance callback shows the window even
  when the second launch passed `--minimized`; it arguably should stay hidden.
- **Relation caching.** Relations are fetched from AniList on every detail-page open;
  they could be cached in the database.
- **Empty synopsis refetch.** `fetch_anime_detail_inner` calls AniList again on every
  open when a synopsis is legitimately empty.

## Manual testing for 1.0.23 (Windows)

These can't be covered by unit tests.

- [ ] Settings › AniList: "Retry blocked" appears when rows are blocked; the
      expired-login notice appears (store a garbage token, then Sync Now or Import Library).
- [ ] Settings › Tracking toggle starts and stops tracking at once and survives a
      restart; Now Playing Start/Stop is remembered too.
- [ ] Confirm on a low-confidence mpv/VLC match advances progress and stops re-prompting.
- [ ] Export to file / Import from file dialogs; Backup, then Restore and restart.
- [ ] No console window flashes at startup with launch-on-startup on.
- [ ] A player run as administrator is detected.
- [ ] Settings › Sonarr tag checkboxes list Sonarr's tags, save on toggle, and filter
      the import.
- [ ] The dashboard updates after an episode is detected, without switching views.

## Manual testing for 1.0.24 (Windows)

Both changes are layout-dependent, which jsdom can't reproduce.

- [ ] Library › Plan to Watch: the airing chip sits next to long titles without being
      clipped, in table, compact table and grid view, grouped and ungrouped.
- [ ] Scroll down Seasons (a dated season and Future Seasons), open a show, press Back:
      the page returns to the same spot. Same for Library, Collection and Search.
- [ ] A detail view opened from far down a list starts at its top.

## Manual testing for 1.0.25 (Windows)

The selection logic is unit-tested; the header layout is not.

- [ ] Library › Plan to Watch, grouped: each season header has a checkbox that lines up
      with the row checkboxes (table, compact table) and sits left of the chevron (grid).
      The accent stripe on "This season" / "Next season" still shows.
- [ ] Clicking a season checkbox does not collapse or expand the season; a partly checked
      season shows a dash.
- [ ] Check a season, open a show, press Back: the checks and the batch bar are still there.

## Manual testing for the Sonarr coverage check (Windows)

Needs the real Sonarr. Run an AniList sync first so `anime.format` is filled.

- [ ] Sonarr's `/api/v3/importlist` AniList entry has `implementation` containing
      "AniList" plus `rootFolderPath`, `qualityProfileId`, `shouldMonitor` and
      `monitorNewItems` (the add copies these; fix `SonarrImportList` if not).
- [ ] A Watching/Planning show known to be in Sonarr is not in the dashboard's
      "Not in Sonarr" panel.
- [ ] A later season whose parent series is in Sonarr is not flagged, or resolves
      with Link (the dialog marks the series "In Sonarr").
- [ ] Adding a missing show (e.g. Overgeared) creates it in Sonarr with the import
      list's root folder, quality profile and tags, and a search starts.
- [ ] Ignore hides a show from the panel; the detail page's Undo brings it back.
- [ ] With Sonarr stopped, the panel says "Sonarr unreachable".
- [ ] Movies on the list are not flagged.
