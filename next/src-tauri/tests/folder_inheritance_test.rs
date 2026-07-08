use anivault_core::engine::storage::Tests;
use anivault_core::engine::library_scanner::{match_file, unanimous_dir_anime};
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

#[test]
fn unanimous_dir_anime_direct_children_only() {
    let prefix = "C:\\Lib\\ShowA\\";
    // Direct children agreeing → Some
    let rows = vec![
        ("C:\\Lib\\ShowA\\ep1.mkv".to_string(), 7),
        ("C:\\Lib\\ShowA\\ep2.mkv".to_string(), 7),
    ];
    assert_eq!(unanimous_dir_anime(&rows, prefix), Some(7));

    // Disagreement → None
    let mixed = vec![
        ("C:\\Lib\\ShowA\\ep1.mkv".to_string(), 7),
        ("C:\\Lib\\ShowA\\other.mkv".to_string(), 8),
    ];
    assert_eq!(unanimous_dir_anime(&mixed, prefix), None);

    // A row in a subdirectory is not a direct sibling — ignored.
    let sub = vec![("C:\\Lib\\ShowA\\Season 1\\ep1.mkv".to_string(), 7)];
    assert_eq!(unanimous_dir_anime(&sub, prefix), None);

    // No rows → None
    assert_eq!(unanimous_dir_anime(&[], prefix), None);
}

#[tokio::test]
async fn file_inherits_unanimous_folder_anime() {
    let storage = Tests::new_in_memory().await;
    let dir = unique_temp_dir("inherit");
    let ep1 = dir.join("Zzqx Qwpv - 01.mkv");
    let ep2 = dir.join("Zzqx Qwpv - 02.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&ep2, b"x").unwrap();

    // The anime title shares no words with the filename, so title matching
    // fails — exactly the situation that forced manual mapping.
    storage.insert_minimal_anime(7, "Completely Unrelated Title").await.unwrap();
    // Episode 1 was mapped manually (confidence 100).
    storage
        .upsert_file_index(&ep1.to_string_lossy(), Some(7), 1, 100, now())
        .await
        .unwrap();

    let (anime_id, confidence, episode) = match_file(&storage, &ep2.as_path()).await.unwrap();
    assert_eq!(anime_id, Some(7), "episode 2 must inherit the folder's anime");
    assert_eq!(confidence, 85);
    assert_eq!(episode, Some(2));

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn mixed_folder_does_not_inherit() {
    let storage = Tests::new_in_memory().await;
    let dir = unique_temp_dir("mixed");
    let a = dir.join("Zzqx Qwpv - 01.mkv");
    let b = dir.join("Vvbnm Rrtyu - 01.mkv");
    let c = dir.join("Zzqx Qwpv - 02.mkv");
    for f in [&a, &b, &c] {
        fs::write(f, b"x").unwrap();
    }
    storage.insert_minimal_anime(7, "Completely Unrelated Title").await.unwrap();
    storage.insert_minimal_anime(8, "Another Unrelated Title").await.unwrap();
    storage.upsert_file_index(&a.to_string_lossy(), Some(7), 1, 100, now()).await.unwrap();
    storage.upsert_file_index(&b.to_string_lossy(), Some(8), 1, 100, now()).await.unwrap();

    let (anime_id, confidence, _) = match_file(&storage, &c.as_path()).await.unwrap();
    assert_eq!(anime_id, None, "disagreeing siblings must not inherit");
    assert_eq!(confidence, 0);

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn no_episode_number_does_not_inherit() {
    let storage = Tests::new_in_memory().await;
    let dir = unique_temp_dir("noep");
    let ep1 = dir.join("Zzqx Qwpv - 01.mkv");
    let extra = dir.join("Zzqx Qwpv.mkv"); // no parsable episode number
    fs::write(&ep1, b"x").unwrap();
    fs::write(&extra, b"x").unwrap();
    storage.insert_minimal_anime(7, "Completely Unrelated Title").await.unwrap();
    storage.upsert_file_index(&ep1.to_string_lossy(), Some(7), 1, 100, now()).await.unwrap();

    let (anime_id, confidence, _) = match_file(&storage, &extra.as_path()).await.unwrap();
    assert_eq!(anime_id, None, "no episode number → leave unmatched");
    assert_eq!(confidence, 0);

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn confident_title_match_beats_inheritance() {
    let storage = Tests::new_in_memory().await;
    let dir = unique_temp_dir("titlewins");
    let sibling = dir.join("Zzqx Qwpv - 01.mkv");
    let movie = dir.join("Great Vault Movie - 01.mkv");
    fs::write(&sibling, b"x").unwrap();
    fs::write(&movie, b"x").unwrap();
    storage.insert_minimal_anime(7, "Completely Unrelated Title").await.unwrap();
    storage.insert_minimal_anime(9, "Great Vault Movie").await.unwrap();
    storage.upsert_file_index(&sibling.to_string_lossy(), Some(7), 1, 100, now()).await.unwrap();

    let (anime_id, confidence, _) = match_file(&storage, &movie.as_path()).await.unwrap();
    assert_eq!(anime_id, Some(9), "a confident title match must win over inheritance");
    assert!(confidence >= 40);

    fs::remove_dir_all(&dir).ok();
}
