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
                initialize_engine_at(database_path, Some(app.handle().clone())),
            )
            .map_err(|error| Box::<dyn std::error::Error>::from(error))?;
            sync_worker::spawn_sync_worker(&state);
            app.manage(state);

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
            commands::confirm_identification,
            commands::delete_setting,
            commands::disconnect_anilist,
            commands::drain_engine_events,
            commands::fetch_anime_detail,
            commands::get_engine_status,
            commands::get_launch_on_startup,
            commands::get_library_stats,
            commands::get_session_state,
            commands::get_setting,
            commands::get_sync_status,
            commands::get_tracking_status,
            commands::identify_file,
            commands::import_anilist_library,
            commands::list_known_files,
            commands::list_recent_history,
            commands::mark_episode_watched,
            commands::preview_migration_report,
            commands::search_library,
            commands::set_launch_on_startup,
            commands::set_setting,
            commands::start_tracking,
            commands::stop_tracking,
            commands::store_anilist_token,
            commands::toggle_pause_tracking,
            commands::update_list_entry,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Taiga Next");
}
