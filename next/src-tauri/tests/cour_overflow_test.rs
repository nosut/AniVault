//! Sonarr often keeps a show's 2nd cour as episodes 14+ of its first season,
//! in the same `Season 1` folder, while AniList lists the cour as its own
//! entry. Those episodes belong to the 2nd cour entry, renumbered from 1.

use anivault_core::engine::events::EngineEvent;
use anivault_core::engine::library_scanner::{rescan_anime_dirs, scan_specific_dirs};
use anivault_core::engine::runtime::{fresh_test_state, EngineState};
use anivault_core::engine::scanner::ScanResult;
use anivault_core::engine::session::process_scan_result;
use anivault_core::engine::storage::MappingSource;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const COUR1: i64 = 180082;
const COUR2: i64 = 198727;

fn unique_temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("anivault_cour_{tag}_{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

async fn add_part(
    state: &EngineState,
    id: i64,
    english: &str,
    romaji: &str,
    episodes: i32,
    year: i32,
    format: &str,
) {
    let titles = serde_json::json!({ "english": english, "romaji": romaji, "synonyms": ["Chiramune"] });
    state
        .storage
        .upsert_anime(id, &titles.to_string(), episodes, None, 0)
        .await
        .unwrap();
    state
        .storage
        .set_anime_season(id, Some("FALL"), Some(year))
        .await
        .unwrap();
    state.storage.set_anime_format(id, Some(format)).await.unwrap();
}

/// The Chitose library as Sonarr lays it out: one `Season 1` folder holding
/// episodes 1-13 (mapped to the first cour) and the 2nd cour's 14+.
async fn chitose_fixture(tag: &str) -> (EngineState, PathBuf, PathBuf) {
    let state = fresh_test_state().await;
    add_part(
        &state,
        COUR1,
        "Chitose Is in the Ramune Bottle",
        "Chitose-kun wa Ramune Bin no Naka",
        13,
        2025,
        "TV",
    )
    .await;
    add_part(
        &state,
        COUR2,
        "Chitose Is in the Ramune Bottle 2nd Cour",
        "Chitose-kun wa Ramune Bin no Naka Part 2",
        11,
        2026,
        "TV",
    )
    .await;
    // A film sharing the title must never take overflow episodes.
    add_part(
        &state,
        1,
        "Chitose Is in the Ramune Bottle Movie",
        "Chitose-kun wa Ramune Bin no Naka Movie",
        1,
        2025,
        "MOVIE",
    )
    .await;

    let root = unique_temp_dir(tag);
    let season = root.join("Chitose Is in the Ramune Bottle").join("Season 1");
    fs::create_dir_all(&season).unwrap();
    for ep in 1..=13 {
        let f = season.join(format!("Chitose Is in the Ramune Bottle - S01E{ep:02} - Title.mkv"));
        fs::write(&f, b"x").unwrap();
        state
            .storage
            .upsert_file_index(&f.to_string_lossy(), Some(COUR1), ep, 100, MappingSource::Legacy, now())
            .await
            .unwrap();
    }
    (state, root, season)
}

fn episode_file(season: &std::path::Path, ep: i32) -> PathBuf {
    season.join(format!("Chitose Is in the Ramune Bottle - S01E{ep:02} - Title.mkv"))
}

#[tokio::test]
async fn scan_puts_episodes_past_the_first_cour_on_the_second() {
    let (state, root, season) = chitose_fixture("scan").await;
    let e14 = episode_file(&season, 14);
    let e15 = episode_file(&season, 15);
    fs::write(&e14, b"x").unwrap();
    fs::write(&e15, b"x").unwrap();

    scan_specific_dirs(&state.storage, &[season.to_string_lossy().to_string()])
        .await
        .unwrap();

    for (file, ep) in [(&e14, 1), (&e15, 2)] {
        let row = state.storage.get_file_index(&file.to_string_lossy()).await.unwrap().unwrap();
        assert_eq!((row.anime_id, row.episode), (Some(COUR2), Some(ep)), "{}", file.display());
    }
    // The first cour keeps its own episodes.
    let e13 = state
        .storage
        .get_file_index(&episode_file(&season, 13).to_string_lossy())
        .await
        .unwrap()
        .unwrap();
    assert_eq!((e13.anime_id, e13.episode), (Some(COUR1), Some(13)));
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn scan_moves_overflow_episodes_already_indexed_on_the_first_cour() {
    let (state, root, season) = chitose_fixture("repair").await;
    let e14 = episode_file(&season, 14);
    fs::write(&e14, b"x").unwrap();
    // How earlier versions indexed it: inherited from the folder, episode 14.
    state
        .storage
        .upsert_file_index(&e14.to_string_lossy(), Some(COUR1), 14, 85, MappingSource::Inherited, now())
        .await
        .unwrap();

    scan_specific_dirs(&state.storage, &[season.to_string_lossy().to_string()])
        .await
        .unwrap();

    let row = state.storage.get_file_index(&e14.to_string_lossy()).await.unwrap().unwrap();
    assert_eq!((row.anime_id, row.episode), (Some(COUR2), Some(1)));
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn scan_leaves_a_manual_overflow_mapping_alone() {
    let (state, root, season) = chitose_fixture("manual").await;
    let e14 = episode_file(&season, 14);
    fs::write(&e14, b"x").unwrap();
    state
        .storage
        .upsert_file_index(&e14.to_string_lossy(), Some(COUR1), 14, 100, MappingSource::Manual, now())
        .await
        .unwrap();

    scan_specific_dirs(&state.storage, &[season.to_string_lossy().to_string()])
        .await
        .unwrap();

    let row = state.storage.get_file_index(&e14.to_string_lossy()).await.unwrap().unwrap();
    assert_eq!((row.anime_id, row.episode), (Some(COUR1), Some(14)));
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn episode_past_every_known_part_stays_where_it_was() {
    let (state, root, season) = chitose_fixture("past_end").await;
    // 13 + 11 = 24 episodes known; a 30th has nowhere to go.
    let e30 = episode_file(&season, 30);
    fs::write(&e30, b"x").unwrap();

    scan_specific_dirs(&state.storage, &[season.to_string_lossy().to_string()])
        .await
        .unwrap();

    let row = state.storage.get_file_index(&e30.to_string_lossy()).await.unwrap().unwrap();
    assert_eq!((row.anime_id, row.episode), (Some(COUR1), Some(30)));
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn rescanning_either_cour_reports_no_conflicts_for_the_shared_folder() {
    let (state, root, season) = chitose_fixture("conflicts").await;
    fs::write(episode_file(&season, 14), b"x").unwrap();
    scan_specific_dirs(&state.storage, &[season.to_string_lossy().to_string()])
        .await
        .unwrap();

    let cour1 = rescan_anime_dirs(&state.storage, COUR1).await.unwrap();
    assert!(cour1.mapping_conflicts.is_empty(), "{:?}", cour1.mapping_conflicts);
    let cour2 = rescan_anime_dirs(&state.storage, COUR2).await.unwrap();
    assert!(cour2.mapping_conflicts.is_empty(), "{:?}", cour2.mapping_conflicts);
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn a_season_2_file_on_season_1_is_still_a_conflict() {
    let state = fresh_test_state().await;
    let root = unique_temp_dir("skeleton");
    let season = root.join("Skeleton Knight in Another World").join("Season 2");
    fs::create_dir_all(&season).unwrap();
    let ep1 = season.join("Skeleton Knight in Another World - S02E01.mkv");
    let ep2 = season.join("Skeleton Knight in Another World - S02E02.mkv");
    fs::write(&ep1, b"x").unwrap();
    fs::write(&ep2, b"x").unwrap();
    for (id, title, year) in [
        (132474, "Skeleton Knight in Another World", 2022),
        (185542, "Skeleton Knight in Another World Season 2", 2026),
    ] {
        let titles = serde_json::json!({ "english": title, "romaji": title });
        state.storage.upsert_anime(id, &titles.to_string(), 12, None, 0).await.unwrap();
        state.storage.set_anime_season(id, Some("SPRING"), Some(year)).await.unwrap();
        state.storage.set_anime_format(id, Some("TV")).await.unwrap();
    }
    state
        .storage
        .upsert_file_index(&ep1.to_string_lossy(), Some(185542), 1, 100, MappingSource::Manual, now())
        .await
        .unwrap();
    state
        .storage
        .upsert_file_index(&ep2.to_string_lossy(), Some(132474), 2, 100, MappingSource::Legacy, now())
        .await
        .unwrap();

    let report = rescan_anime_dirs(&state.storage, 185542).await.unwrap();
    assert_eq!(report.mapping_conflicts.len(), 1);
    assert_eq!(report.mapping_conflicts[0].file_path, ep2.to_string_lossy());
    assert_eq!(report.mapping_conflicts[0].episode, Some(2));
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn repairing_onto_the_second_cour_renumbers_the_episode() {
    let (state, root, season) = chitose_fixture("repair_conflict").await;
    let e14 = episode_file(&season, 14);
    fs::write(&e14, b"x").unwrap();
    // Wrongly mapped to an unrelated show; the 2nd cour already has a file
    // here, so its rescan covers this folder.
    state.storage.insert_minimal_anime(42, "Unrelated Show").await.unwrap();
    state
        .storage
        .upsert_file_index(&e14.to_string_lossy(), Some(42), 14, 100, MappingSource::Legacy, now())
        .await
        .unwrap();
    let e15 = episode_file(&season, 15);
    fs::write(&e15, b"x").unwrap();
    state
        .storage
        .upsert_file_index(&e15.to_string_lossy(), Some(COUR2), 2, 100, MappingSource::Manual, now())
        .await
        .unwrap();

    let report = rescan_anime_dirs(&state.storage, COUR2).await.unwrap();
    assert_eq!(report.mapping_conflicts.len(), 1, "{:?}", report.mapping_conflicts);
    assert_eq!(report.mapping_conflicts[0].episode, Some(1));
    fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn playing_an_indexed_file_records_its_indexed_episode() {
    let (state, root, season) = chitose_fixture("playback").await;
    let e14 = episode_file(&season, 14);
    fs::write(&e14, b"x").unwrap();
    scan_specific_dirs(&state.storage, &[season.to_string_lossy().to_string()])
        .await
        .unwrap();
    state.events.drain();

    let name = e14.file_name().unwrap().to_string_lossy().to_string();
    process_scan_result(
        &state,
        ScanResult {
            player_name: "mpv.exe".to_string(),
            file_path: Some(e14.to_string_lossy().to_string()),
            window_title: Some(name),
            detected_at_unix: 1_782_769_000,
        },
    )
    .await
    .unwrap();

    let identified: Vec<(i64, i32)> = state
        .events
        .drain()
        .into_iter()
        .filter_map(|e| match e {
            EngineEvent::AnimeIdentified(a) => Some((a.anime_id, a.episode)),
            _ => None,
        })
        .collect();
    assert_eq!(identified, vec![(COUR2, 1)]);
    fs::remove_dir_all(root).ok();
}
