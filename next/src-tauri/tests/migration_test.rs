use anivault_core::engine::migration::{live_import, DuplicateStrategy, V1DataPaths};
use anivault_core::engine::storage::Storage;

fn create_v1_sqlite(rows: &[(i64, &str, i64)]) -> String {
    use sqlx::sqlite::SqliteConnectOptions;
    use std::str::FromStr;

    let tmp = std::env::temp_dir().join(format!("test_migration_{}.sqlite", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
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

        sqlx::query(
            "CREATE TABLE anime_list (
                list_id INTEGER PRIMARY KEY, anime_id INTEGER, watched_episodes INTEGER,
                my_score INTEGER, my_status INTEGER, my_start_date TEXT, my_finish_date TEXT,
                modified INTEGER, tags TEXT
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
                sqlx::query("INSERT INTO anime_list (anime_id, watched_episodes, my_status, modified) VALUES (?, ?, 1, 1000)")
                    .bind(id)
                    .bind(watched)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        }

        pool.close().await;
    });
    tmp.to_string_lossy().to_string()
}

#[tokio::test]
async fn import_snapshot_reports_imported_and_skipped_records() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let db_path = create_v1_sqlite(&[(10, "Frieren", 12), (0, "Broken", 1)]);

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

    let db_path = create_v1_sqlite(&[(10, "Frieren", 12)]);

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
