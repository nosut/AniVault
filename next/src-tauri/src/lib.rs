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

/// Handle to the tray's "Pause Tracking" menu item so the label can be kept in
/// sync when tracking is paused/resumed from the UI rather than the tray.
pub struct TrayPauseItem(pub tauri::menu::MenuItem<tauri::Wry>);

/// Update the tray pause/resume label to match the current paused state.
/// No-op when the tray isn't set up (e.g. tests).
pub fn update_tray_pause_label(app: &tauri::AppHandle, paused: bool) {
    if let Some(item) = app.try_state::<TrayPauseItem>() {
        let label = if paused { "Resume Tracking" } else { "Pause Tracking" };
        let _ = item.0.set_text(label);
    }
}

pub fn run() {
    tauri::Builder::default()
        // Single-instance MUST be the first plugin registered. When a second
        // launch is attempted, this callback runs in the already-running instance
        // and brings its window forward instead of opening a duplicate.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .setup(|app| {
            // Initialize file logging first (before engine, so all startup is logged).
            // Fall back to the app-data dir if the platform log dir can't be resolved,
            // rather than silently running with no logging at all.
            let log_dir = app
                .path()
                .app_log_dir()
                .or_else(|_| app.path().app_data_dir().map(|d| d.join("logs")));
            match log_dir {
                Ok(dir) => crate::engine::log::init_logging(&dir),
                Err(e) => eprintln!("[AniVault] could not resolve a log directory: {e}"),
            }

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
            engine::library_watcher::spawn_library_scan_worker(&state);
            engine::library_watcher::spawn_library_watcher(&state);

            // Auto-start playback tracking on launch unless the user disabled it.
            {
                let s = state.clone();
                tauri::async_runtime::spawn(async move {
                    // One-time cleanup of any bogus window-title "paths" stored by
                    // older builds (they shadowed real mappings).
                    if let Ok(n) = s.storage.delete_pathless_file_index().await {
                        if n > 0 {
                            tracing::info!("Removed {n} bogus (pathless) file-index rows");
                        }
                    }

                    // Self-heal the Windows launch-on-startup registry entry so it
                    // always points at the exe that's actually running (repairs a
                    // stale path from a reinstall / identifier change).
                    commands::reconcile_startup_registry(&s).await;

                    let enabled = s
                        .storage
                        .get_setting("tracking.enabled")
                        .await
                        .ok()
                        .flatten()
                        .map(|v| v != "false")
                        .unwrap_or(true);
                    if enabled {
                        let _ = commands::start_tracking_inner(&s).await;
                    }
                });
            }

            app.manage(state);

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

            // Expose the pause item so UI-driven toggles can update its label too.
            app.manage(TrayPauseItem(pause_item.clone()));

            let _tray = TrayIconBuilder::new()
                // Crisp 32px tray icon (extracted from the multi-size icon.ico).
                // Using the full-size PNG here made Windows downscale it to tray
                // size on the fly, which looked pixelated.
                .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png")).unwrap())
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
                            update_tray_pause_label(app, !current);
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

            // Show the main window unless launched with --minimized (start in tray).
            // The window is created hidden (visible: false) to avoid a flash.
            let start_minimized = std::env::args().any(|a| a == "--minimized");
            if !start_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::backup_database,
            commands::continue_watching,
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
            commands::get_anime_relations,
            commands::get_calendar,
            commands::queue_anilist_sync,
            commands::get_statistics,
            commands::get_engine_status,
            commands::check_for_update,
            commands::get_episode_files,
            commands::get_future_anime,
            commands::get_ready_to_watch,
            commands::get_season_anime,
            commands::search_sonarr_episode,
            commands::get_launch_on_startup,
            commands::get_library_folders,
            commands::get_library_ids,
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
            commands::list_sonarr_series,
            commands::import_anilist_library,
            commands::import_database,
            commands::list_known_files,
            commands::set_known_file_ignored,
            commands::delete_known_file,
            commands::set_known_file_mapping,
            commands::set_known_file_mappings,
            commands::set_known_files_ignored,
            commands::delete_known_files,
            commands::unmap_known_files,
            commands::pick_folder,
            commands::map_folder_to_anime,
            commands::import_anilist_anime,
            commands::deep_match_via_anilist,
            commands::delete_anime,
            commands::get_next_airing,
            commands::list_recent_history,
            commands::open_episode_file,
            commands::open_containing_folder,
            commands::mark_episode_watched,
            commands::preview_migration,
            commands::remap_sonarr,
            commands::rematch_unmapped_files,
            commands::repair_anime_file_mappings,
            commands::rescan_anime_files,
            commands::restore_database,
            commands::run_migration,
            commands::scan_library_folders,
            commands::search_anime,
            commands::search_library,
            commands::set_launch_on_startup,
            commands::get_start_in_tray,
            commands::set_start_in_tray,
            commands::set_library_folders,
            commands::set_setting,
            commands::start_tracking,
            commands::stop_tracking,
            commands::store_anilist_token,
            commands::test_sonarr_connection,
            commands::toggle_pause_tracking,
            commands::trigger_sync,
            commands::update_list_entry,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run AniVault");
}
