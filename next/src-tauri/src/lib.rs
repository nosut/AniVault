pub mod commands;
pub mod engine;

use crate::engine::runtime::initialize_engine_at;
use crate::engine::runtime::EngineState;
use crate::engine::sync_worker;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .setup(|app| {
            let database_path = app
                .path()
                .app_data_dir()
                .map_err(|error| Box::<dyn std::error::Error>::from(error))?
                .join("anivault.db");

            let state = tauri::async_runtime::block_on(
                initialize_engine_at(database_path.clone(), Some(app.handle().clone())),
            )
            .map_err(|error| Box::<dyn std::error::Error>::from(error))?;
            sync_worker::spawn_sync_worker(&state);
            app.manage(state);

            // Initialize file logging
            if let Ok(log_dir) = app.path().app_log_dir() {
                crate::engine::log::init_logging(&log_dir);
            }

            tracing::info!("AniVault engine initialized at {}", database_path.display());

            // Build tray menu
            let show_item = MenuItemBuilder::with_id("show", "Show AniVault").build(app)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let pause_item = MenuItemBuilder::with_id("pause", "Pause Tracking").build(app)?;
            let separator2 = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&separator)
                .item(&pause_item)
                .item(&separator2)
                .item(&quit_item)
                .build()?;

            let pause_handle = pause_item.clone();
            let _tray = TrayIconBuilder::new()
                .icon(tauri::image::Image::from_bytes(include_bytes!("../../../Icon.png")).unwrap())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| {
                    let id = event.id().as_ref();
                    match id {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "pause" => {
                            let state = app.state::<EngineState>();
                            let current = state
                                .tracking_paused
                                .load(std::sync::atomic::Ordering::Relaxed);
                            state
                                .tracking_paused
                                .store(!current, std::sync::atomic::Ordering::Relaxed);
                            let label = if current {
                                "Resume Tracking"
                            } else {
                                "Pause Tracking"
                            };
                            let _ = pause_handle.set_text(label);
                        }
                        "quit" => {
                            let confirmed = app
                                .dialog()
                                .message("Quit AniVault? Tracking will stop.")
                                .title("Quit AniVault")
                                .buttons(MessageDialogButtons::OkCancelCustom(
                                    "Quit".into(),
                                    "Cancel".into(),
                                ))
                                .blocking_show();
                            if confirmed {
                                app.exit(0);
                            }
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::backup_database,
            commands::confirm_identification,
            commands::connect_anilist_oauth,
            commands::connect_sonarr,
            commands::delete_setting,
            commands::disconnect_anilist,
            commands::disconnect_sonarr,
            commands::discover_v1_data,
            commands::drain_engine_events,
            commands::export_database,
            commands::fetch_anime_detail,
            commands::get_anilist_connection_status,
            commands::get_calendar,
            commands::get_statistics,
            commands::get_engine_status,
            commands::get_episode_files,
            commands::get_launch_on_startup,
            commands::get_library_folders,
            commands::get_library_stats,
            commands::get_session_state,
            commands::get_sonarr_availability,
            commands::get_sonarr_status,
            commands::get_setting,
            commands::get_sync_status,
            commands::get_tracking_status,
            commands::get_watch_history,
            commands::identify_file,
            commands::import_sonarr_series,
            commands::import_anilist_library,
            commands::import_database,
            commands::list_known_files,
            commands::list_recent_history,
            commands::open_episode_file,
            commands::mark_episode_watched,
            commands::preview_migration,
            commands::remap_sonarr,
            commands::restore_database,
            commands::run_migration,
            commands::scan_library_folders,
            commands::search_library,
            commands::set_launch_on_startup,
            commands::set_library_folders,
            commands::set_setting,
            commands::start_tracking,
            commands::stop_tracking,
            commands::store_anilist_token,
            commands::test_sonarr_connection,
            commands::toggle_pause_tracking,
            commands::update_list_entry,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run AniVault");
}
