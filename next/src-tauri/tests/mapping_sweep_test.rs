use anivault_core::commands::{set_known_file_mapping_inner, FileMappingInput};
use anivault_core::engine::runtime::fresh_test_state;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("anivault_sweep_{tag}_{nanos}"));
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
async fn manual_mapping_sweeps_unmatched_siblings() {
    let state = fresh_test_state().await;
    let dir = unique_temp_dir("basic");
    let ep1 = dir.join("Zzqx Qwpv - 01.mkv");
    let ep2 = dir.join("Zzqx Qwpv - 02.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&ep2, b"x").unwrap();

    state.storage.insert_minimal_anime(7, "Completely Unrelated Title").await.unwrap();
    // Both start unmatched, as a failed scan would leave them.
    state.storage.upsert_file_index(&ep1.to_string_lossy(), None, 0, 0, now()).await.unwrap();
    state.storage.upsert_file_index(&ep2.to_string_lossy(), None, 0, 0, now()).await.unwrap();

    // Manually map episode 1 — episode 2 must self-map via inheritance.
    set_known_file_mapping_inner(&state, &ep1.to_string_lossy(), 7, 1)
        .await
        .unwrap();

    let ep1_row = state.storage.get_file_index(&ep1.to_string_lossy()).await.unwrap().unwrap();
    assert_eq!(ep1_row.anime_id, Some(7));
    assert_eq!(ep1_row.confidence, 100);

    let ep2_row = state.storage.get_file_index(&ep2.to_string_lossy()).await.unwrap().unwrap();
    assert_eq!(ep2_row.anime_id, Some(7), "sibling must be swept into the mapping");
    assert_eq!(ep2_row.confidence, 85);
    assert_eq!(ep2_row.episode, Some(2));

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn sweep_leaves_ignored_files_alone() {
    let state = fresh_test_state().await;
    let dir = unique_temp_dir("ignored");
    let ep1 = dir.join("Zzqx Qwpv - 01.mkv");
    let junk = dir.join("Zzqx Qwpv - 02.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&junk, b"x").unwrap();

    state.storage.insert_minimal_anime(7, "Completely Unrelated Title").await.unwrap();
    state.storage.upsert_file_index(&ep1.to_string_lossy(), None, 0, 0, now()).await.unwrap();
    state.storage.upsert_file_index(&junk.to_string_lossy(), None, 0, 0, now()).await.unwrap();
    state.storage.set_file_index_ignored(&junk.to_string_lossy(), true).await.unwrap();

    set_known_file_mapping_inner(&state, &ep1.to_string_lossy(), 7, 1)
        .await
        .unwrap();

    let junk_row = state.storage.get_file_index(&junk.to_string_lossy()).await.unwrap().unwrap();
    assert_eq!(junk_row.anime_id, None, "ignored files must never be swept");
    assert!(junk_row.ignored);

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn bulk_mapping_sweeps_siblings_too() {
    let state = fresh_test_state().await;
    let dir = unique_temp_dir("bulk");
    let ep1 = dir.join("Zzqx Qwpv - 01.mkv");
    let ep2 = dir.join("Zzqx Qwpv - 02.mkv");
    let ep3 = dir.join("Zzqx Qwpv - 03.mkv");
    for f in [&ep1, &ep2, &ep3] {
        fs::write(f, b"x").unwrap();
    }
    state.storage.insert_minimal_anime(7, "Completely Unrelated Title").await.unwrap();
    for f in [&ep1, &ep2, &ep3] {
        state.storage.upsert_file_index(&f.to_string_lossy(), None, 0, 0, now()).await.unwrap();
    }

    // Bulk-map episodes 1 and 2; episode 3 must be swept.
    let n = anivault_core::commands::set_known_file_mappings_inner(
        &state,
        vec![
            FileMappingInput { file_path: ep1.to_string_lossy().to_string(), anime_id: 7, episode: 1 },
            FileMappingInput { file_path: ep2.to_string_lossy().to_string(), anime_id: 7, episode: 2 },
        ],
    )
    .await
    .unwrap();
    assert_eq!(n, 2);

    let ep3_row = state.storage.get_file_index(&ep3.to_string_lossy()).await.unwrap().unwrap();
    assert_eq!(ep3_row.anime_id, Some(7));
    assert_eq!(ep3_row.confidence, 85);
    assert_eq!(ep3_row.episode, Some(3));

    fs::remove_dir_all(&dir).ok();
}
