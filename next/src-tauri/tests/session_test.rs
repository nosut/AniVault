use taiga_next::engine::events::EngineEvent;
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

#[tokio::test]
async fn process_scan_result_auto_confirms_high_confidence() {
    let state = make_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Cowboy Bebop")
        .await
        .unwrap();

    // "Cowboy Bebop - 01" will be parsed to title "Cowboy Bebop" (100% match) + episode 1
    let result = make_scan_result(
        "mpv.exe",
        "D:/Anime/Cowboy Bebop - 01.mkv",
        "Cowboy Bebop - 01",
    );

    process_scan_result(&state, result).await.unwrap();

    // Events: AnimeIdentified (auto-confirm) + PlaybackDetected
    let events = state.events.drain();
    assert_eq!(events.len(), 2, "should emit both AnimeIdentified and PlaybackDetected");

    let has_identified = events.iter().any(|e| {
        matches!(
            e,
            EngineEvent::AnimeIdentified(ev)
                if ev.anime_id == 1 && ev.episode == 1
        )
    });
    assert!(has_identified, "should emit AnimeIdentified for auto-confirm");

    let has_playback = events.iter().any(|e| {
        matches!(e, EngineEvent::PlaybackDetected { .. })
    });
    assert!(has_playback, "should also emit PlaybackDetected");

    // File index should be written by auto-confirm
    let idx = state
        .storage
        .get_file_index("D:/Anime/Cowboy Bebop - 01.mkv")
        .await
        .unwrap()
        .expect("file index should be written by auto-confirm");
    assert_eq!(idx.anime_id, Some(1));
    assert_eq!(idx.episode, Some(1));
    assert_eq!(idx.confidence, 100);
}
