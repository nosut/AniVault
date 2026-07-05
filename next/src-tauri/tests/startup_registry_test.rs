use anivault_core::commands::desired_run_value;

const EXE: &str = r"C:\Program Files\AniVault\anivault.exe";

#[test]
fn disabled_means_no_registry_entry() {
    assert_eq!(desired_run_value(false, false, EXE), None);
    assert_eq!(desired_run_value(false, true, EXE), None);
}

#[test]
fn enabled_quotes_the_exe_path() {
    assert_eq!(
        desired_run_value(true, false, EXE),
        Some(format!("\"{EXE}\""))
    );
}

#[test]
fn enabled_with_tray_appends_minimized_flag() {
    assert_eq!(
        desired_run_value(true, true, EXE),
        Some(format!("\"{EXE}\" --minimized"))
    );
}
