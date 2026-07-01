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
            commands::get_seasonal_anime,
            commands::get_watching_anime,
            commands::open_url,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run AniVault");
}

fn start_local_tracking(runtime: commands::TrackingRuntime) {
    tauri::async_runtime::spawn(async move {
        let database_url = match engine::database_url() {
            Ok(url) => url,
            Err(_) => return,
        };
        let Ok(storage) = engine::storage::Storage::connect(&database_url).await else {
            return;
        };
        if storage.migrate().await.is_err() {
            return;
        }
        if engine::recognition::matcher::build_fts_index(&storage).await.is_err() {
            return;
        }

        // Try importing from existing Taiga installation
        try_import_from_taiga(&storage).await;

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

async fn try_import_from_taiga(storage: &engine::storage::Storage) {
    use sqlx::sqlite::SqlitePoolOptions;

    let app_data = match std::env::var_os("APPDATA") {
        Some(p) => std::path::PathBuf::from(p),
        None => return,
    };

    // Try multiple possible Taiga DB paths
    let candidates = [
        app_data.join("Taiga").join("data").join("Taiga.db"),
        app_data.join("Taiga").join("data").join("taiga.db"),
    ];

    let taiga_db = match candidates.iter().find(|p| p.exists()) {
        Some(p) => p.clone(),
        None => return,
    };

    // Count existing anime to avoid re-import
    let existing: i64 = sqlx::query("SELECT COUNT(*) FROM anime")
        .fetch_one(storage.pool())
        .await
        .map(|r| r.get(0))
        .unwrap_or(0);
    if existing > 0 {
        return; // already populated
    }

    let taiga_path = taiga_db.to_string_lossy().to_string();
    let Ok(taiga_pool) = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&taiga_path)
                .read_only(true),
        )
        .await else {
        return;
    };

    use sqlx::Row;

    // Try primary schema, fall back to alternate column names
    let primary = sqlx::query("SELECT id, title, watched_episodes FROM anime")
        .fetch_all(&taiga_pool)
        .await;

    let rows = match primary {
        Ok(r) if !r.is_empty() => r,
        _ => {
            match sqlx::query("SELECT id, title, COALESCE(progress, 0) as watched_episodes FROM anime")
                .fetch_all(&taiga_pool)
                .await
            {
                Ok(r) => r,
                Err(_) => return,
            }
        }
    };

    let snapshot = engine::migration::TaigaSnapshot {
        anime: rows
            .iter()
            .map(|r| engine::migration::TaigaAnime {
                id: r.get(0),
                title: r.get(1),
                watched_episodes: r.get(2),
            })
            .collect(),
    };

    let _ = engine::migration::import_taiga_snapshot(storage, snapshot).await;
}
