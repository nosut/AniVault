use std::path::PathBuf;

use taiga_next::engine::events::EngineEvent;
use taiga_next::engine::runtime::{initialize_engine_at, sqlite_url_for_path};

#[test]
fn sqlite_url_for_path_normalizes_windows_separators() {
    let path = PathBuf::from(r"C:\Users\example\AppData\Roaming\AniVault\anivault.db");
    assert_eq!(
        sqlite_url_for_path(&path),
        "sqlite:///C:/Users/example/AppData/Roaming/AniVault/anivault.db"
    );
}

#[tokio::test]
async fn initialize_engine_creates_parent_dir_and_migrates_database() {
    let root = std::env::temp_dir().join(format!(
        "anivault-runtime-test-{}",
        std::process::id()
    ));
    let db_path = root.join("nested").join("anivault.db");

    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }

    let state = initialize_engine_at(db_path.clone()).await.unwrap();

    assert_eq!(state.database_path, db_path);
    assert!(state.database_path.exists());
    assert!(state.storage.migration_count().await.unwrap() >= 1);
    assert!(state.events.drain().is_empty());

    state.events.publish(EngineEvent::SyncQueued {
        service: "anilist".to_string(),
        anime_id: 1,
    });
    assert_eq!(state.events.drain().len(), 1);

    // Close pool before cleanup to release file lock on Windows
    state.storage.close().await;
    std::fs::remove_dir_all(root).unwrap();
}
