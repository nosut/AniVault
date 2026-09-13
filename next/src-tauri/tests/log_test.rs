use anivault_core::engine::log::build_log_appender;

#[test]
fn building_the_log_appender_prunes_old_daily_logs() {
    let dir = std::env::temp_dir().join(format!("anivault-log-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for day in 1..=20 {
        std::fs::write(dir.join(format!("anivault.log.2026-01-{day:02}")), "old").unwrap();
    }
    // Not ours: must survive pruning.
    std::fs::write(dir.join("notes.txt"), "keep").unwrap();

    let appender = build_log_appender(&dir).expect("appender builds");
    drop(appender);

    let logs = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("anivault.log"))
        .count();
    assert!(logs <= 14, "expected at most 14 log files, found {logs}");
    assert!(dir.join("notes.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
