use anivault_core::engine::storage::{MappingSource, Storage};

#[tokio::test]
async fn mapping_source_migration_backfills_legacy_rows() {
    use sqlx::sqlite::SqlitePoolOptions;

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE file_index (
            file_path TEXT PRIMARY KEY,
            anime_id INTEGER,
            episode INTEGER,
            confidence INTEGER NOT NULL DEFAULT 0,
            indexed_at INTEGER NOT NULL,
            ignored INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO file_index
         (file_path, anime_id, episode, confidence, indexed_at, ignored)
         VALUES ('D:/Anime/Show - 01.mkv', 7, 1, 100, 1, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(include_str!(
        "../migrations/0007_file_index_mapping_source.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();

    let source: String = sqlx::query_scalar(
        "SELECT mapping_source FROM file_index WHERE file_path = 'D:/Anime/Show - 01.mkv'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(source, "legacy");
}

#[test]
fn mapping_source_unknown_values_fail_closed_as_legacy() {
    assert_eq!(MappingSource::from_db("manual"), MappingSource::Manual);
    assert_eq!(MappingSource::from_db("unexpected"), MappingSource::Legacy);
    assert!(!MappingSource::Manual.is_repairable());
    assert!(MappingSource::Automatic.is_repairable());
    assert!(MappingSource::Inherited.is_repairable());
    assert!(MappingSource::Legacy.is_repairable());
}

#[tokio::test]
async fn storage_migrates_and_uses_journal_mode_supported_by_memory_sqlite() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let journal_mode = storage.journal_mode().await.unwrap();
    assert_eq!(journal_mode.to_lowercase(), "memory");
}

#[tokio::test]
async fn storage_appends_history_and_queues_sync() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage
        .insert_minimal_anime(1, "Cowboy Bebop")
        .await
        .unwrap();

    let history_id = storage
        .append_watch_history(
            1,
            7,
            Some("D:/Anime/Cowboy Bebop 07.mkv"),
            Some("mpv"),
            "manual",
            1_782_769_008,
        )
        .await
        .unwrap();
    assert!(history_id > 0);

    let sync_id = storage
        .queue_sync(
            1,
            "anilist",
            "update_progress",
            r#"{"episode":7}"#,
            1_782_769_008,
        )
        .await
        .unwrap();
    assert!(sync_id > 0);

    let pending = storage.pending_sync_count("anilist").await.unwrap();
    assert_eq!(pending, 1);
}

#[tokio::test]
async fn get_file_index_by_filename_returns_none_when_ambiguous() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage
        .insert_minimal_anime(1, "Show A Season 1")
        .await
        .unwrap();
    storage
        .insert_minimal_anime(2, "Show A Season 2")
        .await
        .unwrap();

    // Two different shows, each with an episode file that happens to share
    // the exact same basename — a real-world case with generic numbering.
    storage
        .upsert_file_index(
            "D:/Anime/Show A S1/01.mkv",
            Some(1),
            1,
            100,
            MappingSource::Manual,
            1_782_769_000,
        )
        .await
        .unwrap();
    storage
        .upsert_file_index(
            "D:/Anime/Show A S2/01.mkv",
            Some(2),
            1,
            100,
            MappingSource::Manual,
            1_782_769_001,
        )
        .await
        .unwrap();

    let result = storage.get_file_index_by_filename("01.mkv").await.unwrap();

    assert!(
        result.is_none(),
        "an ambiguous basename match (two different anime_ids) must not silently pick a winner, got {:?}",
        result
    );
}

#[tokio::test]
async fn get_file_index_by_filename_still_resolves_unambiguous_match() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage
        .insert_minimal_anime(1, "Cowboy Bebop")
        .await
        .unwrap();
    storage
        .upsert_file_index(
            "D:/Anime/Cowboy Bebop - 01.mkv",
            Some(1),
            1,
            100,
            MappingSource::Manual,
            1_782_769_000,
        )
        .await
        .unwrap();

    let result = storage
        .get_file_index_by_filename("Cowboy Bebop - 01.mkv")
        .await
        .unwrap();

    assert_eq!(result.map(|r| r.anime_id), Some(Some(1)));
}

#[tokio::test]
async fn file_index_anime_id_index_exists_after_migration() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let exists = storage.has_index("idx_file_index_anime_id").await.unwrap();

    assert!(
        exists,
        "expected idx_file_index_anime_id to exist after migrate()"
    );
}
