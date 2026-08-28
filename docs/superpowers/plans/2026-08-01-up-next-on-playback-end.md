# Up Next on Playback End Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show the Up Next prompt when playback of a library episode ends, offering the episode after the one that just played, instead of when tracked progress advances.

**Architecture:** The Rust tracking loop (`tracker.rs`, one tick every 2s) gains a watch session driven by a pure state machine in `session.rs`. `process_scan_result` returns the identified episode as a `SessionKey`; the tracker feeds it to `advance_session`, which reports when a session ends. Past a configurable minimum watch time the tracker publishes a new `PlaybackEnded` engine event. The Svelte frontend triggers its existing Up Next toast off that event instead of `ProgressAdvanced`, and passes the just-played episode to `get_up_next` so rewatches resolve correctly.

**Tech Stack:** Rust (Tauri 2, tokio, sqlx, anyhow, serde_json), Svelte + TypeScript, vitest, cargo test.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-08-01-up-next-on-playback-end-design.md`.
- Active codebase is `next/`. Frontend `next/src`, backend `next/src-tauri`.
- Verification: `npm run verify` in `next/` (typecheck + vitest + `cargo check --tests`); `cargo test` in `next/src-tauri`.
- If `cargo test` ICEs, re-run with `CARGO_INCREMENTAL=0` — it is a stale incremental cache, not the diff.
- Setting key: `up_next_min_watch_minutes`, default `5`, range `0`–`60`, where `0` means always prompt.
- Grace ticks before a session is considered ended: `2` — the session ends on the
  third consecutive miss, ~6s at the 2s tick, and the frontend drains events on
  its own 3s timer, so ~9s worst case from player close to prompt.
- `ProgressAdvanced` keeps being published (history, sync, Now Playing consume it) but is no longer an Up Next trigger. Manual mark-watched must not prompt.
- Do not build the installer, bump the version, push, or create a release unless the user asks. Task 6 covers the ask.
- Commit after each task with the message given in that task's final step.

---

### Task 1: Watch session state machine

**Files:**
- Modify: `next/src-tauri/src/engine/session.rs:7-21` (replace the unused `ActivePlayback` / `WatchSession` structs)
- Test: `next/src-tauri/src/engine/session.rs` (the existing `#[cfg(test)] mod tests` block at the bottom)

**Interfaces:**
- Consumes: nothing.
- Produces: `SessionKey { anime_id: i64, episode: i32, file_key: String }`, `ActivePlayback { key, started_at, last_seen_at, missed_ticks }`, `EndedPlayback { anime_id: i64, episode: i32, file_key: String, watched_secs: i64 }`, `advance_session(&mut Option<ActivePlayback>, Option<SessionKey>, i64, u8) -> Option<EndedPlayback>`, `passes_min_watch(i64, i64) -> bool`.

- [ ] **Step 1: Write the failing tests**

Add to the bottom of the existing `mod tests` block in `next/src-tauri/src/engine/session.rs` (keep the `guess_episode` tests that are already there):

```rust
    fn key(anime_id: i64, episode: i32) -> SessionKey {
        SessionKey {
            anime_id,
            episode,
            file_key: format!("D:/Anime/a{anime_id}-e{episode}.mkv"),
        }
    }

    #[test]
    fn same_key_across_ticks_does_not_end_the_session() {
        let mut session = None;
        assert_eq!(advance_session(&mut session, Some(key(1, 5)), 100, 2), None);
        assert_eq!(advance_session(&mut session, Some(key(1, 5)), 102, 2), None);
        let active = session.expect("session stays open");
        assert_eq!(active.started_at, 100);
        assert_eq!(active.last_seen_at, 102);
    }

    #[test]
    fn key_change_ends_the_previous_session() {
        let mut session = None;
        advance_session(&mut session, Some(key(1, 5)), 100, 2);
        advance_session(&mut session, Some(key(1, 5)), 400, 2);
        let ended = advance_session(&mut session, Some(key(1, 6)), 402, 2).expect("previous ended");
        assert_eq!(ended.anime_id, 1);
        assert_eq!(ended.episode, 5);
        assert_eq!(ended.watched_secs, 300);
        assert_eq!(session.expect("new session opened").key, key(1, 6));
    }

    #[test]
    fn one_missed_tick_within_grace_keeps_the_session() {
        let mut session = None;
        advance_session(&mut session, Some(key(1, 5)), 100, 2);
        assert_eq!(advance_session(&mut session, None, 102, 2), None);
        assert_eq!(advance_session(&mut session, None, 104, 2), None);
        assert!(session.is_some(), "two misses is still within a grace of 2");
    }

    #[test]
    fn misses_beyond_grace_end_the_session_excluding_grace_time() {
        let mut session = None;
        advance_session(&mut session, Some(key(1, 5)), 100, 2);
        advance_session(&mut session, Some(key(1, 5)), 400, 2);
        advance_session(&mut session, None, 402, 2);
        advance_session(&mut session, None, 404, 2);
        let ended = advance_session(&mut session, None, 406, 2).expect("grace exhausted");
        assert_eq!(ended.episode, 5);
        assert_eq!(
            ended.watched_secs, 300,
            "grace ticks must not inflate the watched time"
        );
        assert!(session.is_none(), "session is cleared once it ends");
    }

    #[test]
    fn nothing_observed_without_a_session_is_a_no_op() {
        let mut session = None;
        assert_eq!(advance_session(&mut session, None, 100, 2), None);
        assert!(session.is_none());
    }

    #[test]
    fn zero_grace_ends_the_session_on_the_first_miss() {
        let mut session = None;
        advance_session(&mut session, Some(key(1, 5)), 100, 0);
        advance_session(&mut session, Some(key(1, 5)), 700, 0);
        let ended = advance_session(&mut session, None, 702, 0).expect("ends immediately");
        assert_eq!(ended.watched_secs, 600);
    }

    #[test]
    fn min_watch_gate_compares_against_minutes() {
        assert!(!passes_min_watch(299, 5));
        assert!(passes_min_watch(300, 5));
        assert!(passes_min_watch(0, 0), "zero minutes always prompts");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd next/src-tauri && cargo test --lib engine::session`
Expected: FAIL — `cannot find function advance_session in this scope`, `cannot find function passes_min_watch`, and `SessionKey` not found.

- [ ] **Step 3: Write the implementation**

In `next/src-tauri/src/engine/session.rs`, replace the existing struct block (`pub struct ActivePlayback { … }` through `pub struct WatchSession { … }`, lines 7-21) with:

```rust
/// Identity of a playback session: which library episode is on screen.
///
/// `file_key` is the player-reported file path when there is one, else the
/// window title — mpv and VLC report only a title, and the key only has to be
/// stable across ticks of the same playback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionKey {
    pub anime_id: i64,
    pub episode: i32,
    pub file_key: String,
}

/// A playback session the tracker is currently observing.
#[derive(Debug, Clone)]
pub struct ActivePlayback {
    pub key: SessionKey,
    pub started_at: i64,
    pub last_seen_at: i64,
    pub missed_ticks: u8,
}

/// A session that just stopped being observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndedPlayback {
    pub anime_id: i64,
    pub episode: i32,
    pub file_key: String,
    pub watched_secs: i64,
}

impl ActivePlayback {
    fn end(&self) -> EndedPlayback {
        EndedPlayback {
            anime_id: self.key.anime_id,
            episode: self.key.episode,
            file_key: self.key.file_key.clone(),
            watched_secs: self.last_seen_at - self.started_at,
        }
    }
}

/// Advance the watch session by one tracker tick. Pure: no clock, no storage.
///
/// `observed` is the identified library episode on screen this tick, or `None`
/// when nothing recognisable is playing. A session survives up to `grace_ticks`
/// consecutive misses, so a momentary window-title glitch during a seek does not
/// end it; `watched_secs` is measured to the last tick the file was actually
/// seen, so grace never inflates it. Returns the session that just ended, if any.
pub fn advance_session(
    session: &mut Option<ActivePlayback>,
    observed: Option<SessionKey>,
    now: i64,
    grace_ticks: u8,
) -> Option<EndedPlayback> {
    let Some(key) = observed else {
        let Some(active) = session.as_mut() else {
            return None;
        };
        active.missed_ticks = active.missed_ticks.saturating_add(1);
        if active.missed_ticks <= grace_ticks {
            return None;
        }
        return session.take().as_ref().map(ActivePlayback::end);
    };

    if let Some(active) = session.as_mut() {
        if active.key == key {
            active.last_seen_at = now;
            active.missed_ticks = 0;
            return None;
        }
    }

    let ended = session.take().as_ref().map(ActivePlayback::end);
    *session = Some(ActivePlayback {
        key,
        started_at: now,
        last_seen_at: now,
        missed_ticks: 0,
    });
    ended
}

/// Whether a finished session outlasted the configured minimum. `0` always passes.
pub fn passes_min_watch(watched_secs: i64, min_minutes: i64) -> bool {
    watched_secs >= min_minutes.max(0) * 60
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd next/src-tauri && cargo test --lib engine::session`
Expected: PASS — 5 pre-existing `guess_episode` tests plus the 7 new ones.

- [ ] **Step 5: Commit**

```bash
git add next/src-tauri/src/engine/session.rs
git commit -m "feat: add watch session state machine for playback end"
```

---

### Task 2: Emit `PlaybackEnded` from the tracking loop

**Files:**
- Modify: `next/src-tauri/src/engine/events.rs:28-60` (add the `PlaybackEnded` variant to `EngineEvent`)
- Modify: `next/src-tauri/src/engine/session.rs:45-176` (`process_scan_result` return type)
- Modify: `next/src-tauri/src/engine/tracker.rs:1-65` (session wiring, threshold, publish)
- Test: `next/src-tauri/tests/session_test.rs`

**Interfaces:**
- Consumes: `SessionKey`, `ActivePlayback`, `EndedPlayback`, `advance_session`, `passes_min_watch` from Task 1.
- Produces: `EngineEvent::PlaybackEnded { anime_id: i64, episode: i32, file_key: String, watched_secs: i64 }`; `process_scan_result(&EngineState, ScanResult) -> anyhow::Result<Option<SessionKey>>`.

- [ ] **Step 1: Write the failing tests**

Append to `next/src-tauri/tests/session_test.rs`:

```rust
#[tokio::test]
async fn process_scan_result_returns_a_session_key_for_identified_playback() {
    let state = make_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Cowboy Bebop")
        .await
        .unwrap();

    let result = make_scan_result(
        "mpv.exe",
        "D:/Anime/Cowboy Bebop - 01.mkv",
        "Cowboy Bebop - 01",
    );

    let key = process_scan_result(&state, result)
        .await
        .unwrap()
        .expect("identified playback opens a session");
    assert_eq!(key.anime_id, 1);
    assert_eq!(key.episode, 1);
    assert_eq!(key.file_key, "D:/Anime/Cowboy Bebop - 01.mkv");
}

#[tokio::test]
async fn process_scan_result_returns_no_session_key_for_unknown_playback() {
    let state = make_state().await;
    let result = make_scan_result(
        "mpv.exe",
        "D:/Anime/Nothing In The Library - 01.mkv",
        "Nothing In The Library - 01",
    );

    assert!(
        process_scan_result(&state, result).await.unwrap().is_none(),
        "playback with no confident library match must not open a session"
    );
}

#[tokio::test]
async fn process_scan_result_falls_back_to_the_window_title_as_the_session_key() {
    let state = make_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Cowboy Bebop")
        .await
        .unwrap();

    // mpv/VLC report no file path — only a window title.
    let result = ScanResult {
        player_name: "vlc.exe".to_string(),
        file_path: None,
        window_title: Some("Cowboy Bebop - 01".to_string()),
        detected_at_unix: 1_782_769_000,
    };

    let key = process_scan_result(&state, result)
        .await
        .unwrap()
        .expect("title-only playback still opens a session");
    assert_eq!(key.file_key, "Cowboy Bebop - 01");
}
```

The title-only case is the mpv/VLC path that `process_scan_result` already
handles specially (it refuses to store a window title as a file-index key). If
that third test fails at *recognition* rather than at the session key, check
`recognize_file` in `next/src-tauri/src/engine/matcher.rs` for how it treats an
empty path before changing anything in `session.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd next/src-tauri && cargo test --test session_test`
Expected: FAIL — `expect` / `is_none` called on `()`; `process_scan_result` returns `Result<()>`.

- [ ] **Step 3: Add the event variant**

In `next/src-tauri/src/engine/events.rs`, add to the `EngineEvent` enum, directly after the `ProgressAdvanced { … }` variant:

```rust
    /// Playback of an identified library episode stopped being detected — the
    /// player closed or moved on to another file. Published only once the
    /// session outlasted the configured minimum watch time; the Up Next prompt
    /// is driven off this, not off `ProgressAdvanced`.
    PlaybackEnded {
        anime_id: AnimeId,
        episode: EpisodeNumber,
        file_key: String,
        watched_secs: i64,
    },
```

- [ ] **Step 4: Return the session key from `process_scan_result`**

In `next/src-tauri/src/engine/session.rs`, change the signature:

```rust
pub async fn process_scan_result(
    state: &EngineState,
    result: ScanResult,
) -> anyhow::Result<Option<SessionKey>> {
```

Declare the key just below the `recognize_file` call, before the `const AUTO_CONFIRM_THRESHOLD` line:

```rust
    let mut session_key: Option<SessionKey> = None;
```

Inside `if episode > 0 {`, as the first statements of that block (before the `if !recognition.known_file` file-index write):

```rust
            // The tracker keys the watch session on the path when the player
            // reports one, else the window title (mpv/VLC report title only).
            let file_key = result
                .file_path
                .clone()
                .or_else(|| result.window_title.clone())
                .unwrap_or_default();
            session_key = Some(SessionKey {
                anime_id,
                episode,
                file_key,
            });
```

Change the final line of the function from `Ok(())` to:

```rust
    Ok(session_key)
```

Leave every existing publish and side effect (`AnimeIdentified`, progress upsert, history, auto-complete, sync enqueue, `ProgressAdvanced`, `notify_progress`, `PlaybackDetected`) exactly as it is.

- [ ] **Step 5: Wire the tracking loop**

Replace the body of `next/src-tauri/src/engine/tracker.rs` above its `#[cfg(test)]` block with:

```rust
use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::sync::watch;

use crate::engine::events::EngineEvent;
use crate::engine::player_registry::builtin_player_registry;
use crate::engine::runtime::EngineState;
use crate::engine::scanner::{scan_active_players, ScannerConfig};
use crate::engine::session::{
    advance_session, passes_min_watch, process_scan_result, ActivePlayback, EndedPlayback,
    SessionKey,
};

/// Consecutive scan misses tolerated before a session is treated as ended. At
/// the 2s tick the session ends on the third straight miss, ~6s (plus up to 3s
/// of frontend drain latency), enough to ride out an unrecognised tick or two.
const GRACE_TICKS: u8 = 2;

/// Fallback when `up_next_min_watch_minutes` is unset or unparseable.
const DEFAULT_MIN_WATCH_MINUTES: i64 = 5;

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Minutes a session must last before it is worth an Up Next prompt. `0` always
/// prompts. A missing or malformed setting falls back to the default.
async fn min_watch_minutes(state: &EngineState) -> i64 {
    state
        .storage
        .get_setting("up_next_min_watch_minutes")
        .await
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<i64>(&raw).ok())
        .unwrap_or(DEFAULT_MIN_WATCH_MINUTES)
}

async fn publish_playback_ended(state: &EngineState, ended: EndedPlayback) {
    if !passes_min_watch(ended.watched_secs, min_watch_minutes(state).await) {
        return;
    }
    state.events.publish(EngineEvent::PlaybackEnded {
        anime_id: ended.anime_id,
        episode: ended.episode,
        file_key: ended.file_key,
        watched_secs: ended.watched_secs,
    });
}

pub async fn run_tracking_loop(
    state: EngineState,
    interval_ms: u64,
    cancel: watch::Receiver<bool>,
) {
    let config = ScannerConfig {
        known_players: builtin_player_registry(),
    };
    let mut session: Option<ActivePlayback> = None;

    loop {
        if *cancel.borrow() {
            break;
        }

        if state.tracking_paused.load(Ordering::Relaxed) {
            // Pausing ends any open session immediately — no grace, because the
            // scanner deliberately stops looking.
            if let Some(ended) = advance_session(&mut session, None, unix_now(), 0) {
                publish_playback_ended(&state, ended).await;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
            continue;
        }

        let config_clone = config.clone();
        let results = tokio::task::spawn_blocking(move || scan_active_players(&config_clone))
            .await
            .unwrap_or_default();

        // Update watching status — lock scope must not span .await
        let observed: Option<SessionKey> = if let Some(result) = results.first() {
            {
                let mut ctrl = state.tracking.lock().unwrap();
                ctrl.watching = Some(crate::engine::runtime::ActivePlaybackPub {
                    player_name: result.player_name.clone(),
                    file_path: result.file_path.clone(),
                    window_title: result.window_title.clone(),
                    episode_guess: crate::engine::session::guess_episode(
                        result.file_path.as_deref(),
                        result.window_title.as_deref(),
                    ),
                });
            } // lock dropped here

            match process_scan_result(&state, result.clone()).await {
                Ok(key) => key,
                Err(e) => {
                    tracing::warn!("session error: {e}");
                    None
                }
            }
        } else {
            let mut ctrl = state.tracking.lock().unwrap();
            ctrl.watching = None;
            None
        };

        if let Some(ended) = advance_session(&mut session, observed, unix_now(), GRACE_TICKS) {
            publish_playback_ended(&state, ended).await;
        }

        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
    }

    // Cleanup — a session open at shutdown still counts as ended.
    if let Some(ended) = advance_session(&mut session, None, unix_now(), 0) {
        publish_playback_ended(&state, ended).await;
    }

    let mut ctrl = state.tracking.lock().unwrap();
    ctrl.active = false;
    ctrl.watching = None;
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cd next/src-tauri && cargo test --test session_test && cargo test --lib engine::tracker`
Expected: PASS — the three new session-key tests plus the three pre-existing `process_scan_result` tests and the tracker's `track_nothing_when_no_active_players`.

- [ ] **Step 7: Check the whole crate still builds**

Run: `cd next/src-tauri && cargo check --tests`
Expected: no errors. If `process_scan_result`'s new return type breaks another call site, fix it by ignoring the value with `let _ = …` only where the caller genuinely has no session to track.

- [ ] **Step 8: Commit**

```bash
git add next/src-tauri/src/engine/events.rs next/src-tauri/src/engine/session.rs next/src-tauri/src/engine/tracker.rs next/src-tauri/tests/session_test.rs
git commit -m "feat: publish PlaybackEnded when a tracked episode stops playing"
```

---

### Task 3: Resolve Up Next relative to the episode just played

**Files:**
- Modify: `next/src-tauri/src/commands.rs:3070-3087` (`get_up_next_inner`, `get_up_next`)
- Test: `next/src-tauri/src/commands.rs` (the `mod tests` block, next to `up_next_picks_first_downloaded_episode_after_watched` at line 3395)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `get_up_next_inner(&EngineState, i64, Option<i32>) -> anyhow::Result<Option<UpNext>>`; the `get_up_next` Tauri command now takes `after: Option<i32>` alongside `anime_id`.

- [ ] **Step 1: Write the failing test**

Add to the `mod tests` block in `next/src-tauri/src/commands.rs`, after `up_next_none_when_no_unwatched_file`:

```rust
    #[test]
    fn up_next_after_the_played_episode_offers_the_following_file() {
        // A completed show (progress 12) where the user just replayed episode 3:
        // the threshold is the played episode, not the recorded progress.
        let files = vec![
            file_row(1, Some(1), 10, false),
            file_row(1, Some(2), 20, false),
            file_row(1, Some(3), 30, false),
            file_row(1, Some(4), 40, false),
        ];
        let un = up_next_from(1, "Show".into(), None, 3, &files).expect("has next");
        assert_eq!(un.episode, 4);
        assert!(un.file_path.contains("e4"));
    }
```

- [ ] **Step 2: Run the test to verify it passes already**

Run: `cd next/src-tauri && cargo test --lib up_next`
Expected: PASS. `up_next_from` already takes the threshold as a parameter — this test pins the behaviour the new `after` argument depends on. The wiring below is what actually changes.

- [ ] **Step 3: Thread `after` through the command**

In `next/src-tauri/src/commands.rs`, replace `get_up_next_inner` and `get_up_next`:

```rust
pub async fn get_up_next_inner(
    state: &EngineState,
    anime_id: i64,
    after: Option<i32>,
) -> anyhow::Result<Option<UpNext>> {
    let Some((title, image_url, watched)) = state.storage.up_next_meta(anime_id).await? else {
        return Ok(None);
    };
    let files = state.storage.file_index_by_anime(anime_id).await?;
    // `after` is the episode that just finished playing. Without it, fall back to
    // recorded progress — which is also what keeps rewatches from returning None.
    Ok(up_next_from(
        anime_id,
        title,
        image_url,
        after.unwrap_or(watched),
        &files,
    ))
}

#[tauri::command]
pub async fn get_up_next(
    anime_id: i64,
    after: Option<i32>,
    state: tauri::State<'_, EngineState>,
) -> Result<Option<UpNext>, String> {
    get_up_next_inner(&state, anime_id, after)
        .await
        .map_err(command_error)
}
```

- [ ] **Step 4: Verify the crate builds and tests pass**

Run: `cd next/src-tauri && cargo test --lib up_next && cargo check --tests`
Expected: PASS with no errors. `get_up_next` is registered in `next/src-tauri/src/lib.rs:272` and needs no change there — Tauri picks the new argument up from the command signature.

- [ ] **Step 5: Commit**

```bash
git add next/src-tauri/src/commands.rs
git commit -m "feat: resolve Up Next after the episode that just played"
```

---

### Task 4: Trigger the prompt off `PlaybackEnded`

**Files:**
- Modify: `next/src/lib/api.ts:140-187` (event union), `next/src/lib/api.ts:784-786` (`getUpNext`)
- Modify: `next/src/lib/upNext.ts:8-19` (replace `latestProgressAdvance`)
- Modify: `next/src/App.svelte:5` (import), `next/src/App.svelte:223-240` (`maybePromptUpNext`)
- Test: `next/src/lib/upNext.test.ts`, `next/src/lib/api.test.ts:488-492`

**Interfaces:**
- Consumes: `EngineEvent::PlaybackEnded` (Task 2), `get_up_next`'s `after` argument (Task 3).
- Produces: `PlaybackEndedEvent` TS interface; `latestPlaybackEnded(events: EngineEvent[]) -> { anime_id: number; episode: number } | null`; `getUpNext(animeId: number, after?: number, invokeFn?: InvokeFn)`.

- [ ] **Step 1: Write the failing tests**

Replace the whole of `next/src/lib/upNext.test.ts` with:

```ts
import { describe, it, expect } from 'vitest';
import { latestPlaybackEnded, samePrompt } from './upNext';
import type { EngineEvent } from './api';

const pe = (anime_id: number, episode: number): EngineEvent =>
  ({ PlaybackEnded: { anime_id, episode, file_key: `D:/a${anime_id}-e${episode}.mkv`, watched_secs: 1500 } } as EngineEvent);

describe('latestPlaybackEnded', () => {
  it('returns null when there is no PlaybackEnded event', () => {
    expect(latestPlaybackEnded([])).toBeNull();
    expect(latestPlaybackEnded([{ LibraryUpdated: { indexed: 1, removed: 0 } } as EngineEvent])).toBeNull();
  });
  it('ignores ProgressAdvanced, which no longer drives the prompt', () => {
    const pa = { ProgressAdvanced: { anime_id: 1, old_episode: 2, new_episode: 3, source: 'manual' } } as EngineEvent;
    expect(latestPlaybackEnded([pa])).toBeNull();
  });
  it('returns the last PlaybackEnded in the batch', () => {
    expect(latestPlaybackEnded([pe(1, 3), pe(2, 5)])).toEqual({ anime_id: 2, episode: 5 });
  });
  it('returns the last PlaybackEnded even when another event trails it', () => {
    expect(
      latestPlaybackEnded([pe(1, 3), { LibraryUpdated: { indexed: 1, removed: 0 } } as EngineEvent]),
    ).toEqual({ anime_id: 1, episode: 3 });
  });
});

describe('samePrompt', () => {
  it('treats identical anime+episode as the same prompt', () => {
    expect(samePrompt({ anime_id: 1, episode: 13 }, { anime_id: 1, episode: 13 })).toBe(true);
    expect(samePrompt({ anime_id: 1, episode: 13 }, { anime_id: 1, episode: 14 })).toBe(false);
    expect(samePrompt(null, { anime_id: 1, episode: 13 })).toBe(false);
    expect(samePrompt(null, null)).toBe(false);
  });
});
```

In `next/src/lib/api.test.ts`, replace the existing `getUpNext` test (line 488) with:

```ts
  it('getUpNext passes animeId and the after episode, and returns the prompt or null', async () => {
    const fake = vi.fn().mockResolvedValue({ anime_id: 1, title: 'Frieren', image_url: null, episode: 13, file_path: 'C:/x/e13.mkv' });
    const res = await getUpNext(1, 12, fake);
    expect(fake).toHaveBeenCalledWith('get_up_next', { animeId: 1, after: 12 });
    expect(res?.episode).toBe(13);
  });

  it('getUpNext sends a null after when none is given', async () => {
    const fake = vi.fn().mockResolvedValue(null);
    await getUpNext(1, undefined, fake);
    expect(fake).toHaveBeenCalledWith('get_up_next', { animeId: 1, after: null });
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd next && npx vitest run src/lib/upNext.test.ts src/lib/api.test.ts`
Expected: FAIL — `latestPlaybackEnded` is not exported, and `getUpNext` is called with `{ animeId: 1 }`.

- [ ] **Step 3: Add the event type and the `after` argument**

In `next/src/lib/api.ts`, add after the `ProgressAdvancedEvent` interface:

```ts
export interface PlaybackEndedEvent {
  PlaybackEnded: {
    anime_id: number;
    episode: number;
    file_key: string;
    watched_secs: number;
  };
}
```

Add it to the union:

```ts
export type EngineEvent =
  | MediaDetectedEvent
  | PlaybackDetectedEvent
  | PlaybackEndedEvent
  | AnimeIdentifiedEvent
  | ProgressAdvancedEvent
  | SyncQueuedEvent
  | SyncFailedEvent
  | LibraryUpdatedEvent;
```

Replace `getUpNext`:

```ts
export function getUpNext(animeId: number, after?: number, invokeFn: InvokeFn = tauriInvoke): Promise<UpNext | null> {
  return invokeFn<UpNext | null>('get_up_next', { animeId, after: after ?? null });
}
```

- [ ] **Step 4: Replace the trigger helper**

In `next/src/lib/upNext.ts`, replace the `latestProgressAdvance` function (and its doc comment) with:

```ts
/** The most recent PlaybackEnded in a polled event batch, if any. */
export function latestPlaybackEnded(
  events: EngineEvent[],
): { anime_id: number; episode: number } | null {
  for (let i = events.length - 1; i >= 0; i--) {
    const ev = events[i];
    if (ev && 'PlaybackEnded' in ev) {
      return { anime_id: ev.PlaybackEnded.anime_id, episode: ev.PlaybackEnded.episode };
    }
  }
  return null;
}
```

Leave `PromptKey` and `samePrompt` untouched.

- [ ] **Step 5: Point the prompt at the new trigger**

In `next/src/App.svelte`, change the import on line 5:

```ts
  import { latestPlaybackEnded, samePrompt, type PromptKey } from './lib/upNext';
```

Replace `maybePromptUpNext`:

```ts
  // The prompt fires when playback of a library episode ends — not when progress
  // advances, so marking episodes watched by hand never prompts.
  async function maybePromptUpNext(events: EngineEvent[]) {
    try {
      const ended = latestPlaybackEnded(events);
      if (!ended) return;
      const toastOn = (await getSetting<boolean>('up_next_toast_enabled')) ?? true;
      const notifyOn = (await getSetting<boolean>('up_next_notification_enabled')) ?? true;
      if (!toastOn && !notifyOn) return;
      const next = await getUpNext(ended.anime_id, ended.episode);
      if (!next) return;
      const key: PromptKey = { anime_id: next.anime_id, episode: next.episode };
      if (samePrompt(key, lastPromptKey)) return; // already surfaced this one
      lastPromptKey = key;
      if (toastOn) upNextPrompt = next;
      if (notifyOn) void notifyUpNext(next.title, next.episode);
    } catch {
      // Best-effort; a failed lookup just means no prompt this cycle.
    }
  }
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cd next && npx vitest run src/lib/upNext.test.ts src/lib/api.test.ts && npm run typecheck`
Expected: PASS, and typecheck clean. A typecheck error naming `latestProgressAdvance` means a leftover import — remove it.

- [ ] **Step 7: Commit**

```bash
git add next/src/lib/api.ts next/src/lib/upNext.ts next/src/lib/upNext.test.ts next/src/lib/api.test.ts next/src/App.svelte
git commit -m "feat: prompt Up Next when playback ends instead of on progress"
```

---

### Task 5: Minimum watch time setting in Settings

**Files:**
- Modify: `next/src/lib/SettingsView.svelte:42-43` (state), `:216-217` (load), `:222-231` (save helpers), `:537-549` (markup), style block (input width)

**Interfaces:**
- Consumes: the `up_next_min_watch_minutes` setting read by `min_watch_minutes` in Task 2.
- Produces: nothing other tasks depend on.

- [ ] **Step 1: Add the state and loader**

In `next/src/lib/SettingsView.svelte`, next to the existing `upNextToast` / `upNextNotify` declarations (line 42):

```ts
  let upNextMinMinutes = 5;
```

In the same `onMount` block that reads the two toggles (line 216), add:

```ts
      upNextMinMinutes = (await getSetting<number>('up_next_min_watch_minutes')) ?? 5;
```

- [ ] **Step 2: Add the save helper**

Next to `toggleUpNextToast` / `toggleUpNextNotify`:

```ts
  // Clamped here as well as in the input so a typed-in value can't disable the
  // gate by accident. 0 means "prompt however briefly the episode played".
  async function commitUpNextMinMinutes(value: number) {
    const clamped = Math.min(60, Math.max(0, Math.round(Number.isFinite(value) ? value : 5)));
    upNextMinMinutes = clamped;
    try { await setSetting('up_next_min_watch_minutes', clamped); }
    catch { /* keep showing the entered value; the next load re-reads storage */ }
  }
```

- [ ] **Step 3: Add the markup**

Directly after the "Also send a Windows notification" `toggle-row` div (line 549), inside the same section:

```svelte
          <div class="toggle-row">
            <label class="label" for="up-next-min-minutes">Only prompt after an episode played for at least (minutes)</label>
            <input
              id="up-next-min-minutes"
              class="form-input up-next-minutes"
              type="number"
              min="0"
              max="60"
              step="1"
              value={upNextMinMinutes}
              on:change={(e) => commitUpNextMinMinutes(e.currentTarget.valueAsNumber)}
            />
          </div>
          <p class="hint">Set 0 to prompt however briefly the episode played.</p>
```

Add to the component's `<style>` block:

```css
  .up-next-minutes {
    width: 5rem;
    text-align: right;
  }
```

- [ ] **Step 4: Verify**

Run: `cd next && npm run typecheck && npx vitest run`
Expected: typecheck clean, full vitest suite green.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/SettingsView.svelte
git commit -m "feat: add Up Next minimum watch time setting"
```

---

### Task 6: Full verification and release decision

**Files:**
- None changed by default. A version bump touches `next/package.json`, `next/package-lock.json`, `next/src-tauri/Cargo.toml`, `next/src-tauri/tauri.conf.json`, `next/src-tauri/Cargo.lock` — only if the user asks for a build.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: nothing.

- [ ] **Step 1: Run the full verification**

Run: `cd next && npm run verify`
Expected: typecheck, vitest and `cargo check --tests` all pass.

- [ ] **Step 2: Run the Rust suite**

Run: `cd next/src-tauri && cargo test`
Expected: PASS. On an ICE, re-run as `CARGO_INCREMENTAL=0 cargo test` before investigating the diff.

- [ ] **Step 3: Report and ask about the build**

Report the verification output as it actually came out. Then ask the user whether they want a version bump and installer build for this change — patch bump from the current `1.0.13`, all four version files kept in sync, `npm run bundle` in `next/`, then push and GitHub release, per `CLAUDE.md`. Do not bump, build, push or release without an explicit yes.

- [ ] **Step 4: Manual smoke check (user-driven, optional)**

Worth confirming in the running app, since none of it is covered by automated tests:
- Play a library episode for longer than the configured minimum, then close the player → the Up Next toast offers the following episode within ~9s (three missed 2s ticks plus up to one 3s frontend drain).
- Let a playlist roll from one episode straight into the next → no toast for the episode that just ended, because the next one is already playing.
- Pause tracking (tray or Settings) mid-episode, or turn tracking off → no toast.
- Replay an already-watched episode of a completed show → the toast offers the episode after the one replayed.
- Close a library episode after a few seconds → no toast.
- Mark an episode watched from the UI → no toast.
