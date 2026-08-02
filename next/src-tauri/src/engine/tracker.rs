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
/// the 2s tick this is ~4s, enough to ride out a window-title glitch mid-seek.
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

#[cfg(test)]
mod tests {
    use crate::engine::runtime::fresh_test_state;

    #[tokio::test]
    async fn track_nothing_when_no_active_players() {
        let state = fresh_test_state().await;
        // With no active media players, run_tracking_loop would just loop
        // until cancelled. Here we just verify the state is valid.
        let ctrl = state.tracking.lock().unwrap();
        assert!(!ctrl.active);
        assert!(ctrl.watching.is_none());
    }
}
