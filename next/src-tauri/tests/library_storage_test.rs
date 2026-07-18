use anivault_core::engine::storage::{MappingSource, Tests};

#[tokio::test]
async fn browsing_cache_rows_stay_out_of_the_library() {
    let storage = Tests::new_in_memory().await;

    // 1: metadata cached by opening a detail view — no list entry, no files.
    storage.insert_minimal_anime(1, "Browsed Only").await.unwrap();
    // 2: actually in the list.
    storage.insert_minimal_anime(2, "Listed Show").await.unwrap();
    storage
        .upsert_list_entry_full(2, "watching", 1, None, "", 1000, 1000)
        .await
        .unwrap();
    // 3: not listed, but has a mapped local file — the legitimate "Unlisted" case.
    storage.insert_minimal_anime(3, "Files Only").await.unwrap();
    storage
        .upsert_file_index("D:/anime/files-only/ep1.mkv", Some(3), 1, 90, MappingSource::Automatic, 1000)
        .await
        .unwrap();

    let results = storage.search_library("", None, 10, 0).await.unwrap();
    let mut ids: Vec<i64> = results.iter().map(|r| r.anime_id).collect();
    ids.sort();
    assert_eq!(ids, vec![2, 3], "cache-only anime must not appear in the library");

    let stats = storage.library_stats().await.unwrap();
    assert_eq!(stats.total, 2, "cache-only anime must not count toward the library total");
    assert_eq!(stats.watching, 1);
}

#[tokio::test]
async fn search_library_finds_by_title() {
    let storage = Tests::new_in_memory().await;

    storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();
    storage
        .upsert_list_entry_full(1, "watching", 5, None, "", 1000, 1000)
        .await
        .unwrap();

    let results = storage
        .search_library("bebop", None, 10, 0)
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].title.contains("Cowboy"));
    assert_eq!(results[0].status, "watching");
    assert_eq!(results[0].watched_episodes, 5);
}

#[tokio::test]
async fn library_stats_counts_statuses() {
    let storage = Tests::new_in_memory().await;

    for id in 1..=3 {
        storage
            .insert_minimal_anime(id, &format!("Anime {}", id))
            .await
            .unwrap();
    }

    storage
        .upsert_list_entry_full(1, "watching", 5, None, "", 1000, 1000)
        .await
        .unwrap();
    storage
        .upsert_list_entry_full(2, "watching", 10, None, "", 1000, 1000)
        .await
        .unwrap();
    storage
        .upsert_list_entry_full(3, "completed", 26, None, "", 1000, 1000)
        .await
        .unwrap();

    let stats = storage.library_stats().await.unwrap();

    assert_eq!(stats.total, 3);
    assert_eq!(stats.watching, 2);
    assert_eq!(stats.completed, 1);
    assert_eq!(stats.on_hold, 0);
    assert_eq!(stats.dropped, 0);
    assert_eq!(stats.plan_to_watch, 0);
}

#[tokio::test]
async fn all_library_ids_excludes_unlisted_anime() {
    let storage = Tests::new_in_memory().await;

    // Anime cached locally (e.g. browsed) but never added to the list.
    storage.insert_minimal_anime(1, "Unlisted Anime").await.unwrap();
    // Anime actually in the library, with a higher anime.id than the
    // unlisted one above — this mirrors AniList ids for newer seasonal
    // shows, which are numerically larger than older cached entries.
    storage.insert_minimal_anime(2, "Plan To Watch Anime").await.unwrap();
    storage
        .upsert_list_entry_full(2, "plan_to_watch", 0, None, "", 1000, 1000)
        .await
        .unwrap();

    let ids = storage.all_library_ids().await.unwrap();

    assert_eq!(ids, vec![2]);
}

#[tokio::test]
async fn anime_detail_returns_full_row() {
    let storage = Tests::new_in_memory().await;

    storage
        .upsert_anime(
            1,
            r#"{"romaji":"Cowboy Bebop"}"#,
            26,
            Some("https://img.jpg"),
            1000,
        )
        .await
        .unwrap();

    storage
        .upsert_list_entry_full(1, "watching", 5, Some(85), "Great", 2000, 3000)
        .await
        .unwrap();

    storage
        .upsert_tracker_mapping(1, "anilist", "42")
        .await
        .unwrap();

    let detail = storage.anime_detail(1).await.unwrap();

    assert_eq!(detail.anime_id, 1);
    assert_eq!(detail.list_status, Some("watching".to_string()));
    assert_eq!(detail.tracker_id, Some("42".to_string()));
    assert_eq!(detail.episode_count, Some(26));
    assert_eq!(detail.score, Some(85));
    assert_eq!(detail.watched_episodes, Some(5));
}

#[tokio::test]
async fn episode_meta_backfill_storage() {
    let storage = Tests::new_in_memory().await;

    // Anime 1: in the library but with unknown episode count / airing status.
    storage.insert_minimal_anime(1, "Show A").await.unwrap();
    storage
        .update_list_entry_partial(1, Some("watching"), Some(3), None)
        .await
        .unwrap();
    // Anime 2: known count + status — should NOT be a backfill candidate.
    storage
        .upsert_anime_full(2, r#"{"romaji":"Show B"}"#, 12, None, None, None, Some("FINISHED"), 1000)
        .await
        .unwrap();
    storage
        .update_list_entry_partial(2, Some("watching"), Some(1), None)
        .await
        .unwrap();

    let missing = storage.library_anime_missing_meta(50).await.unwrap();
    assert_eq!(missing, vec![1], "only the anime with unknown metadata is a candidate");

    storage
        .update_anime_episode_meta(1, Some(13), Some("RELEASING"), 2000)
        .await
        .unwrap();

    assert!(
        storage.library_anime_missing_meta(50).await.unwrap().is_empty(),
        "a backfilled anime is no longer a candidate"
    );
    let detail = storage.anime_detail(1).await.unwrap();
    assert_eq!(detail.episode_count, Some(13));
}

#[tokio::test]
async fn update_list_entry_partial_changes_only_status() {
    let storage = Tests::new_in_memory().await;

    storage.insert_minimal_anime(1, "Test").await.unwrap();

    // Insert initial list entry with watching + 5 episodes via the partial method (UPSERT)
    storage
        .update_list_entry_partial(1, Some("watching"), Some(5), None)
        .await
        .unwrap();

    // Change only status to completed, keep watched_episodes and score unchanged
    storage
        .update_list_entry_partial(1, Some("completed"), None, None)
        .await
        .unwrap();

    // Fetch via existing public method
    let entry = storage.get_list_entry(1).await.unwrap().unwrap();
    assert_eq!(entry.status, "completed");
    assert_eq!(entry.watched_episodes, 5);
}
