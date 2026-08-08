use anivault_core::engine::scanner::{scan_active_players, PlayerDef, ScannerConfig};

#[test]
fn empty_config_returns_no_players() {
    let config = ScannerConfig {
        known_players: vec![],
    };
    let scan = scan_active_players(&config);
    assert!(scan.players.is_empty());
    assert!(
        scan.enumerated,
        "with nothing trackable, an empty result is a fact the tracker may act on"
    );
}

#[test]
fn config_accepts_player_definitions() {
    let config = ScannerConfig {
        known_players: vec![
            PlayerDef {
                process_name: "mpv.exe".to_string(),
                window_title_hint: None,
            },
        ],
    };
    assert_eq!(config.known_players.len(), 1);
}
