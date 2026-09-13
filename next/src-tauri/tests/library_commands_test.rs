use anivault_core::engine::runtime::fresh_test_state;

#[tokio::test]
async fn search_library_returns_matches() {
    let state = fresh_test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Cowboy Bebop")
        .await
        .unwrap();
    state
        .storage
        .upsert_list_entry_full(1, "watching", 5, None, "", 0, 0)
        .await
        .unwrap();
    let result =
        anivault_core::commands::search_library_inner("bebop".into(), None, 10, 0, &state)
            .await
            .unwrap();
    assert_eq!(result.len(), 1);
}

#[tokio::test]
async fn get_library_stats_returns_counts() {
    let state = fresh_test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "A")
        .await
        .unwrap();
    state
        .storage
        .upsert_list_entry_full(1, "watching", 3, None, "", 0, 0)
        .await
        .unwrap();
    let stats = anivault_core::commands::get_library_stats_inner(&state)
        .await
        .unwrap();
    assert!(stats.watching >= 1);
}

#[tokio::test]
async fn fetch_anime_detail_returns_data() {
    let state = fresh_test_state().await;
    state
        .storage
        .upsert_anime(1, r#"{"romaji":"Test"}"#, 12, None, 0)
        .await
        .unwrap();
    state
        .storage
        .upsert_list_entry_full(1, "completed", 12, Some(8), "", 0, 0)
        .await
        .unwrap();
    let detail = anivault_core::commands::fetch_anime_detail_inner(1, &state)
        .await
        .unwrap();
    assert_eq!(detail.list_status, Some("completed".to_string()));
    assert_eq!(detail.score, Some(8));
}

#[tokio::test]
async fn update_list_entry_edits_progress() {
    let state = fresh_test_state().await;
    state
        .storage
        .insert_minimal_anime(1, "Test")
        .await
        .unwrap();
    state
        .storage
        .upsert_list_entry_full(1, "watching", 3, None, "", 0, 0)
        .await
        .unwrap();
    anivault_core::commands::update_list_entry_inner(1, None, Some(7), None, &state)
        .await
        .unwrap();
    let entry = state.storage.get_list_entry(1).await.unwrap().unwrap();
    assert_eq!(entry.watched_episodes, 7);
}

#[tokio::test]
async fn map_folder_to_anime_rejects_nonexistent_folder() {
    let state = fresh_test_state().await;
    state.storage.insert_minimal_anime(1, "Test Anime").await.unwrap();

    let result = anivault_core::commands::map_folder_to_anime_inner(
        "D:/this/path/does/not/exist",
        1,
        &state,
    )
    .await;

    assert!(result.is_err(), "mapping a nonexistent folder must fail");
}

#[tokio::test]
async fn map_folder_to_anime_rejects_filesystem_root() {
    let state = fresh_test_state().await;
    state.storage.insert_minimal_anime(1, "Test Anime").await.unwrap();

    // Whichever root exists on this machine — on Windows this is a drive
    // root, on Unix it's "/". Either way it has no parent.
    let root = std::path::Path::new(".")
        .canonicalize()
        .unwrap()
        .ancestors()
        .last()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let result = anivault_core::commands::map_folder_to_anime_inner(&root, 1, &state).await;

    assert!(result.is_err(), "mapping a filesystem root must be refused");
}

#[tokio::test]
async fn manual_progress_increase_is_recorded_in_watch_history() {
    let state = fresh_test_state().await;
    state.storage.insert_minimal_anime(1, "Test").await.unwrap();
    state
        .storage
        .upsert_list_entry_full(1, "watching", 3, None, "", 0, 0)
        .await
        .unwrap();

    anivault_core::commands::update_list_entry_inner(1, None, Some(4), None, &state)
        .await
        .unwrap();
    assert_eq!(state.storage.watch_history_count(1, 4).await.unwrap(), 1);

    // Unchanged or lowered progress, or a status-only edit, writes nothing.
    anivault_core::commands::update_list_entry_inner(1, None, Some(4), None, &state)
        .await
        .unwrap();
    anivault_core::commands::update_list_entry_inner(1, None, Some(2), None, &state)
        .await
        .unwrap();
    anivault_core::commands::update_list_entry_inner(1, Some("on_hold".into()), None, None, &state)
        .await
        .unwrap();
    assert_eq!(state.storage.watch_history_total_count().await.unwrap(), 1);
}

#[tokio::test]
async fn ready_to_watch_includes_an_imported_show_never_played_here() {
    let state = fresh_test_state().await;
    state.storage.insert_minimal_anime(1, "Imported").await.unwrap();
    state
        .storage
        .upsert_list_entry_full(1, "watching", 3, None, "", 1000, 1000)
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(
            "D:/Anime/Imported - 04.mkv",
            Some(1),
            4,
            100,
            anivault_core::engine::storage::MappingSource::Manual,
            1000,
        )
        .await
        .unwrap();

    let ready = anivault_core::commands::get_ready_to_watch_inner(&state).await.unwrap();
    assert_eq!(ready.len(), 1);
    assert_eq!((ready[0].anime_id, ready[0].next_episode), (1, 4));
}

#[tokio::test]
async fn episode_files_in_bulk_are_keyed_by_anime() {
    let state = fresh_test_state().await;
    for (id, title) in [(1, "One"), (2, "Two")] {
        state.storage.insert_minimal_anime(id, title).await.unwrap();
    }
    for (path, id, ep) in [("D:/a/One - 01.mkv", 1, 1), ("D:/a/One - 02.mkv", 1, 2), ("D:/a/Two - 01.mkv", 2, 1)] {
        state
            .storage
            .upsert_file_index(path, Some(id), ep, 100, anivault_core::engine::storage::MappingSource::Manual, 1)
            .await
            .unwrap();
    }

    let files = anivault_core::commands::get_episode_files_bulk_inner(&state, vec![1, 2, 3])
        .await
        .unwrap();
    assert_eq!(files[&1].len(), 2);
    assert_eq!(files[&2].len(), 1);
    assert!(!files.contains_key(&3));
}
