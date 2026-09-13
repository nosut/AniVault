//! Bulk storage reads that replace per-row query loops, and the query fixes
//! that ride along with them.

use anivault_core::engine::storage::{MappingSource, SonarrMappingDb, SonarrSeriesDb, Storage, Tests};

async fn anime(storage: &Storage, id: i64, title: &str) {
    storage.insert_minimal_anime(id, title).await.unwrap();
}

async fn file(storage: &Storage, path: &str, anime_id: Option<i64>, episode: i32) {
    storage
        .upsert_file_index(path, anime_id, episode, 100, MappingSource::Manual, 1000)
        .await
        .unwrap();
}

#[tokio::test]
async fn file_index_for_anime_ids_matches_the_per_anime_query() {
    let storage = Tests::new_in_memory().await;
    anime(&storage, 1, "One").await;
    anime(&storage, 2, "Two").await;
    anime(&storage, 3, "Three").await;
    file(&storage, "D:/a/One - 02.mkv", Some(1), 2).await;
    file(&storage, "D:/a/One - 01.mkv", Some(1), 1).await;
    file(&storage, "D:/a/Two - 01.mkv", Some(2), 1).await;
    file(&storage, "D:/a/Three - 01.mkv", Some(3), 1).await;

    let bulk = storage.file_index_for_anime_ids(&[1, 2, 99]).await.unwrap();

    assert_eq!(bulk.len(), 2, "only requested ids with rows: {:?}", bulk.keys());
    for id in [1, 2] {
        let single = storage.file_index_by_anime(id).await.unwrap();
        let paths = |rows: &[anivault_core::engine::storage::FileIndexRow]| {
            rows.iter()
                .map(|r| (r.file_path.clone(), r.episode, r.ignored))
                .collect::<Vec<_>>()
        };
        assert_eq!(paths(&bulk[&id]), paths(&single), "anime {id}");
    }
}

#[tokio::test]
async fn file_index_for_anime_ids_handles_more_ids_than_sqlite_parameters() {
    let storage = Tests::new_in_memory().await;
    anime(&storage, 1500, "Last").await;
    file(&storage, "D:/a/Last - 01.mkv", Some(1500), 1).await;
    let ids: Vec<i64> = (1..=1500).collect();

    let bulk = storage.file_index_for_anime_ids(&ids).await.unwrap();
    assert_eq!(bulk[&1500].len(), 1);
    assert!(storage.file_index_for_anime_ids(&[]).await.unwrap().is_empty());
}

#[tokio::test]
async fn watch_history_and_list_entries_in_bulk() {
    let storage = Tests::new_in_memory().await;
    anime(&storage, 1, "One").await;
    anime(&storage, 2, "Two").await;
    for (id, ep) in [(1, 1), (1, 2), (1, 2), (2, 5)] {
        storage
            .append_watch_history(id, ep, None, None, "manual", 1000)
            .await
            .unwrap();
    }
    storage
        .upsert_list_entry_progress(1, "watching", 2, 1000)
        .await
        .unwrap();

    let history = storage.watch_history_episodes_for(&[1, 2, 3]).await.unwrap();
    let mut eps1 = history[&1].clone();
    eps1.sort_unstable();
    assert_eq!(eps1, vec![1, 2]);
    assert_eq!(history[&2], vec![5]);
    assert!(!history.contains_key(&3));

    let entries = storage.list_entries_for(&[1, 2]).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[&1].watched_episodes, 2);
    assert_eq!(entries[&1].status, "watching");
}

#[tokio::test]
async fn file_index_rows_under_a_dir_take_like_metacharacters_literally() {
    let storage = Tests::new_in_memory().await;
    anime(&storage, 1, "One").await;
    file(&storage, "D:/100%_done/One - 01.mkv", Some(1), 1).await;
    file(&storage, "D:/100xxdone/One - 02.mkv", Some(1), 2).await;
    file(&storage, "D:/elsewhere/One - 03.mkv", None, 3).await;

    let rows = storage.file_index_rows_under("D:/100%_done").await.unwrap();
    let paths: Vec<&str> = rows.iter().map(|r| r.file_path.as_str()).collect();
    assert_eq!(paths, vec!["D:/100%_done/One - 01.mkv"]);
}

#[tokio::test]
async fn continue_watching_includes_shows_with_no_play_history() {
    let storage = Tests::new_in_memory().await;
    anime(&storage, 1, "Imported").await;
    anime(&storage, 2, "Played").await;
    // Imported from AniList as watching; never played in AniVault.
    storage
        .upsert_list_entry_progress(1, "watching", 3, 5000)
        .await
        .unwrap();
    storage
        .upsert_list_entry_progress(2, "watching", 1, 1000)
        .await
        .unwrap();
    storage
        .append_watch_history(2, 1, None, None, "manual", 2000)
        .await
        .unwrap();

    let rows = storage.continue_watching(10).await.unwrap();
    let ids: Vec<i64> = rows.iter().map(|r| r.anime_id).collect();
    assert_eq!(ids, vec![1, 2], "ordered by last activity");
    assert_eq!(rows[0].last_watched_at, 5000);
}

#[tokio::test]
async fn library_and_history_search_treat_percent_and_underscore_literally() {
    let storage = Tests::new_in_memory().await;
    anime(&storage, 1, "100% Pascal-sensei").await;
    anime(&storage, 2, "Cowboy Bebop").await;
    for id in [1, 2] {
        storage
            .upsert_list_entry_progress(id, "watching", 1, 1000)
            .await
            .unwrap();
        storage
            .append_watch_history(id, 1, None, None, "manual", 1000)
            .await
            .unwrap();
    }

    let pct = storage.search_library("%", None, 50, 0).await.unwrap();
    assert_eq!(pct.iter().map(|r| r.anime_id).collect::<Vec<_>>(), vec![1]);
    assert!(storage.search_library("_", None, 50, 0).await.unwrap().is_empty());

    let pct = storage.search_watch_history("%", 50, 0).await.unwrap();
    assert_eq!(pct.iter().map(|r| r.anime_id).collect::<Vec<_>>(), vec![1]);
    assert!(storage.search_watch_history("_", 50, 0).await.unwrap().is_empty());
}

fn series(sonarr_id: i64) -> SonarrSeriesDb {
    SonarrSeriesDb {
        sonarr_id,
        title: format!("Series {sonarr_id}"),
        season_count: 1,
        episode_count: 12,
        episode_file_count: 0,
        monitored: true,
        next_airing: None,
        path: None,
        poster_url: None,
        overview: None,
        network: None,
        status: None,
        added: 1_700_000_000,
        last_synced: 1_700_000_000,
    }
}

#[tokio::test]
async fn sonarr_mapping_by_anime_returns_the_most_recent_mapping() {
    let storage = Tests::new_in_memory().await;
    anime(&storage, 42, "Shared").await;
    for (sonarr_id, mapped_at) in [(1, 100), (2, 200)] {
        storage.sonarr_series_upsert(&series(sonarr_id)).await.unwrap();
        storage
            .sonarr_mapping_upsert(&SonarrMappingDb {
                id: None,
                sonarr_id,
                anime_id: Some(42),
                title_match: format!("Series {sonarr_id}"),
                confidence: 90,
                mapped_at,
                user_confirmed: false,
            })
            .await
            .unwrap();
    }

    let found = storage.sonarr_mapping_by_anime(42).await.unwrap().unwrap();
    assert_eq!(found.sonarr_id, 2);
}

#[tokio::test]
async fn a_negative_known_files_limit_returns_every_row() {
    let storage = Tests::new_in_memory().await;
    for i in 0..7 {
        file(&storage, &format!("D:/a/Show - {i:02}.mkv"), None, i).await;
    }
    assert_eq!(storage.list_known_files(-1, 0).await.unwrap().len(), 7);
}
