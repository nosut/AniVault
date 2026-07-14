//! Database backup, restore, export, and import.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::storage::Storage;

// ── Backup / Restore ─────────────────────────────────────────────────────────

/// Copy the current database file to a timestamped backup.
/// Returns the backup file path.
pub async fn backup_database(storage: &Storage) -> anyhow::Result<String> {
    let db_path = storage.database_path().to_owned();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup_path = format!("{}.backup.{}", db_path, timestamp);

    // Close pool, copy, reopen (sqlx doesn't expose the file handle)
    // For safety: copy while pool is alive by using WAL checkpoint
    storage.wal_checkpoint().await?;
    std::fs::copy(&db_path, &backup_path)?;

    Ok(backup_path)
}

/// Restore the database from a backup file.
/// This will close the current pool, replace the DB file, and requires app restart.
/// Returns the backup path that was restored.
pub async fn restore_database(
    storage: &Storage,
    backup_path: &str,
) -> anyhow::Result<String> {
    let db_path = storage.database_path().to_owned();

    // Verify backup exists and is actually a SQLite database before touching
    // the live DB — a wrong or corrupt path must not destroy working data.
    if !std::path::Path::new(backup_path).exists() {
        anyhow::bail!("Backup file not found: {}", backup_path);
    }
    verify_sqlite_file(backup_path)?;

    // Safety net: back up the *current* (pre-restore) database so a restore
    // from the wrong backup, or a change of mind, is itself reversible.
    let pre_restore_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let pre_restore_path = format!("{}.pre-restore.{}", db_path, pre_restore_timestamp);
    storage.wal_checkpoint().await?;
    std::fs::copy(&db_path, &pre_restore_path)?;

    // Close pool, then replace DB file.
    storage.close().await;

    // Replace DB file. Remove stale WAL/SHM sidecars so leftover journal pages
    // from the old database can't be replayed over the restored file.
    std::fs::copy(backup_path, &db_path)?;
    let _ = std::fs::remove_file(format!("{db_path}-wal"));
    let _ = std::fs::remove_file(format!("{db_path}-shm"));

    Ok(format!(
        "Database restored from {}. Previous database saved to {}. Restart required.",
        backup_path, pre_restore_path
    ))
}

/// Check the file starts with SQLite's 16-byte magic header, rejecting
/// anything that clearly isn't a SQLite database before we copy it over the
/// live DB.
fn verify_sqlite_file(path: &str) -> anyhow::Result<()> {
    const SQLITE_HEADER: &[u8] = b"SQLite format 3\0";
    let mut file = std::fs::File::open(path)?;
    let mut header = [0u8; 16];
    use std::io::Read;
    file.read_exact(&mut header)
        .map_err(|_| anyhow::anyhow!("{} is too small to be a SQLite database", path))?;
    if header != SQLITE_HEADER {
        anyhow::bail!("{} does not look like a SQLite database file", path);
    }
    Ok(())
}

// ── Export / Import ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DatabaseExport {
    pub exported_at: i64,
    pub version: String,
    pub anime: Vec<AnimeExport>,
    pub list_entries: Vec<ListEntryExport>,
    pub watch_history: Vec<WatchHistoryExport>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnimeExport {
    pub id: i64,
    pub titles_json: String,
    pub anime_type: Option<String>,
    pub status: Option<String>,
    pub episode_count: Option<i32>,
    pub image_url: Option<String>,
    pub synopsis: Option<String>,
    pub last_modified: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ListEntryExport {
    pub anime_id: i64,
    pub status: String,
    pub watched_episodes: i32,
    pub score: Option<i32>,
    pub notes: Option<String>,
    pub date_started: Option<String>,
    pub date_completed: Option<String>,
    pub local_updated: i64,
    pub remote_updated: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WatchHistoryExport {
    pub anime_id: i64,
    pub episode: i32,
    pub file_path: Option<String>,
    pub player: Option<String>,
    pub watched_at: i64,
    pub source: String,
}

/// Export all database contents as JSON string.
pub async fn export_database(storage: &Storage) -> anyhow::Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let anime = storage.export_all_anime().await?;
    let list_entries = storage.export_all_list_entries().await?;
    let watch_history = storage.export_all_watch_history().await?;

    let export = DatabaseExport {
        exported_at: now,
        version: "1.0".into(),
        anime,
        list_entries,
        watch_history,
    };

    Ok(serde_json::to_string_pretty(&export)?)
}

/// Import a database export JSON string into the current DB.
/// Uses upsert semantics (INSERT OR REPLACE).
pub async fn import_database(
    storage: &Storage,
    json: &str,
) -> anyhow::Result<super::importer::MigrationReport> {
    let export: DatabaseExport = serde_json::from_str(json)?;

    let mut report = super::importer::MigrationReport::default();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    for anime in &export.anime {
        let ep_count = anime.episode_count.unwrap_or(0);
        storage
            .upsert_anime(
                anime.id,
                &anime.titles_json,
                ep_count,
                anime.image_url.as_deref(),
                anime.last_modified,
            )
            .await?;
        report.imported_anime += 1;
    }

    for entry in &export.list_entries {
        storage
            .upsert_list_entry_full(
                entry.anime_id,
                &entry.status,
                entry.watched_episodes,
                entry.score,
                entry.notes.as_deref().unwrap_or(""),
                now,
                entry.remote_updated.unwrap_or(0),
            )
            .await?;
        report.imported_entries += 1;
    }

    for wh in &export.watch_history {
        storage
            .append_watch_history(
                wh.anime_id,
                wh.episode,
                wh.file_path.as_deref(),
                wh.player.as_deref(),
                &wh.source,
                wh.watched_at,
            )
            .await?;
        report.imported_history += 1;
    }

    Ok(report)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::storage::Tests;

    #[tokio::test]
    async fn export_empty_database() {
        let storage = Tests::new_in_memory().await;
        let json = export_database(&storage).await.unwrap();
        let export: DatabaseExport = serde_json::from_str(&json).unwrap();
        assert!(export.anime.is_empty());
        assert_eq!(export.version, "1.0");
    }

    #[tokio::test]
    async fn export_then_import_roundtrip() {
        let storage = Tests::new_in_memory().await;

        // Insert test data
        let titles = serde_json::json!({"romaji": "Test", "english": null, "japanese": null, "synonyms": []}).to_string();
        storage.upsert_anime(1, &titles, 12, None, 1000).await.unwrap();
        storage.upsert_list_entry_full(1, "watching", 5, Some(80), "", 2000, 0).await.unwrap();
        storage.append_watch_history(1, 1, None, Some("test"), "manual", 3000).await.unwrap();

        // Export
        let json = export_database(&storage).await.unwrap();

        // Fresh DB
        let storage2 = Tests::new_in_memory().await;

        // Import
        let report = import_database(&storage2, &json).await.unwrap();
        assert_eq!(report.imported_anime, 1);
        assert_eq!(report.imported_entries, 1);
        assert_eq!(report.imported_history, 1);

        // Verify
        let anime = storage2.fetch_anime(1).await.unwrap();
        assert!(anime.is_some());

        let entry = storage2.get_list_entry_full(1).await.unwrap();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().status, "watching");

        let history = storage2.list_recent_watch_history(10).await.unwrap();
        assert_eq!(history.len(), 1);
    }

    #[tokio::test]
    async fn restore_rejects_non_sqlite_file() {
        let storage = Tests::new_in_memory().await;
        let bogus_path = std::env::temp_dir().join(format!(
            "anivault-test-bogus-{}.db",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::write(&bogus_path, b"not a sqlite database").unwrap();

        let result = restore_database(&storage, bogus_path.to_str().unwrap()).await;

        std::fs::remove_file(&bogus_path).ok();
        assert!(result.is_err(), "restoring a non-SQLite file must be rejected");
    }

    #[tokio::test]
    async fn restore_takes_a_safety_backup_of_the_current_db_first() {
        // Use a real file-backed database (not :memory:) so backup/restore's
        // file-copy logic has an actual file to operate on.
        let db_path = std::env::temp_dir().join(format!(
            "anivault-test-restore-{}.db",
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let db_url = format!("sqlite:{}", db_path.to_str().unwrap());
        let storage = Storage::connect(&db_url).await.unwrap();
        storage.migrate().await.unwrap();
        storage.insert_minimal_anime(1, "Original DB").await.unwrap();

        // Make a valid backup file to restore *from*.
        let good_backup_path = backup_database(&storage).await.unwrap();

        // Mutate the live DB so we can tell a safety backup captured the
        // pre-restore state.
        storage.insert_minimal_anime(2, "Changed before restore").await.unwrap();

        restore_database(&storage, &good_backup_path).await.unwrap();

        // A safety backup of the pre-restore state must exist on disk,
        // distinct from the backup we restored from.
        let db_dir = db_path.parent().unwrap();
        let stem = db_path.file_name().unwrap().to_str().unwrap();
        let safety_backups: Vec<_> = std::fs::read_dir(db_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with(stem) && name.contains(".pre-restore.")
            })
            .collect();
        assert!(
            !safety_backups.is_empty(),
            "expected a .pre-restore. safety backup file next to {}",
            db_path.display()
        );

        // Cleanup
        std::fs::remove_file(&db_path).ok();
        std::fs::remove_file(&good_backup_path).ok();
        for f in safety_backups {
            std::fs::remove_file(f.path()).ok();
        }
    }
}
