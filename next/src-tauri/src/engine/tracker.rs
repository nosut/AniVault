use std::sync::atomic::Ordering;
use std::time::Duration;

use tokio::sync::watch;

use crate::engine::events::EngineEvent;
use crate::engine::player_registry::builtin_player_registry;
use crate::engine::runtime::EngineState;
use crate::engine::scanner::{scan_active_players, ScannerConfig};
use crate::engine::session::{
    advance_session, passes_min_watch, process_scan_result, superseded_by, ActivePlayback,
    EndedPlayback, SessionKey,
};

/// Consecutive scan misses tolerated before a session is treated as ended: the
/// session ends on the *third* straight miss, ~6s at the 2s tick, enough to ride
/// out a tick or two that failed to recognise the player. The frontend then
/// drains events on its own 3s timer, so the prompt appears up to ~9s after the
/// player actually closed.
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
/// prompts. A missing setting falls back to the default silently — existing
/// installs have no row — but an unreadable or malformed one is logged, because
/// silently reverting to 5 minutes is otherwise indistinguishable from working.
pub(crate) async fn min_watch_minutes(state: &EngineState) -> i64 {
    let raw = match state.storage.get_setting("up_next_min_watch_minutes").await {
        Ok(Some(raw)) => raw,
        Ok(None) => return DEFAULT_MIN_WATCH_MINUTES,
        Err(e) => {
            tracing::warn!(
                "could not read up_next_min_watch_minutes ({e}); \
                 using {DEFAULT_MIN_WATCH_MINUTES} minutes"
            );
            return DEFAULT_MIN_WATCH_MINUTES;
        }
    };
    match serde_json::from_str::<i64>(&raw) {
        Ok(minutes) => minutes,
        Err(e) => {
            tracing::warn!(
                "up_next_min_watch_minutes is not a number ({raw:?}: {e}); \
                 using {DEFAULT_MIN_WATCH_MINUTES} minutes"
            );
            DEFAULT_MIN_WATCH_MINUTES
        }
    }
}

/// End an open session without telling anyone. Used when the user pauses
/// tracking or the loop shuts down: the session must not survive to go stale,
/// but neither is an Up Next prompt wanted — pausing means "stop bothering me",
/// the same reading `notify_progress` and `mark_episode_watched_inner` take of
/// the pause flag before they notify.
fn close_session_silently(session: &mut Option<ActivePlayback>) {
    *session = None;
}

pub(crate) async fn publish_playback_ended(state: &EngineState, ended: EndedPlayback) {
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
            // scanner deliberately stops looking — and without a prompt: the
            // episode is probably still playing, the user just wants quiet.
            close_session_silently(&mut session);
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
            // When a player advances itself through a playlist, one tick sees
            // episode 5 and the next sees episode 6: the session for 5 ends and
            // the session for 6 opens in the same call. Prompting for 6 while 6
            // is on screen would be noise, so let the new session speak for it.
            if !superseded_by(&ended, session.as_ref()) {
                publish_playback_ended(&state, ended).await;
            }
        }

        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
    }

    // Cleanup — the loop is being cancelled (app shutdown, or tracking switched
    // off in Settings). Drop the session so it cannot go stale, silently: the
    // user turned tracking off, they did not finish an episode.
    close_session_silently(&mut session);

    let mut ctrl = state.tracking.lock().unwrap();
    ctrl.active = false;
    ctrl.watching = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::set_setting_inner;
    use crate::engine::runtime::fresh_test_state;
    use crate::engine::session::ActivePlayback;

    #[tokio::test]
    async fn track_nothing_when_no_active_players() {
        let state = fresh_test_state().await;
        // With no active media players, run_tracking_loop would just loop
        // until cancelled. Here we just verify the state is valid.
        let ctrl = state.tracking.lock().unwrap();
        assert!(!ctrl.active);
        assert!(ctrl.watching.is_none());
    }

    fn ended(watched_secs: i64) -> EndedPlayback {
        EndedPlayback {
            anime_id: 1,
            episode: 5,
            file_key: "Cowboy Bebop - 05".to_string(),
            watched_secs,
        }
    }

    #[tokio::test]
    async fn min_watch_minutes_defaults_when_no_setting_row_exists() {
        // Existing installs have no row; the 5-minute default must still apply.
        let state = fresh_test_state().await;
        assert_eq!(min_watch_minutes(&state).await, DEFAULT_MIN_WATCH_MINUTES);
    }

    #[tokio::test]
    async fn min_watch_minutes_reads_back_what_settings_wrote() {
        // The cross-boundary contract: the Settings view writes a JSON number
        // through set_setting_inner, and the tracker parses it as an i64. If
        // those two ever disagree the parse fails and the threshold silently
        // reverts to 5 minutes.
        let state = fresh_test_state().await;
        set_setting_inner(
            "up_next_min_watch_minutes",
            serde_json::json!(12),
            &state,
        )
        .await
        .unwrap();
        assert_eq!(min_watch_minutes(&state).await, 12);
    }

    #[tokio::test]
    async fn min_watch_minutes_reads_back_a_zero() {
        let state = fresh_test_state().await;
        set_setting_inner("up_next_min_watch_minutes", serde_json::json!(0), &state)
            .await
            .unwrap();
        assert_eq!(
            min_watch_minutes(&state).await,
            0,
            "0 must survive the round trip — it means always prompt"
        );
    }

    #[tokio::test]
    async fn min_watch_minutes_falls_back_when_the_row_is_malformed() {
        let state = fresh_test_state().await;
        set_setting_inner(
            "up_next_min_watch_minutes",
            serde_json::json!("ten-ish"),
            &state,
        )
        .await
        .unwrap();
        assert_eq!(min_watch_minutes(&state).await, DEFAULT_MIN_WATCH_MINUTES);
    }

    #[tokio::test]
    async fn a_session_below_the_threshold_publishes_nothing() {
        let state = fresh_test_state().await;
        set_setting_inner("up_next_min_watch_minutes", serde_json::json!(5), &state)
            .await
            .unwrap();

        publish_playback_ended(&state, ended(299)).await;

        assert!(
            state.events.drain().is_empty(),
            "a session shorter than the configured minimum must not prompt"
        );
    }

    #[tokio::test]
    async fn a_session_past_the_threshold_publishes_exactly_one_event() {
        let state = fresh_test_state().await;
        set_setting_inner("up_next_min_watch_minutes", serde_json::json!(5), &state)
            .await
            .unwrap();

        publish_playback_ended(&state, ended(300)).await;

        let events = state.events.drain();
        assert_eq!(events.len(), 1, "exactly one PlaybackEnded");
        match &events[0] {
            EngineEvent::PlaybackEnded {
                anime_id,
                episode,
                file_key,
                watched_secs,
            } => {
                assert_eq!(*anime_id, 1);
                assert_eq!(*episode, 5);
                assert_eq!(file_key, "Cowboy Bebop - 05");
                assert_eq!(*watched_secs, 300);
            }
            other => panic!("expected PlaybackEnded, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn the_default_threshold_gates_a_short_session_without_a_setting_row() {
        let state = fresh_test_state().await;

        publish_playback_ended(&state, ended(299)).await;
        assert!(state.events.drain().is_empty(), "under the 5-minute default");

        publish_playback_ended(&state, ended(301)).await;
        assert_eq!(state.events.drain().len(), 1, "over the 5-minute default");
    }

    #[test]
    fn closing_silently_drops_the_session_without_an_ended_playback() {
        let mut session = Some(ActivePlayback {
            key: SessionKey {
                anime_id: 1,
                episode: 5,
                file_key: "Cowboy Bebop - 05".to_string(),
            },
            started_at: 100,
            last_seen_at: 1300,
            missed_ticks: 0,
        });

        close_session_silently(&mut session);

        assert!(
            session.is_none(),
            "the session is closed so it cannot go stale"
        );
    }
}
