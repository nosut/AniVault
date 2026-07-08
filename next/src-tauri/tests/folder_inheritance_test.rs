use anivault_core::engine::storage::Tests;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Create a unique empty temp directory for a test and return its path.
fn unique_temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("anivault_inherit_{tag}_{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[tokio::test]
async fn mapped_files_under_returns_only_real_matches() {
    let storage = Tests::new_in_memory().await;
    storage.insert_minimal_anime(7, "Some Show").await.unwrap();
    storage.insert_minimal_anime(8, "Other Show").await.unwrap();

    // Windows-style paths; the queries are pure string-prefix matches so no
    // real files are needed here.
    let base = "C:\\Lib\\ShowA\\";
    storage.upsert_file_index("C:\\Lib\\ShowA\\ep1.mkv", Some(7), 1, 100, now()).await.unwrap();
    storage.upsert_file_index("C:\\Lib\\ShowA\\ep2.mkv", None, 0, 0, now()).await.unwrap(); // unmatched
    storage.upsert_file_index("C:\\Lib\\ShowA2\\ep1.mkv", Some(8), 1, 100, now()).await.unwrap(); // sibling folder
    // Ignored row must never count.
    storage.upsert_file_index("C:\\Lib\\ShowA\\junk.mkv", Some(8), 1, 100, now()).await.unwrap();
    storage.set_file_index_ignored("C:\\Lib\\ShowA\\junk.mkv", true).await.unwrap();

    let mapped = storage.mapped_files_under(base).await.unwrap();
    assert_eq!(mapped, vec![("C:\\Lib\\ShowA\\ep1.mkv".to_string(), 7)]);

    let unmatched = storage.unmatched_files_under(base).await.unwrap();
    assert_eq!(unmatched, vec!["C:\\Lib\\ShowA\\ep2.mkv".to_string()]);
}
