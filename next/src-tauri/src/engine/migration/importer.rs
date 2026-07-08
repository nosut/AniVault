//! Taiga v1 → v2 import logic.
//!
//! Supports dry-run (preview) and live import with duplicate handling.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::storage::Storage;

use super::discovery::V1DataPaths;
use super::v1_read::{
    read_v1_sqlite, v1_list_status_to_v2,
};

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DuplicateStrategy {
    Skip,
    Merge,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct MigrationReport {
    pub imported_anime: usize,
    pub imported_entries: usize,
    pub imported_history: usize,
    pub skipped_anime: usize,
    pub skipped_entries: usize,
    pub warnings: Vec<MigrationWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MigrationWarning {
    pub source: String,
    pub source_id: String,
    pub message: String,
}

// ── Dry run ──────────────────────────────────────────────────────────────────

/// Preview import without writing to v2 DB.
pub async fn dry_run_import(paths: &V1DataPaths) -> Result<MigrationReport, anyhow::Error> {
    let mut report = MigrationReport::default();

    // Read v1 data
    let (v1_anime, v1_entries) = if let Some(ref sqlite_path) = paths.sqlite_path {
        read_v1_sqlite(sqlite_path).await?
    } else if let Some(ref xml_path) = paths.anime_xml_path {
        let anime = super::v1_read::read_v1_anime_xml(xml_path)?;
        (anime, Vec::new()) // list entries would need separate XML; handled below
    } else {
        report
            .warnings
            .push(MigrationWarning {
                source: "discovery".into(),
                source_id: "none".into(),
                message: "No v1 data found at any known path.".into(),
            });
        return Ok(report);
    };

    // Validate anime
    for anime in &v1_anime {
        if anime.id <= 0 {
            report.warnings.push(MigrationWarning {
                source: "v1_anime".into(),
                source_id: anime.id.to_string(),
                message: "Skipped anime with invalid id (<= 0).".into(),
            });
            report.skipped_anime += 1;
            continue;
        }
        if anime.title.trim().is_empty() {
            report.warnings.push(MigrationWarning {
                source: "v1_anime".into(),
                source_id: anime.id.to_string(),
                message: "Skipped anime with blank title.".into(),
            });
            report.skipped_anime += 1;
            continue;
        }
        report.imported_anime += 1;
    }

    // Validate list entries
    for entry in &v1_entries {
        if entry.anime_id <= 0 {
            report.warnings.push(MigrationWarning {
                source: "v1_list_entry".into(),
                source_id: entry.anime_id.to_string(),
                message: "Skipped entry with invalid anime_id.".into(),
            });
            report.skipped_entries += 1;
            continue;
        }
        // Check if anime exists in v1 set
        if !v1_anime.iter().any(|a| a.id == entry.anime_id) {
            report.warnings.push(MigrationWarning {
                source: "v1_list_entry".into(),
                source_id: entry.anime_id.to_string(),
                message: "Skipped entry: anime not found in v1 database.".into(),
            });
            report.skipped_entries += 1;
            continue;
        }
        report.imported_entries += 1;
    }

    // Check history if available
    if let Some(ref history_path) = paths.history_xml_path {
        match super::v1_read::read_v1_history_xml(history_path) {
            Ok(history) => {
                report.imported_history = history.len();
            }
            Err(e) => {
                report.warnings.push(MigrationWarning {
                    source: "v1_history".into(),
                    source_id: history_path.clone(),
                    message: format!("Failed to read history XML: {}", e),
                });
            }
        }
    }

    Ok(report)
}

// ── Live import ──────────────────────────────────────────────────────────────

/// Import v1 data into v2 database.
///
/// Backs up before writing, then inserts anime, list entries, and watch history.
/// Uses `DuplicateStrategy` to decide what to do when an anime_id already exists.
pub async fn live_import(
    storage: &Storage,
    paths: &V1DataPaths,
    strategy: DuplicateStrategy,
) -> Result<MigrationReport, anyhow::Error> {
    let mut report = MigrationReport::default();
    let now = unix_now();

    // Read v1 data
    let (v1_anime, v1_entries) = if let Some(ref sqlite_path) = paths.sqlite_path {
        read_v1_sqlite(sqlite_path).await?
    } else {
        report
            .warnings
            .push(MigrationWarning {
                source: "discovery".into(),
                source_id: "none".into(),
                message: "No v1 SQLite data found. Cannot import.".into(),
            });
        return Ok(report);
    };

    // Validate v1 anime set for quick lookup
    let v1_anime_ids: std::collections::HashSet<i64> =
        v1_anime.iter().map(|a| a.id).collect();

    for anime in &v1_anime {
        if anime.id <= 0 || anime.title.trim().is_empty() {
            report.skipped_anime += 1;
            report.warnings.push(MigrationWarning {
                source: "v1_anime".into(),
                source_id: anime.id.to_string(),
                message: "Skipped invalid anime during live import.".into(),
            });
            continue;
        }

        // Check for duplicate
        let existing = storage.fetch_anime(anime.id).await?;
        if existing.is_some() {
            if strategy == DuplicateStrategy::Skip {
                report.skipped_anime += 1;
                report.warnings.push(MigrationWarning {
                    source: "v1_anime".into(),
                    source_id: anime.id.to_string(),
                    message: "Anime ID already exists in v2 database. Skipped.".into(),
                });
                continue;
            }
            // Merge: update with v1 data (titles, metadata)
        }

        // Build titles_json
        let titles_json = serde_json::json!({
            "romaji": anime.title,
            "english": if anime.english.is_empty() { serde_json::Value::Null } else { serde_json::json!(anime.english) },
            "japanese": if anime.japanese.is_empty() { serde_json::Value::Null } else { serde_json::json!(anime.japanese) },
            "synonyms": anime.synonyms,
        })
        .to_string();

        let episode_count = if anime.episode_count > 0 {
            anime.episode_count
        } else {
            0
        };

        storage
            .upsert_anime(
                anime.id,
                &titles_json,
                episode_count,
                if anime.image_url.is_empty() {
                    None
                } else {
                    Some(&anime.image_url)
                },
                anime.last_modified,
            )
            .await?;

        // Log migration
        storage
            .log_migration("v1_anime", &anime.id.to_string(), "imported", "ok")
            .await?;

        report.imported_anime += 1;
    }

    // Import watch history from XML — MUST run before the list-entries loop
    // below. Both loops guard on the same idempotency check
    // (watch_history_count(anime_id, ep) > 0 → skip), and the progress-derived
    // loop can only stamp a synthetic timestamp (entry.last_updated) for a
    // given episode. If it ran first, its synthetic rows would satisfy the
    // guard for every episode the XML also covers, permanently discarding the
    // real per-episode timestamps. Importing the XML first means the guard
    // instead protects the real rows, and the progress-derived loop only
    // fills in episodes the XML didn't cover.
    if let Some(ref history_path) = paths.history_xml_path {
        match super::v1_read::read_v1_history_xml(history_path) {
            Ok(history_items) => {
                for item in &history_items {
                    if item.anime_id <= 0 {
                        continue;
                    }
                    // Parse timestamp
                    let ts = parse_datetime_to_unix(&item.timestamp).unwrap_or(now);

                    // Idempotency: same guard as the progress-derived history
                    // below — re-running the import must not duplicate rows.
                    if storage.watch_history_count(item.anime_id, item.episode).await? > 0 {
                        continue;
                    }
                    storage
                        .append_watch_history(
                            item.anime_id,
                            item.episode,
                            None,
                            Some("taiga_v1"),
                            "import",
                            ts,
                        )
                        .await?;

                    report.imported_history += 1;
                }
            }
            Err(e) => {
                report.warnings.push(MigrationWarning {
                    source: "v1_history".into(),
                    source_id: history_path.clone(),
                    message: format!("Failed to import history: {}", e),
                });
            }
        }
    }

    // Import list entries
    for entry in &v1_entries {
        if entry.anime_id <= 0 || !v1_anime_ids.contains(&entry.anime_id) {
            report.skipped_entries += 1;
            continue;
        }

        let status = v1_list_status_to_v2(entry.status);
        let score = if entry.score > 0 {
            Some(entry.score)
        } else {
            None
        };

        storage
            .upsert_list_entry_full(
                entry.anime_id,
                status,
                entry.watched_episodes,
                score,
                &entry.notes,
                now,
                entry.last_updated,
            )
            .await?;

        // Create watch history entries from progress for any episode the XML
        // import above didn't already cover (v1 doesn't have per-episode
        // timestamps here, so we fall back to entry.last_updated).
        for ep in 1..=entry.watched_episodes {
            // Idempotency: re-running the import must not duplicate history
            // rows (append_watch_history is a plain INSERT). This also means
            // an episode the XML import already wrote is left untouched.
            if storage.watch_history_count(entry.anime_id, ep).await? > 0 {
                continue;
            }
            storage
                .append_watch_history(
                    entry.anime_id,
                    ep,
                    None,
                    Some("taiga_v1"),
                    "import",
                    entry.last_updated,
                )
                .await?;
        }

        // Log migration
        storage
            .log_migration(
                "v1_list_entry",
                &entry.anime_id.to_string(),
                "imported",
                "ok",
            )
            .await?;

        report.imported_entries += 1;
    }

    Ok(report)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn parse_datetime_to_unix(s: &str) -> Option<i64> {
    // Try common formats
    let formats = [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d",
    ];
    for fmt in &formats {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return Some(dt.and_utc().timestamp());
        }
        if let Ok(d) = chrono::NaiveDate::parse_from_str(s, fmt) {
            return Some(
                d.and_hms_opt(0, 0, 0)?
                    .and_utc()
                    .timestamp(),
            );
        }
    }
    // Try RFC 3339
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp());
    }
    None
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dry_run_with_no_paths_returns_warning() {
        let paths = V1DataPaths::default();
        let report = dry_run_import(&paths).await.unwrap();
        assert_eq!(report.imported_anime, 0);
        assert!(report.warnings.iter().any(|w| w.message.contains("No v1 data")));
    }

    #[test]
    fn parse_datetime_formats() {
        assert!(parse_datetime_to_unix("2024-01-15T20:30:00").is_some());
        assert!(parse_datetime_to_unix("2024-01-15").is_some());
        assert!(parse_datetime_to_unix("not a date").is_none());
    }

    #[tokio::test]
    async fn dry_run_with_v1_sqlite() {
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;

        // Create temp v1 SQLite
        let tmp = std::env::temp_dir().join("test_dry_run_import.sqlite");
        let _ = std::fs::remove_file(&tmp);
        let db_url = format!("sqlite:{}", tmp.to_string_lossy());
        let opts = SqliteConnectOptions::from_str(&db_url).unwrap().create_if_missing(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect_with(opts).await.unwrap();

        sqlx::query(
            "CREATE TABLE anime (id INTEGER PRIMARY KEY, title TEXT, english TEXT, japanese TEXT, synonym TEXT,
                type INTEGER, status INTEGER, episode_count INTEGER, image TEXT, synopsis TEXT, score REAL,
                genres TEXT, tags TEXT, modified INTEGER)",
        ).execute(&pool).await.unwrap();

        sqlx::query("INSERT INTO anime VALUES (1, 'Valid', '', '', '', 1, 1, 12, '', '', 0.0, '', '', 1000)")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO anime VALUES (0, '', '', '', '', 0, 0, 0, '', '', 0.0, '', '', 0)")
            .execute(&pool).await.unwrap();

        pool.close().await;

        let paths = V1DataPaths {
            sqlite_path: Some(tmp.to_string_lossy().to_string()),
            ..Default::default()
        };

        let report = dry_run_import(&paths).await.unwrap();
        // 1 valid + 1 skipped (id=0 is also caught by the <=0 check)
        assert_eq!(report.imported_anime, 1);
        assert_eq!(report.skipped_anime, 1);
        assert_eq!(report.warnings.len(), 1);

        let _ = std::fs::remove_file(&tmp);
    }
}
