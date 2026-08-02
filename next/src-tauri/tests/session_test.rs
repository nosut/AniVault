use anivault_core::engine::events::EngineEvent;
use anivault_core::engine::runtime::{fresh_test_state, EngineState};
use anivault_core::engine::scanner::ScanResult;
use anivault_core::engine::session::process_scan_result;

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
        anivault_core::engine::events::EngineEvent::PlaybackDetected { .. }
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

    // Events: AnimeIdentified (auto-confirm) + ProgressAdvanced (history
    // recording, since 7fc141c8) + PlaybackDetected
    let events = state.events.drain();
    assert_eq!(
        events.len(),
        3,
        "should emit AnimeIdentified, ProgressAdvanced and PlaybackDetected"
    );

    let has_progress = events.iter().any(|e| {
        matches!(
            e,
            EngineEvent::ProgressAdvanced { anime_id: 1, new_episode: 1, .. }
        )
    });
    assert!(has_progress, "should emit ProgressAdvanced for the auto-recorded episode");

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

#[tokio::test]
async fn process_scan_result_records_watch_history_on_advance() {
    let state = make_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Cowboy Bebop")
        .await
        .unwrap();

    let result = make_scan_result("mpv.exe", "D:/Anime/Cowboy Bebop - 01.mkv", "Cowboy Bebop - 01");
    process_scan_result(&state, result).await.unwrap();

    let history = state.storage.list_recent_watch_history(10).await.unwrap();
    assert_eq!(history.len(), 1, "auto-detected playback should record a watch-history row");
    assert_eq!(history[0].anime_id, 1);
    assert_eq!(history[0].episode, 1);
    assert_eq!(history[0].player.as_deref(), Some("mpv.exe"));

    // Re-detecting the same episode (a later scan tick of the same file) must not
    // create a duplicate row — history advances once per newly-watched episode.
    let again = make_scan_result("mpv.exe", "D:/Anime/Cowboy Bebop - 01.mkv", "Cowboy Bebop - 01");
    process_scan_result(&state, again).await.unwrap();
    let history = state.storage.list_recent_watch_history(10).await.unwrap();
    assert_eq!(history.len(), 1, "re-detecting the same episode must not duplicate history");
}

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
