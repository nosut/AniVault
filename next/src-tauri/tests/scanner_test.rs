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

#[test]
fn pid_enumeration_grows_the_buffer_until_every_process_fits() {
    use anivault_core::engine::scanner::enumerate_pids_with;
    // 3000 running processes: a 1024-slot buffer comes back completely full.
    let running: Vec<u32> = (1..=3000).collect();
    let mut calls = 0;
    let (pids, ok) = enumerate_pids_with(|buf: &mut [u32]| {
        calls += 1;
        let n = running.len().min(buf.len());
        buf[..n].copy_from_slice(&running[..n]);
        Some((n * std::mem::size_of::<u32>()) as u32)
    });
    assert!(ok);
    assert_eq!(pids, running, "no process is silently dropped");
    assert!(calls >= 2, "the first full buffer triggered a retry");
}

#[test]
fn pid_enumeration_reports_failure() {
    use anivault_core::engine::scanner::enumerate_pids_with;
    let (pids, ok) = enumerate_pids_with(|_buf: &mut [u32]| None);
    assert!(!ok);
    assert!(pids.is_empty());
}
