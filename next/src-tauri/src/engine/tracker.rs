use std::time::Duration;

use tokio::sync::watch;

use crate::engine::player_registry::builtin_player_registry;
use crate::engine::runtime::EngineState;
use crate::engine::scanner::{scan_active_players, ScannerConfig};
use crate::engine::session::process_scan_result;

pub async fn run_tracking_loop(
    state: EngineState,
    interval_ms: u64,
    cancel: watch::Receiver<bool>,
) {
    let config = ScannerConfig {
        known_players: builtin_player_registry(),
    };

    loop {
        if *cancel.borrow() {
            break;
        }

        let config_clone = config.clone();
        let results = tokio::task::spawn_blocking(move || scan_active_players(&config_clone))
            .await
            .unwrap_or_default();

        // Update watching status — lock scope must not span .await
        if let Some(result) = results.first() {
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

            if let Err(e) = process_scan_result(&state, result.clone()).await {
                eprintln!("session error: {e}");
            }
        } else {
            let mut ctrl = state.tracking.lock().unwrap();
            ctrl.watching = None;
        }

        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
    }

    // Cleanup
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
