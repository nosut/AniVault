use anivault_core::engine::detection::process::{is_known_player, scan_players};
use anivault_core::engine::detection::{DetectionDeduper, DetectionKey};

#[test]
fn known_players_are_detected_by_name() {
    assert!(is_known_player("mpv.exe"));
    assert!(is_known_player("mpc-hc64.exe"));
    assert!(is_known_player("vlc.exe"));
    assert!(is_known_player("PotPlayerMini64.exe"));
    assert!(is_known_player("Microsoft.Media.Player.exe"));
}

#[test]
fn unknown_processes_are_not_players() {
    assert!(!is_known_player("notepad.exe"));
    assert!(!is_known_player("explorer.exe"));
    assert!(!is_known_player("firefox.exe"));
}

#[test]
fn scan_players_never_panics_and_returns_player_names() {
    let results = scan_players();
    for player in results {
        assert!(is_known_player(&player.process_name));
    }
}

#[test]
fn deduper_rejects_same_key_within_window() {
    let mut deduper = DetectionDeduper::new(60);
    let key = DetectionKey::FilePath("D:/Anime/Spy x Family - 17.mkv".to_string());

    assert!(deduper.should_emit(&key, 1_000));
    assert!(!deduper.should_emit(&key, 1_030));
    assert!(deduper.should_emit(&key, 1_061));
}

#[test]
fn deduper_treats_different_keys_independently() {
    let mut deduper = DetectionDeduper::new(60);
    let first = DetectionKey::WindowTitle("mpv: Spy x Family - 17".to_string());
    let second = DetectionKey::WindowTitle("mpv: Frieren - 14".to_string());

    assert!(deduper.should_emit(&first, 1_000));
    assert!(deduper.should_emit(&second, 1_001));
}
