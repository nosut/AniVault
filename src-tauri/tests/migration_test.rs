use anivault_core::engine::migration::{import_taiga_snapshot, TaigaAnime, TaigaSnapshot};
use anivault_core::engine::storage::Storage;

#[tokio::test]
async fn import_snapshot_reports_imported_and_skipped_records() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let snapshot = TaigaSnapshot {
        anime: vec![
            TaigaAnime { id: 10, title: "Frieren".to_string(), watched_episodes: 12 },
            TaigaAnime { id: 0, title: "Broken".to_string(), watched_episodes: 1 },
        ],
    };

    let report = import_taiga_snapshot(&storage, snapshot).await.unwrap();
    assert_eq!(report.imported_anime, 1);
    assert_eq!(report.skipped_records, 1);
    assert_eq!(report.warnings[0].source_id, "0");
}

#[tokio::test]
async fn import_snapshot_does_not_duplicate_watch_history_when_rerun() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let snapshot = TaigaSnapshot {
        anime: vec![TaigaAnime { id: 10, title: "Frieren".to_string(), watched_episodes: 12 }],
    };

    import_taiga_snapshot(&storage, snapshot.clone()).await.unwrap();
    import_taiga_snapshot(&storage, snapshot).await.unwrap();

    let history_count = storage.watch_history_count(10, 12).await.unwrap();
    assert_eq!(history_count, 1);
}
