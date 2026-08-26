# AniVault Phase A1 — Local Tracking MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the local half of the tracking loop: detect local playback, recognize anime from filename/title, silently update local progress, and surface current activity in the tray/home UI.

**Architecture:** Add focused engine modules for recognition, detection, and orchestration. A1 deliberately excludes live AniList OAuth/sync and auto-add from network search; those move to A2. Detection uses process/window-title polling plus folder file watching. Recognition uses a pure filename parser and local SQLite FTS5 index. Orchestrator updates local `watch_history` and `list_entry` only.

**Tech Stack:** Tauri v2, Svelte 5, TypeScript, Rust 2021, SQLite FTS5, `windows` crate, Tokio.

## Global Constraints

- Everything runs on Windows only.
- Single local SQLite database is source of truth.
- Engine modules never import UI code.
- UI calls Tauri commands; never touches SQLite directly.
- Existing C++ Taiga source under `src/` must remain untouched.
- App product name: AniVault. Tauri identifier: `app.anivault.desktop`.
- Keep Phase 0 engine behavior unchanged where not explicitly extended.
- Every task ends with a commit and tests that pass.
- `npm run verify` must pass after every task.
- Silent tracking: no popups and no notifications.
- A1 does not perform live AniList OAuth, search, or sync. A2 will add that.
- A1 does not auto-add unknown anime from AniList. Unknown local matches are skipped and logged.
- A1 implements process/window-title detection and folder file watching. Windows media-session listener deferred to A2 unless it is trivial after A1.
- A1 tray scope: tray icon + Show/Quit menu. Minimize-to-tray behavior may be separate if Tauri window hooks need more work.

---

## File Structure

```text
next/src-tauri/
  Cargo.toml                         # Add regex-lite if needed
  migrations/0002_fts5_index.sql      # FTS5 virtual table for local titles
  src/engine/
    recognition/mod.rs                # Recognition module exports
    recognition/parser.rs             # Pure filename parsing
    recognition/matcher.rs            # FTS5 local matcher
    detection/mod.rs                  # DetectionManager + dedup
    detection/process.rs              # Known player process scanner
    detection/fs.rs                   # Folder/file event watcher
    orchestrator/mod.rs               # MediaDetected → local progress
    settings.rs                       # DetectionConfig defaults/settings decode
    models.rs                         # ParseResult, MatchResult, DetectionConfig, TrackingStatus
    storage.rs                        # FTS5 helpers + local progress helpers
    mod.rs                            # New module exports
    commands.rs                       # get_tracking_status command
    lib.rs                            # Starts detection/orchestrator and manages state
  tests/
    parser_test.rs
    matcher_test.rs
    detection_test.rs
    tracking_e2e_test.rs

next/src/
  lib/tray.ts
  lib/now-playing.svelte
  App.svelte
  brand.test.ts
```

---

### Task 1: Add filename recognition parser

**Files:**
- Create: `next/src-tauri/src/engine/recognition/mod.rs`
- Create: `next/src-tauri/src/engine/recognition/parser.rs`
- Create: `next/src-tauri/tests/parser_test.rs`
- Modify: `next/src-tauri/src/engine/models.rs`
- Modify: `next/src-tauri/src/engine/mod.rs`
- Modify: `next/src-tauri/Cargo.toml`

**Interfaces:**
- Consumes: nothing.
- Produces: `ParseResult { title, season, episode, confidence }` and `parse_filename(path: &str) -> ParseResult`.

- [ ] **Step 1:** Write parser tests for at least these filenames: `[SubsPlease] Spy x Family - 17.mkv`, `Frieren - S01E14.mkv`, `[Erai-raws] Jujutsu Kaisen 2nd Season - 05 [1080p][HEVC].mkv`, `Cowboy Bebop - 03.mkv`, `Attack on Titan #25.mkv`, `Mushishi.mkv`, `One.Piece.1088.mkv`, `Kusuriya_no_Hitorigoto_-_24.mkv`, `[HorribleSubs] Kimetsu no Yaiba [1080p] - 11.mkv`, `Bocchi the Rock! (06).mkv`, `Oshi no Ko S2 - 03.mkv`, `Mushoku Tensei Season 2 - 12.mkv`, `Vinland Saga Ep 07.mkv`, `Summer 2024.mkv`, `D:\Anime\Fall 2024\[SubsPlease] Dandadan - 12 (1080p) [54B2E3C0].mkv`.
- [ ] **Step 2:** Run `cargo test --test parser_test` from `next/src-tauri`. Expected fail: module missing.
- [ ] **Step 3:** Add `ParseResult` to `models.rs`, `recognition/mod.rs`, and `pub mod recognition;` in `engine/mod.rs`.
- [ ] **Step 4:** Implement parser. It must strip fansub/release tags, normalize separators, parse season/episode patterns, avoid treating years like `2024` as episodes, and preserve real title words.
- [ ] **Step 5:** Add `regex-lite = "0.1.6"` if parser uses regex.
- [ ] **Step 6:** Run `cargo test --test parser_test` and `npm run verify`. Expected pass.
- [ ] **Step 7:** Commit `feat: add anime filename parser`.

---

### Task 2: Add FTS5 title index and local matcher

**Files:**
- Create: `next/src-tauri/migrations/0002_fts5_index.sql`
- Create: `next/src-tauri/src/engine/recognition/matcher.rs`
- Create: `next/src-tauri/tests/matcher_test.rs`
- Modify: `next/src-tauri/src/engine/storage.rs`

**Interfaces:**
- Consumes: `Storage`, `ParseResult`.
- Produces: `MatchResult { anime_id, title, confidence, source }`, `build_fts_index(&Storage)`, `search_local(&Storage, &ParseResult)`.

- [ ] **Step 1:** Write failing matcher tests: exact romaji title, synonym title, and no-match unknown title.
- [ ] **Step 2:** Run `cargo test --test matcher_test`. Expected fail: matcher missing.
- [ ] **Step 3:** Add FTS5 virtual table `anime_fts(title, synonyms, content='anime', content_rowid='id')` plus triggers for `anime` insert/update/delete.
- [ ] **Step 4:** Extend `Storage` with `ensure_fts_index`, `insert_minimal_anime_with_synonyms`, `anime_by_id`, `update_watched_episodes`, and `insert_or_ignore_anime_local` helpers.
- [ ] **Step 5:** Implement matcher: exact title first, synonym match second, FTS5 match third. Confidence: exact 100, synonym 85, fuzzy 70-80.
- [ ] **Step 6:** Run `cargo test --test matcher_test` and `npm run verify`. Expected pass.
- [ ] **Step 7:** Commit `feat: add fts5 anime title matcher`.

---

### Task 3: Add process and folder detection

**Files:**
- Create: `next/src-tauri/src/engine/detection/mod.rs`
- Create: `next/src-tauri/src/engine/detection/process.rs`
- Create: `next/src-tauri/src/engine/detection/fs.rs`
- Create: `next/src-tauri/tests/detection_test.rs`
- Modify: `next/src-tauri/src/engine/models.rs`
- Modify: `next/src-tauri/src/engine/mod.rs`

**Interfaces:**
- Consumes: `EventBus`, `EngineEvent::MediaDetected`.
- Produces: `DetectionConfig`, `DetectionManager::start(bus, config)`, `DetectionManager::stop()`, `is_known_player`, `scan_players`, and file watcher event emission for configured folders.

- [ ] **Step 1:** Add `DetectionConfig { folders: Vec<String>, poll_interval_ms: u64 }` defaulting to `D:\Anime`, 2000ms.
- [ ] **Step 2:** Write tests for known player names, unknown processes, scan not panicking, and detection dedup helper.
- [ ] **Step 3:** Run `cargo test --test detection_test`. Expected fail.
- [ ] **Step 4:** Implement process scanner using a Windows-safe approach (`tasklist` acceptable for A1) and known players: `mpv`, `mpc-hc`, `vlc`, `PotPlayer`, `Microsoft.Media.Player`.
- [ ] **Step 5:** Implement folder watcher for configured folders. If native `ReadDirectoryChangesW` becomes too large, use a polling watcher with mtime checks for A1 and document it as polling. Do not leave an empty stub.
- [ ] **Step 6:** Implement `DetectionManager` with Tokio tasks and dedup within 60s.
- [ ] **Step 7:** Run detection tests and `npm run verify`. Expected pass.
- [ ] **Step 8:** Commit `feat: add local playback detection`.

---

### Task 4: Add local tracking orchestrator

**Files:**
- Create: `next/src-tauri/src/engine/orchestrator/mod.rs`
- Create/Modify: `next/src-tauri/tests/tracking_e2e_test.rs`
- Modify: `next/src-tauri/src/engine/mod.rs`

**Interfaces:**
- Consumes: `EventBus`, `Storage`, `parse_filename`, `search_local`.
- Produces: `start_tracking_loop(bus: EventBus, storage: Storage)`.

- [ ] **Step 1:** Write an E2E test that inserts `Spy x Family`, builds FTS index, simulates `MediaDetected` for `[SubsPlease] Spy x Family - 17.mkv`, runs orchestrator handler once, then verifies `watch_history_count(1, 17) == 1` and `list_entry.watched_episodes == 17`.
- [ ] **Step 2:** Run `cargo test --test tracking_e2e_test`. Expected fail: orchestrator missing.
- [ ] **Step 3:** Implement orchestrator with a testable `handle_media_detected(storage, detected) -> anyhow::Result<Option<MatchResult>>` function plus background `start_tracking_loop`.
- [ ] **Step 4:** Ensure duplicate same anime/episode does not append duplicate watch history.
- [ ] **Step 5:** Run E2E test and `npm run verify`. Expected pass.
- [ ] **Step 6:** Commit `feat: add local tracking orchestrator`.

---

### Task 5: Wire tracking status command

**Files:**
- Create: `next/src-tauri/src/engine/settings.rs`
- Modify: `next/src-tauri/src/engine/models.rs`
- Modify: `next/src-tauri/src/commands.rs`
- Modify: `next/src-tauri/src/lib.rs`
- Modify: `next/src-tauri/src/engine/mod.rs`

**Interfaces:**
- Consumes: detection/orchestrator.
- Produces: Tauri command `get_tracking_status() -> TrackingStatus` where `TrackingStatus { is_running: bool, current_anime: Option<String> }`.

- [ ] **Step 1:** Add `TrackingStatus` model.
- [ ] **Step 2:** Add `settings.rs` with `detection_config_from_settings(raw: &str) -> anyhow::Result<DetectionConfig>` returning defaults if no settings exist.
- [ ] **Step 3:** Wire `EventBus`, detection, and orchestrator setup into `run()` without blocking Tauri startup.
- [ ] **Step 4:** Add `get_tracking_status` command and register it in `generate_handler!`.
- [ ] **Step 5:** Run `npm run verify`. Expected pass.
- [ ] **Step 6:** Commit `feat: wire local tracking into tauri`.

---

### Task 6: Add tray and Now Playing UI

**Files:**
- Create: `next/src/lib/tray.ts`
- Create: `next/src/lib/now-playing.svelte`
- Modify: `next/src/App.svelte`
- Modify: `next/src/brand.test.ts`
- Modify: `next/src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: `get_tracking_status` command.
- Produces: tray setup module and Now Playing card.

- [ ] **Step 1:** Create `tray.ts` with Tauri tray icon and menu: Show AniVault, Quit.
- [ ] **Step 2:** Create `now-playing.svelte`. It polls `get_tracking_status` every 2s, shows current anime if present, and hides after 5s inactivity.
- [ ] **Step 3:** Integrate Now Playing below the banner on Home.
- [ ] **Step 4:** Extend brand test with existence checks for `src/lib/tray.ts` and `src/lib/now-playing.svelte`.
- [ ] **Step 5:** Configure tray icon in `tauri.conf.json` if Tauri schema accepts it. If schema rejects it, keep runtime tray setup only and document in report.
- [ ] **Step 6:** Run `npm run verify` and `npm run build`. Expected pass.
- [ ] **Step 7:** Commit `feat: add tray and now-playing ui`.

---

### Task 7: Final A1 installer verification

**Files:**
- Modify: `next/README.md`

**Interfaces:**
- Consumes: all A1 tasks.
- Produces: rebuilt installer and README note for A1 capabilities.

- [ ] **Step 1:** Update `next/README.md` with Phase A1 features: local filename recognition, local progress update, process/folder detection, tray, Now Playing; note live AniList sync is A2.
- [ ] **Step 2:** Run `npm run verify` from `next`.
- [ ] **Step 3:** Run `npm run bundle` from `next`.
- [ ] **Step 4:** Confirm installer artifact at `next/src-tauri/target/release/bundle/nsis/AniVault_0.1.0_x64-setup.exe`.
- [ ] **Step 5:** Commit `docs: document phase a1 local tracking`.

---

## Final Verification

Run from `next`:

```powershell
npm run verify
npm run bundle
```

Expected:
- TypeScript check passes.
- Vitest passes.
- Rust tests pass: parser, matcher, detection, tracking_e2e, existing Phase 0 tests.
- Tauri bundle produces installer.

## Completion Criteria

- Parser extracts title + episode from 15 real-world filename formats.
- FTS5 local matcher finds anime by romaji title, synonyms, and fuzzy title.
- Detection scanner identifies known player processes and folder changes.
- Orchestrator updates local watch history and list entry without duplicates.
- `get_tracking_status` command returns running state.
- Tray icon and Now Playing card exist and build.
- `npm run verify` passes.
- `npm run bundle` produces installer.
