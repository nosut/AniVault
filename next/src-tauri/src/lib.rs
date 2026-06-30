pub mod commands;
pub mod engine;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_engine_status,
            commands::preview_migration_report,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run AniVault");
}
