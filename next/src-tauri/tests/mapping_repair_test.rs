use anivault_core::commands::repair_anime_file_mappings_inner;
use anivault_core::engine::library_scanner::rescan_anime_dirs;
use anivault_core::engine::runtime::{fresh_test_state, EngineState};
use anivault_core::engine::storage::MappingSource;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("anivault_repair_{tag}_{nanos}"));
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
async fn targeted_rescan_reports_confident_wrong_series_without_changing_it() {
    let state = fresh_test_state().await;
    let root = unique_temp_dir("confident_wrong_series");
    let season = root
        .join("Skeleton Knight in Another World")
        .join("Season 2");
    fs::create_dir_all(&season).unwrap();
    let ep1 = season.join("Skeleton Knight in Another World - S02E01.mkv");
    let ep2 = season.join("Skeleton Knight in Another World - S02E02.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&ep2, b"x").unwrap();

    state
        .storage
        .insert_minimal_anime(132474, "Skeleton Knight in Another World")
        .await
        .unwrap();
    state
        .storage
        .insert_minimal_anime(185542, "Skeleton Knight in Another World Season 2")
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            &ep1.to_string_lossy(),
            Some(185542),
            1,
            100,
            MappingSource::Manual,
            now(),
        )
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            &ep2.to_string_lossy(),
            Some(132474),
            2,
            100,
            MappingSource::Legacy,
            now(),
        )
        .await
        .unwrap();

    let report = rescan_anime_dirs(&state.storage, 185542).await.unwrap();

    assert_eq!(report.mapping_conflicts.len(), 1);
    let conflict = &report.mapping_conflicts[0];
    assert_eq!(conflict.file_path, ep2.to_string_lossy());
    assert_eq!(conflict.current_anime_id, 132474);
    assert_eq!(conflict.current_anime_title, "Skeleton Knight in Another World");
    assert_eq!(conflict.mapping_source, MappingSource::Legacy);
    assert!(conflict.repairable);
    assert_eq!(
        state
            .storage
            .get_file_index(&ep2.to_string_lossy())
            .await
            .unwrap()
            .unwrap()
            .anime_id,
        Some(132474)
    );

    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn targeted_rescan_reports_manual_conflict_as_protected() {
    let state = fresh_test_state().await;
    let root = unique_temp_dir("manual_conflict");
    let season = root
        .join("Skeleton Knight in Another World")
        .join("Season 2");
    fs::create_dir_all(&season).unwrap();
    let ep1 = season.join("Skeleton Knight in Another World - S02E01.mkv");
    let ep2 = season.join("Skeleton Knight in Another World - S02E02.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&ep2, b"x").unwrap();
    state
        .storage
        .insert_minimal_anime(132474, "Skeleton Knight in Another World")
        .await
        .unwrap();
    state
        .storage
        .insert_minimal_anime(185542, "Skeleton Knight in Another World Season 2")
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            &ep1.to_string_lossy(),
            Some(185542),
            1,
            100,
            MappingSource::Manual,
            now(),
        )
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            &ep2.to_string_lossy(),
            Some(132474),
            2,
            100,
            MappingSource::Manual,
            now(),
        )
        .await
        .unwrap();

    let report = rescan_anime_dirs(&state.storage, 185542).await.unwrap();
    assert_eq!(report.mapping_conflicts.len(), 1);
    assert!(!report.mapping_conflicts[0].repairable);
    assert_eq!(
        state
            .storage
            .get_file_index(&ep2.to_string_lossy())
            .await
            .unwrap()
            .unwrap()
            .anime_id,
        Some(132474)
    );
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn targeted_rescan_ignores_nested_and_weak_filename_conflicts() {
    let state = fresh_test_state().await;
    let root = unique_temp_dir("unrelated_conflicts");
    let season = root
        .join("Skeleton Knight in Another World")
        .join("Season 2");
    let specials = season.join("Specials");
    fs::create_dir_all(&specials).unwrap();
    let anchor = season.join("Skeleton Knight in Another World - S02E01.mkv");
    let unrelated = season.join("Unrelated Movie - 01.mkv");
    let nested = specials.join("Skeleton Knight in Another World - S02E99.mkv");
    for path in [&anchor, &unrelated, &nested] {
        fs::write(path, b"x").unwrap();
    }
    state
        .storage
        .insert_minimal_anime(7, "Unrelated Movie")
        .await
        .unwrap();
    state
        .storage
        .insert_minimal_anime(185542, "Skeleton Knight in Another World Season 2")
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            &anchor.to_string_lossy(),
            Some(185542),
            1,
            100,
            MappingSource::Manual,
            now(),
        )
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            &unrelated.to_string_lossy(),
            Some(7),
            1,
            100,
            MappingSource::Automatic,
            now(),
        )
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            &nested.to_string_lossy(),
            Some(7),
            99,
            100,
            MappingSource::Automatic,
            now(),
        )
        .await
        .unwrap();

    let report = rescan_anime_dirs(&state.storage, 185542).await.unwrap();
    assert!(report.mapping_conflicts.is_empty());
    fs::remove_dir_all(root).ok();
}

async fn skeleton_knight_mixed_fixture(
    wrong_source: MappingSource,
) -> (EngineState, PathBuf, PathBuf, PathBuf) {
    let state = fresh_test_state().await;
    let root = unique_temp_dir("repair_fixture");
    let season = root
        .join("Skeleton Knight in Another World")
        .join("Season 2");
    fs::create_dir_all(&season).unwrap();
    let ep1 = season.join("Skeleton Knight in Another World - S02E01.mkv");
    let ep2 = season.join("Skeleton Knight in Another World - S02E02.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&ep2, b"x").unwrap();
    state
        .storage
        .insert_minimal_anime(132474, "Skeleton Knight in Another World")
        .await
        .unwrap();
    state
        .storage
        .insert_minimal_anime(185542, "Skeleton Knight in Another World Season 2")
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            &ep1.to_string_lossy(),
            Some(185542),
            1,
            100,
            MappingSource::Manual,
            now(),
        )
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            &ep2.to_string_lossy(),
            Some(132474),
            2,
            100,
            wrong_source,
            now(),
        )
        .await
        .unwrap();
    (state, root, ep1, ep2)
}

#[tokio::test]
async fn confirmed_repair_moves_legacy_episode_and_detail_query_returns_both() {
    let (state, root, _ep1, ep2) = skeleton_knight_mixed_fixture(MappingSource::Legacy).await;

    let before = rescan_anime_dirs(&state.storage, 185542).await.unwrap();
    assert_eq!(before.mapping_conflicts.len(), 1);
    assert_eq!(
        state.storage.file_index_by_anime(185542).await.unwrap().len(),
        1
    );

    let repaired = repair_anime_file_mappings_inner(&state, 185542)
        .await
        .unwrap();
    assert_eq!(repaired.repaired, 1);
    assert_eq!(repaired.protected, 0);

    let files = state.storage.file_index_by_anime(185542).await.unwrap();
    assert_eq!(
        files.iter().map(|row| row.episode).collect::<Vec<_>>(),
        vec![Some(1), Some(2)]
    );
    let ep2_row = state
        .storage
        .get_file_index(&ep2.to_string_lossy())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ep2_row.mapping_source, MappingSource::Manual);

    let second = repair_anime_file_mappings_inner(&state, 185542)
        .await
        .unwrap();
    assert_eq!(second.repaired, 0);
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn repair_revalidates_and_preserves_manual_mapping_changed_after_rescan() {
    let (state, root, _ep1, ep2) = skeleton_knight_mixed_fixture(MappingSource::Legacy).await;
    assert_eq!(
        rescan_anime_dirs(&state.storage, 185542)
            .await
            .unwrap()
            .mapping_conflicts
            .len(),
        1
    );

    state
        .storage
        .upsert_file_index(
            &ep2.to_string_lossy(),
            Some(132474),
            2,
            100,
            MappingSource::Manual,
            now(),
        )
        .await
        .unwrap();
    let result = repair_anime_file_mappings_inner(&state, 185542)
        .await
        .unwrap();

    assert_eq!(result.repaired, 0);
    assert_eq!(result.protected, 1);
    assert_eq!(
        state
            .storage
            .get_file_index(&ep2.to_string_lossy())
            .await
            .unwrap()
            .unwrap()
            .anime_id,
        Some(132474)
    );
    fs::remove_dir_all(root).ok();
}
