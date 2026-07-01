pub mod commands;
pub mod engine;

use crate::engine::runtime::initialize_engine_at;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let database_path = app
                .path()
                .app_data_dir()
                .map_err(|error| Box::<dyn std::error::Error>::from(error))?
                .join("anivault.db");

            let state = tauri::async_runtime::block_on(initialize_engine_at(database_path))
                .map_err(|error| Box::<dyn std::error::Error>::from(error))?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_engine_status,
            commands::preview_migration_report,
            commands::get_setting,
            commands::set_setting,
            commands::delete_setting,
            commands::drain_engine_events,
            commands::start_tracking,
            commands::stop_tracking,
            commands::get_tracking_status,
            commands::mark_episode_watched,
            commands::list_recent_history,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Taiga Next");
}
