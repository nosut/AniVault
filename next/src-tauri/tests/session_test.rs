use taiga_next::engine::runtime::{fresh_test_state, EngineState};
use taiga_next::engine::scanner::ScanResult;
use taiga_next::engine::session::process_scan_result;

fn make_scan_result(player: &str, file: &str, title: &str) -> ScanResult {
    ScanResult {
        player_name: player.to_string(),
        file_path: Some(file.to_string()),
        window_title: Some(title.to_string()),
        detected_at_unix: 1_782_769_000,
    }
}

async fn make_state() -> EngineState {
    fresh_test_state().await
}

#[tokio::test]
async fn process_scan_result_emits_playback_detected_event() {
    let state = make_state().await;
    let result = make_scan_result("mpv.exe", "D:/Anime/Show - 01.mkv", "Show - 01");

    process_scan_result(&state, result).await.unwrap();

    let events = state.events.drain();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        taiga_next::engine::events::EngineEvent::PlaybackDetected { .. }
    ));
}
