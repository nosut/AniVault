pub mod commands;
pub mod engine;

use std::sync::Arc;

pub fn run() {
    let tracking_runtime = commands::TrackingRuntime::default();
    let oauth_runtime = commands::OAuthRuntime::default();
    tauri::Builder::default()
        .manage(tracking_runtime.clone())
        .manage(oauth_runtime)
        .setup(move |_app| {
            start_local_tracking(tracking_runtime.clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_engine_status,
            commands::preview_migration_report,
            commands::get_tracking_status,
            commands::app_exit,
            commands::start_oauth,
            commands::complete_oauth,
            commands::get_oauth_status,
            commands::get_sync_status,
            commands::set_watched_episodes,
            commands::get_pending_matches,
            commands::confirm_match,
            commands::reject_match,
            commands::get_library_anime,
            commands::get_sonarr_config,
            commands::set_sonarr_config,
            commands::test_sonarr_connection,
            commands::get_sonarr_mappings,
            commands::map_sonarr_series,
            commands::set_sonarr_monitored,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run AniVault");
}

fn start_local_tracking(runtime: commands::TrackingRuntime) {
    tauri::async_runtime::spawn(async move {
        let database_url = local_database_url();
        let Ok(storage) = engine::storage::Storage::connect(&database_url).await else {
            return;
        };
        if storage.migrate().await.is_err() {
            return;
        }
        if engine::recognition::matcher::build_fts_index(&storage).await.is_err() {
            return;
        }

        let bus = engine::event_bus::EventBus::default();
        let status_runtime = runtime.clone();
        engine::orchestrator::start_tracking_loop_with_status(
            bus.clone(),
            storage.clone(),
            Arc::new(move |anime, anime_id, episode| status_runtime.set_tracking_info(anime, anime_id, episode)),
        );
        let _sync = engine::sync::SyncManager::start(storage.clone());
        let _manager = engine::detection::DetectionManager::start(
            bus,
            engine::settings::detection_config_from_settings("").unwrap_or_default(),
        );
        runtime.mark_running();
    });
}

fn local_database_url() -> String {
    let app_data = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    let directory = app_data.join("AniVault");
    let _ = std::fs::create_dir_all(&directory);
    let normalized = directory.join("anivault.db").to_string_lossy().replace('\\', "/");
    format!("sqlite:///{}", normalized)
}
