use anivault_core::engine::library_scanner::{
    rescan_anime_dirs, scan_library_folders, set_library_folders,
};
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
    let dir = std::env::temp_dir().join(format!("anivault_scan_{tag}_{nanos}"));
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
async fn scan_prunes_files_deleted_from_disk() {
    let storage = Tests::new_in_memory().await;
    let dir = unique_temp_dir("prune");
    let keep = dir.join("Show - 01.mkv");
    let gone = dir.join("Show - 02.mkv");
    fs::write(&keep, b"x").unwrap();
    fs::write(&gone, b"x").unwrap();

    set_library_folders(&storage, vec![dir.to_string_lossy().to_string()])
        .await
        .unwrap();

    // First scan indexes both files.
    let r1 = scan_library_folders(&storage).await.unwrap();
    assert_eq!(r1.found, 2);
    assert_eq!(r1.removed, 0);

    // Delete one file, then rescan — its row must be pruned, the other kept.
    fs::remove_file(&gone).unwrap();
    let r2 = scan_library_folders(&storage).await.unwrap();
    assert_eq!(r2.removed, 1, "the deleted file's row should be pruned");
    assert!(
        storage
            .get_file_index(&gone.to_string_lossy())
            .await
            .unwrap()
            .is_none(),
        "deleted file's index row should be gone"
    );
    assert!(
        storage
            .get_file_index(&keep.to_string_lossy())
            .await
            .unwrap()
            .is_some(),
        "surviving file's index row should remain"
    );

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn offline_folder_does_not_prune_its_files() {
    let storage = Tests::new_in_memory().await;

    // A folder that does not exist on disk (simulates an offline network drive).
    let offline = unique_temp_dir("offline");
    let ghost_file = offline.join("Show - 01.mkv").to_string_lossy().to_string();
    // Index a row for a file under it, then remove the folder so it's "offline".
    storage.insert_minimal_anime(1, "Show").await.unwrap();
    storage
        .upsert_file_index(&ghost_file, Some(1), 1, 90, now())
        .await
        .unwrap();
    fs::remove_dir_all(&offline).unwrap();

    set_library_folders(&storage, vec![offline.to_string_lossy().to_string()])
        .await
        .unwrap();

    let report = scan_library_folders(&storage).await.unwrap();
    assert_eq!(report.removed, 0, "offline folder must never prune");
    assert!(
        storage.get_file_index(&ghost_file).await.unwrap().is_some(),
        "row under an offline folder must survive"
    );
}

#[tokio::test]
async fn rescan_anime_dirs_prunes_only_that_shows_folder() {
    let storage = Tests::new_in_memory().await;
    let dir = unique_temp_dir("rescan");
    let file = dir.join("Show - 01.mkv");
    fs::write(&file, b"x").unwrap();

    // Map the file to anime 1 directly.
    storage.insert_minimal_anime(1, "Show").await.unwrap();
    storage
        .upsert_file_index(&file.to_string_lossy(), Some(1), 1, 90, now())
        .await
        .unwrap();

    // Delete the file; a targeted rescan of anime 1 should prune it.
    fs::remove_file(&file).unwrap();
    let report = rescan_anime_dirs(&storage, 1).await.unwrap();
    assert_eq!(report.removed, 1);
    assert!(
        storage
            .file_index_by_anime(1)
            .await
            .unwrap()
            .is_empty(),
        "anime's deleted file should be pruned by targeted rescan"
    );

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn rescan_prunes_when_whole_show_folder_is_deleted() {
    // Regression: deleting the entire season folder (not just files inside it)
    // must still clear the show's episodes on a targeted rescan. The offline
    // guard keys off the library root, which stays online.
    let storage = Tests::new_in_memory().await;
    let lib_root = unique_temp_dir("wholedel_root");
    let season = lib_root.join("Show").join("Season 1");
    fs::create_dir_all(&season).unwrap();
    let ep1 = season.join("Show - S01E01.mkv");
    let ep2 = season.join("Show - S01E02.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&ep2, b"x").unwrap();

    // Configure the library root (stays present) and map both files to anime 1.
    set_library_folders(&storage, vec![lib_root.to_string_lossy().to_string()])
        .await
        .unwrap();
    storage.insert_minimal_anime(1, "Show").await.unwrap();
    for (i, f) in [&ep1, &ep2].iter().enumerate() {
        storage
            .upsert_file_index(&f.to_string_lossy(), Some(1), i as i32 + 1, 90, now())
            .await
            .unwrap();
    }

    // Delete the ENTIRE season (and show) folder, leaving the library root intact.
    fs::remove_dir_all(lib_root.join("Show")).unwrap();

    let report = rescan_anime_dirs(&storage, 1).await.unwrap();
    assert_eq!(report.removed, 2, "both episodes should be pruned");
    assert!(
        storage.file_index_by_anime(1).await.unwrap().is_empty(),
        "deleting the whole folder must clear the show's files"
    );

    fs::remove_dir_all(&lib_root).ok();
}

#[tokio::test]
async fn rescan_does_not_prune_when_library_root_is_offline() {
    // If the whole library root is gone (e.g. an unmounted network drive), a
    // targeted rescan must NOT wipe the show's files.
    let storage = Tests::new_in_memory().await;
    let lib_root = unique_temp_dir("offline_root");
    let file = lib_root.join("Show").join("Show - S01E01.mkv");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, b"x").unwrap();

    set_library_folders(&storage, vec![lib_root.to_string_lossy().to_string()])
        .await
        .unwrap();
    storage.insert_minimal_anime(1, "Show").await.unwrap();
    storage
        .upsert_file_index(&file.to_string_lossy(), Some(1), 1, 90, now())
        .await
        .unwrap();

    // Take the whole library root offline.
    fs::remove_dir_all(&lib_root).unwrap();

    let report = rescan_anime_dirs(&storage, 1).await.unwrap();
    assert_eq!(report.removed, 0, "offline library root must not prune");
    assert_eq!(
        storage.file_index_by_anime(1).await.unwrap().len(),
        1,
        "files must survive when their library root is offline"
    );
}

#[tokio::test]
async fn rescan_anime_with_no_files_falls_back_to_full_scan() {
    let storage = Tests::new_in_memory().await;
    let dir = unique_temp_dir("fallback");
    fs::write(dir.join("Show - 01.mkv"), b"x").unwrap();

    set_library_folders(&storage, vec![dir.to_string_lossy().to_string()])
        .await
        .unwrap();

    // Anime 99 has no indexed files, so there's no folder to derive — the rescan
    // falls back to a full library scan and indexes the file it finds.
    let report = rescan_anime_dirs(&storage, 99).await.unwrap();
    assert_eq!(report.found, 1, "fallback full scan should walk library folders");

    fs::remove_dir_all(&dir).ok();
}
