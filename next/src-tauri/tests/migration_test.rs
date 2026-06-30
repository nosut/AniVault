use taiga_next::engine::migration::{import_taiga_snapshot, TaigaAnime, TaigaSnapshot};
use taiga_next::engine::storage::Storage;

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
