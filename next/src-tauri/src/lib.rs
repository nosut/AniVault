pub mod commands;
pub mod engine;

use crate::engine::runtime::initialize_engine_at;
use crate::engine::sync_worker;
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
            sync_worker::spawn_sync_worker(&state);
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
            commands::identify_file,
            commands::confirm_identification,
            commands::list_known_files,
            commands::store_anilist_token,
            commands::disconnect_anilist,
            commands::import_anilist_library,
            commands::get_sync_status,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Taiga Next");
}
