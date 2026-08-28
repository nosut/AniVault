# Up Next on playback end

Date: 2026-08-01
Status: approved

## Problem

The Up Next prompt only appears when playback advances the tracked progress.
`App.svelte:maybePromptUpNext` scans drained engine events for `ProgressAdvanced`,
which `engine/session.rs` publishes only when the detected episode is greater than
the stored `watched_episodes`. Three consequences:

- Replaying or resuming an episode already watched never prompts, even though the
  engine emits `PlaybackDetected` and `AnimeIdentified` on every 2s tick.
- The prompt fires at the *start* of an episode. There is no dwell threshold, so
  progress advances on the first tick that recognises the file — starting episode 5
  immediately offers episode 6, and the toast has no auto-dismiss.
- `get_up_next` resolves "next" relative to watched progress, so a rewatch of a
  finished show would return `None` even if the trigger were fixed.

Manual "mark watched" also publishes `ProgressAdvanced` (`commands.rs:484`) and
therefore also prompts today.

## Goal

Prompt once, when playback of a library show ends, offering the episode after the
one that just played — and stop prompting on progress changes.

## Design

### 1. Watch session state machine (`engine/session.rs`)

Replace the currently-unused `ActivePlayback` / `WatchSession` structs with a real,
pure transition function:

```rust
pub struct SessionKey { anime_id: i64, episode: i32, file_key: String }
pub struct ActivePlayback { key: SessionKey, started_at: i64, last_seen_at: i64, missed_ticks: u8 }
pub struct EndedPlayback { anime_id: i64, episode: i32, file_key: String, watched_secs: i64 }

/// Pure transition: no I/O, no clock, no storage.
pub fn advance_session(
    session: &mut Option<ActivePlayback>,
    observed: Option<SessionKey>,
    now: i64,
    grace_ticks: u8,
) -> Option<EndedPlayback>
```

Rules:

- Same key observed → refresh `last_seen_at`, reset `missed_ticks`, return `None`.
- Different key observed → end the previous session, open a new one, return the
  `EndedPlayback` for the previous.
- Nothing observed → increment `missed_ticks`; end the session only once
  `missed_ticks > grace_ticks`. With `grace_ticks = 2` the session ends on the
  third consecutive miss (~6s at the 2s tick), so a tick or two that fails to
  recognise the player does not end a session.
- `watched_secs = last_seen_at - started_at`, so grace ticks do not inflate it.

Session identity is `anime_id` + `episode` only, compared through an explicit
`same_session` rather than a derived `PartialEq`. `file_key` rides along as
metadata for the published event. There is no real media path to key on:
`scan_active_players` sets `file_path` to the window title (falling back to the
player executable), and window titles mutate mid-playback — an elapsed time, a
percentage, a paused marker — so keying on one would end and restart the session
every tick, leaving `watched_secs` at ~0 and the minimum-watch gate rejecting
every session.

Because a session ends and the next opens in the same `advance_session` call,
`superseded_by(&ended, session.as_ref())` suppresses the publish when the newly
opened session is a later episode of the same anime — a player advancing itself
through a playlist. The user is already watching the episode the prompt would
offer.

### 2. Identification hand-off (`engine/session.rs`)

`process_scan_result` changes its return type from `anyhow::Result<()>` to
`anyhow::Result<Option<SessionKey>>`. It already computes `anime_id` and `episode`
in its confident branch (`known_file` or confidence >= 80, with `episode > 0`); it
now returns them. All existing event publishing — `AnimeIdentified`,
`ProgressAdvanced`, `PlaybackDetected` — and all progress/history/sync side effects
are unchanged.

Only a confidently identified file with a known episode opens a session. That is
the definition of "a show playing that is in the library".

### 3. Threshold and the new event (`engine/tracker.rs`, `engine/events.rs`)

`run_tracking_loop` owns the `Option<ActivePlayback>` as a loop-local and feeds
`advance_session` each tick with the key returned by `process_scan_result` (or
`None` when no player is detected). It also closes the session when tracking is
paused and when the loop exits — silently in both cases, via
`close_session_silently`. Pausing means "stop bothering me" (the same reading
`notify_progress` and `mark_episode_watched_inner` take of the pause flag), and
the episode is most likely still playing; the session only has to be dropped so
it cannot go stale.

Policy lives in the tracker, not in the pure function. On an `EndedPlayback` the
tracker reads the `up_next_min_watch_minutes` setting — one storage read per
session end, not per tick — and publishes only when
`watched_secs >= minutes * 60`:

```rust
EngineEvent::PlaybackEnded {
    anime_id: AnimeId,
    episode: EpisodeNumber,
    file_key: String,
    watched_secs: i64,
}
```

`up_next_min_watch_minutes` defaults to `5`; `0` means always prompt.

### 4. Resolving the next episode (`commands.rs`)

`get_up_next` and `get_up_next_inner` gain an optional `after: Option<i32>`.
The threshold passed to `up_next_from` becomes `after.unwrap_or(watched)`;
`up_next_from` itself — first non-ignored indexed file with an episode greater than
the threshold — is unchanged.

This is what makes rewatches work: on a completed show where episode 3 just ended,
`after: Some(3)` offers episode 4. When the episode is unknown the call omits
`after` and the old watched-progress behaviour applies.

### 5. Frontend (`lib/upNext.ts`, `lib/api.ts`, `App.svelte`, `lib/SettingsView.svelte`)

- `upNext.ts`: delete `latestProgressAdvance`; add `latestPlaybackEnded(events)`
  with the same last-one-wins scan, returning `{ anime_id, episode }`.
  `samePrompt` / `PromptKey` stay as the anti-double-toast guard.
- `api.ts`: add the `PlaybackEnded` variant to the `EngineEvent` union; `getUpNext`
  takes an optional `after` and always forwards `{ animeId, after }`, passing
  `null` when it is absent so the Rust side sees `None`.
- `App.svelte`: `maybePromptUpNext` triggers off `latestPlaybackEnded` and calls
  `getUpNext(anime_id, episode)`. The `up_next_toast_enabled` /
  `up_next_notification_enabled` checks, the toast markup, `playUpNext`,
  `dismissUpNext` and `notify_up_next` are unchanged.
- `SettingsView.svelte`: a number stepper for `up_next_min_watch_minutes`
  (default 5, range 0–60) beneath the two existing Up Next toggles.

Marking an episode watched by hand no longer prompts. `ProgressAdvanced` keeps
being published — history, sync and the Now Playing panel still consume it — it is
simply no longer an Up Next trigger.

### 6. Error handling

Unchanged in character: best-effort and silent. A failed settings read or
`get_up_next` lookup means no prompt for that session; the tracking loop keeps
running and no error toast is shown. Session state is loop-local, so a tracker
restart begins from a clean slate.

## Testing

Rust (`cargo test` in `next/src-tauri`):

- `advance_session`: same key across ticks emits nothing; key change ends the
  previous session with the correct `watched_secs`; one missed tick does not end a
  session; missed ticks beyond `grace_ticks` do; `watched_secs` excludes grace ticks.
- `up_next_from` with the threshold set to a just-played episode below recorded
  progress — the rewatch case on a completed show. `get_up_next_inner` only
  forwards `after.unwrap_or(watched)` into it, so it needs no separate test.
- `process_scan_result` returns a `SessionKey` for identified playback, `None` for
  unrecognised playback, and falls back to the window title as the key.

TypeScript (`npm run verify` in `next/`):

- `upNext.test.ts`: `latestPlaybackEnded` picks the last event in a batch, returns
  `null` for a batch with no `PlaybackEnded`; `samePrompt` unchanged.
- `api.test.ts`: `getUpNext` forwards the given `after`, and sends `null` when it
  is omitted.

## Out of scope

- Moving Up Next resolution or the notification into the engine (the "engine does
  the whole thing" option). The tracker stays out of toast policy.
- Auto-dismissing the toast on a timer.
- Prompting for low-confidence matches that were never auto-tracked.
