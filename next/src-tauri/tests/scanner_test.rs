use taiga_next::engine::scanner::{scan_active_players, PlayerDef, ScannerConfig};

#[test]
fn empty_config_returns_empty_vec() {
    let config = ScannerConfig {
        known_players: vec![],
    };
    let results = scan_active_players(&config);
    assert!(results.is_empty());
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
