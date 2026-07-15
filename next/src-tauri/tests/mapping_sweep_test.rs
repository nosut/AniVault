use anivault_core::commands::{set_known_file_mapping_inner, FileMappingInput};
use anivault_core::engine::runtime::fresh_test_state;
use anivault_core::engine::storage::MappingSource;
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

    state
        .storage
        .insert_minimal_anime(7, "Completely Unrelated Title")
        .await
        .unwrap();
    // Both start unmatched, as a failed scan would leave them.
    state
        .storage
        .upsert_file_index(
            &ep1.to_string_lossy(),
            None,
            0,
            0,
            MappingSource::Automatic,
            now(),
        )
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            &ep2.to_string_lossy(),
            None,
            0,
            0,
            MappingSource::Automatic,
            now(),
        )
        .await
        .unwrap();

    // Manually map episode 1 — episode 2 must self-map via inheritance.
    set_known_file_mapping_inner(&state, &ep1.to_string_lossy(), 7, 1)
        .await
        .unwrap();

    let ep1_row = state
        .storage
        .get_file_index(&ep1.to_string_lossy())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ep1_row.anime_id, Some(7));
    assert_eq!(ep1_row.confidence, 100);

    let ep2_row = state
        .storage
        .get_file_index(&ep2.to_string_lossy())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        ep2_row.anime_id,
        Some(7),
        "sibling must be swept into the mapping"
    );
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

    state
        .storage
        .insert_minimal_anime(7, "Completely Unrelated Title")
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            &ep1.to_string_lossy(),
            None,
            0,
            0,
            MappingSource::Automatic,
            now(),
        )
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            &junk.to_string_lossy(),
            None,
            0,
            0,
            MappingSource::Automatic,
            now(),
        )
        .await
        .unwrap();
    state
        .storage
        .set_file_index_ignored(&junk.to_string_lossy(), true)
        .await
        .unwrap();

    set_known_file_mapping_inner(&state, &ep1.to_string_lossy(), 7, 1)
        .await
        .unwrap();

    let junk_row = state
        .storage
        .get_file_index(&junk.to_string_lossy())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(junk_row.anime_id, None, "ignored files must never be swept");
    assert!(junk_row.ignored);

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn sweep_does_not_recurse_into_subdirectories() {
    let state = fresh_test_state().await;
    // A show folder with a "Specials" subdirectory — `unmatched_files_under`
    // is a prefix query, so without the direct-child guard the sweep would
    // also visit (and can auto-match by title score) files nested below the
    // mapped file's directory.
    let wrapper = unique_temp_dir("recurse");
    let dir = wrapper.join("Zzqx Qwpv");
    let sub_dir = dir.join("Specials");
    fs::create_dir_all(&sub_dir).unwrap();

    let ep1 = dir.join("Zzqx Qwpv - 01.mkv");
    // Bare filename: no title of its own, so it can only be matched via its
    // grandparent directory name ("Zzqx Qwpv") — which exactly matches a
    // *different* anime (id 8) than the one manually mapped for ep1 (id 7).
    let sub_ep = sub_dir.join("01.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&sub_ep, b"x").unwrap();

    state
        .storage
        .insert_minimal_anime(7, "Completely Unrelated Title")
        .await
        .unwrap();
    state
        .storage
        .insert_minimal_anime(8, "Zzqx Qwpv")
        .await
        .unwrap();
    // Both start unmatched, as a failed scan would leave them.
    state
        .storage
        .upsert_file_index(
            &ep1.to_string_lossy(),
            None,
            0,
            0,
            MappingSource::Automatic,
            now(),
        )
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            &sub_ep.to_string_lossy(),
            None,
            0,
            0,
            MappingSource::Automatic,
            now(),
        )
        .await
        .unwrap();

    // Manually map the top-level episode to anime 7 — the file in the
    // SUBDIRECTORY must NOT be swept (it isn't a direct sibling), even though
    // it's still "unmatched under" the mapped file's directory by prefix, and
    // even though it would title-match anime 8 if visited.
    set_known_file_mapping_inner(&state, &ep1.to_string_lossy(), 7, 1)
        .await
        .unwrap();

    let ep1_row = state
        .storage
        .get_file_index(&ep1.to_string_lossy())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ep1_row.anime_id, Some(7));

    let sub_row = state
        .storage
        .get_file_index(&sub_ep.to_string_lossy())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        sub_row.anime_id, None,
        "a file in a subdirectory must not be swept by a mapping in the parent directory"
    );

    fs::remove_dir_all(&wrapper).ok();
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
    state
        .storage
        .insert_minimal_anime(7, "Completely Unrelated Title")
        .await
        .unwrap();
    for f in [&ep1, &ep2, &ep3] {
        state
            .storage
            .upsert_file_index(
                &f.to_string_lossy(),
                None,
                0,
                0,
                MappingSource::Automatic,
                now(),
            )
            .await
            .unwrap();
    }

    // Bulk-map episodes 1 and 2; episode 3 must be swept.
    let n = anivault_core::commands::set_known_file_mappings_inner(
        &state,
        vec![
            FileMappingInput {
                file_path: ep1.to_string_lossy().to_string(),
                anime_id: 7,
                episode: 1,
            },
            FileMappingInput {
                file_path: ep2.to_string_lossy().to_string(),
                anime_id: 7,
                episode: 2,
            },
        ],
    )
    .await
    .unwrap();
    assert_eq!(n, 2);

    let ep3_row = state
        .storage
        .get_file_index(&ep3.to_string_lossy())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ep3_row.anime_id, Some(7));
    assert_eq!(ep3_row.confidence, 85);
    assert_eq!(ep3_row.episode, Some(3));

    fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn scanner_prefers_unanimous_folder_mapping_over_base_title_match() {
    let state = fresh_test_state().await;
    let wrapper = unique_temp_dir("season_override");
    let dir = wrapper
        .join("Skeleton Knight in Another World")
        .join("Season 2");
    fs::create_dir_all(&dir).unwrap();
    let ep1 = dir.join(
        "Skeleton Knight in Another World - S02E01 - Duel to the Death! Silver Knight VS Silver Knight!.mkv",
    );
    let ep2 = dir.join(
        "Skeleton Knight in Another World - S02E02 - The Toxic Fangs of Assassination Target the Elvish Blade.mkv",
    );
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
            None,
            0,
            0,
            MappingSource::Automatic,
            now(),
        )
        .await
        .unwrap();

    let matched = anivault_core::engine::library_scanner::match_file(&state.storage, &ep2)
        .await
        .unwrap();
    assert_eq!(matched.anime_id, Some(185542));
    assert_eq!(matched.confidence, 85);
    assert_eq!(matched.episode, Some(2));
    assert_eq!(matched.mapping_source, MappingSource::Inherited);

    fs::remove_dir_all(&wrapper).ok();
}

#[tokio::test]
async fn rematch_unmapped_does_not_rewrite_an_existing_automatic_mapping() {
    let state = fresh_test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Wrong Existing Anime")
        .await
        .unwrap();
    state
        .storage
        .insert_minimal_anime(2, "Target Show")
        .await
        .unwrap();
    let path = "D:/Anime/Target Show - 01.mkv";
    state
        .storage
        .upsert_file_index(path, Some(1), 1, 50, MappingSource::Automatic, now())
        .await
        .unwrap();

    let changed = anivault_core::commands::rematch_unmapped_files_inner(&state)
        .await
        .unwrap();

    assert_eq!(changed, 0);
    assert_eq!(
        state
            .storage
            .get_file_index(path)
            .await
            .unwrap()
            .unwrap()
            .anime_id,
        Some(1)
    );
}
