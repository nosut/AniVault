use anivault_core::engine::migration::{live_import, DuplicateStrategy, V1DataPaths};
use anivault_core::engine::storage::Storage;

// Async (no nested runtime — building one inside #[tokio::test] panics) and
// tagged per test so parallel tests don't race on a shared temp file.
async fn create_v1_sqlite(tag: &str, rows: &[(i64, &str, i64)]) -> String {
    use sqlx::sqlite::SqliteConnectOptions;
    use std::str::FromStr;

    let tmp = std::env::temp_dir().join(format!(
        "test_migration_{}_{}.sqlite",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_file(&tmp);
    {
        let db_url = format!("sqlite:{}", tmp.to_string_lossy());
        let opts = SqliteConnectOptions::from_str(&db_url)
            .unwrap()
            .create_if_missing(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE anime (
                id INTEGER PRIMARY KEY, title TEXT, english TEXT, japanese TEXT, synonym TEXT,
                type INTEGER, status INTEGER, episode_count INTEGER, image TEXT, synopsis TEXT, score REAL,
                genres TEXT, tags TEXT, modified INTEGER
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Match the real v1 schema that v1_read.rs expects (media_id/progress/
        // last_updated…); with wrong column names every row parses as anime_id 0
        // and is skipped.
        sqlx::query(
            "CREATE TABLE anime_list (
                media_id INTEGER PRIMARY KEY,
                progress INTEGER, score INTEGER, status INTEGER,
                date_start TEXT, date_end TEXT, notes TEXT,
                last_updated INTEGER, rewatched_times INTEGER,
                rewatching INTEGER, rewatching_ep INTEGER
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        for (id, title, watched) in rows {
            sqlx::query("INSERT INTO anime VALUES (?, ?, '', '', '', 1, 1, 12, '', '', 0.0, '', '', 1000)")
                .bind(id)
                .bind(title)
                .execute(&pool)
                .await
                .unwrap();

            if *watched > 0 {
                sqlx::query(
                    "INSERT INTO anime_list (media_id, progress, score, status, date_start, date_end, notes, last_updated, rewatched_times, rewatching, rewatching_ep)
                     VALUES (?, ?, 0, 1, '', '', '', 1000, 0, 0, 0)",
                )
                .bind(id)
                .bind(watched)
                .execute(&pool)
                .await
                .unwrap();
            }
        }

        pool.close().await;
    }
    tmp.to_string_lossy().to_string()
}

#[tokio::test]
async fn import_snapshot_reports_imported_and_skipped_records() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let db_path = create_v1_sqlite("report", &[(10, "Frieren", 12), (0, "Broken", 1)]).await;

    let paths = V1DataPaths {
        sqlite_path: Some(db_path.clone()),
        ..Default::default()
    };

    let report = live_import(&storage, &paths, DuplicateStrategy::Skip)
        .await
        .unwrap();
    assert_eq!(report.imported_anime, 1);
    assert!(report.warnings.iter().any(|w| w.source_id == "0"));

    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn import_snapshot_does_not_duplicate_watch_history_when_rerun() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let db_path = create_v1_sqlite("rerun", &[(10, "Frieren", 12)]).await;

    let paths = V1DataPaths {
        sqlite_path: Some(db_path.clone()),
        ..Default::default()
    };

    live_import(&storage, &paths, DuplicateStrategy::Skip)
        .await
        .unwrap();
    live_import(&storage, &paths, DuplicateStrategy::Skip)
        .await
        .unwrap();

    let history_count = storage.watch_history_count(10, 12).await.unwrap();
    assert_eq!(history_count, 1);

    let _ = std::fs::remove_file(&db_path);
}
