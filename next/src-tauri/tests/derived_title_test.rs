//! Storage-level behaviour of the derived English display title.
//!
//! The property these tests defend is that deriving a title can never damage an
//! entry: AniList's own `english` always wins, and clearing `english_derived`
//! restores exactly the pre-feature display.

use anivault_core::engine::storage::Storage;

async fn new_storage() -> Storage {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage
}

fn titles(romaji: &str, english: Option<&str>) -> String {
    serde_json::json!({
        "romaji": romaji,
        "english": english,
        "japanese": null,
        "synonyms": [],
    })
    .to_string()
}

async fn in_library(storage: &Storage, id: i64) {
    storage
        .upsert_list_entry_full(id, "watching", 0, None, "", 1000, 1000)
        .await
        .unwrap();
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn derived_of(titles_json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(titles_json).unwrap();
    v["english_derived"].as_str().map(String::from)
}

/// A throwaway data directory holding a freshly written AniDB dump.
struct DumpDir(std::path::PathBuf);

impl DumpDir {
    fn new(name: &str, dump: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("anivault-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("anidb-titles.dat"), dump).unwrap();
        DumpDir(dir)
    }
}

impl Drop for DumpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const DUMP: &str = "# <aid>|<type>|<language>|<title>\n100|1|x-jat|Sekai Saikyou no Majo, Hajimemashita\n100|4|en|The Strongest Witch Begins\n";

#[tokio::test]
async fn candidates_are_only_rows_with_no_english_title() {
    let storage = new_storage().await;
    storage
        .upsert_anime(1, &titles("Sousou no Frieren 3rd Season", None), 12, None, 1000)
        .await
        .unwrap();
    storage
        .upsert_anime(2, &titles("Kimetsu no Yaiba", Some("Demon Slayer")), 26, None, 1000)
        .await
        .unwrap();
    // An empty string counts as missing, not as a title.
    storage
        .upsert_anime(3, &titles("Dandadan 3rd Season", Some("")), 12, None, 1000)
        .await
        .unwrap();
    for id in [1, 2, 3] {
        in_library(&storage, id).await;
    }

    let got = storage.anime_missing_english_title(50).await.unwrap();
    let ids: Vec<i64> = got.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, vec![1, 3]);
    assert_eq!(got[0].1, "Sousou no Frieren 3rd Season");
}

#[tokio::test]
async fn deriving_a_title_leaves_the_anilist_english_field_untouched() {
    let storage = new_storage().await;
    storage
        .upsert_anime(1, &titles("Sousou no Frieren 3rd Season", None), 12, None, 1000)
        .await
        .unwrap();

    storage
        .set_anime_derived_english(1, "Frieren: Beyond Journey's End Season 3")
        .await
        .unwrap();

    let row = storage.fetch_anime(1).await.unwrap().unwrap();
    let v: serde_json::Value = serde_json::from_str(&row.titles_json).unwrap();
    assert_eq!(
        v["english_derived"].as_str(),
        Some("Frieren: Beyond Journey's End Season 3")
    );
    assert!(v["english"].is_null(), "AniList's english field must stay authoritative");
    assert_eq!(v["romaji"].as_str(), Some("Sousou no Frieren 3rd Season"));
}

#[tokio::test]
async fn a_row_that_already_has_a_derived_title_is_not_a_candidate_again() {
    let storage = new_storage().await;
    storage
        .upsert_anime(1, &titles("Sousou no Frieren 3rd Season", None), 12, None, 1000)
        .await
        .unwrap();

    in_library(&storage, 1).await;
    assert_eq!(storage.anime_missing_english_title(50).await.unwrap().len(), 1);
    storage.set_anime_derived_english(1, "Frieren: Beyond Journey's End Season 3").await.unwrap();
    assert!(
        storage.anime_missing_english_title(50).await.unwrap().is_empty(),
        "the pass must be incremental, not redo work every cycle"
    );
}

#[tokio::test]
async fn watch_history_search_binds_every_placeholder() {
    // This query mixes anonymous `?` placeholders with LIMIT/OFFSET, so an
    // under-bound pattern lands in the integer LIMIT slot and fails at runtime
    // (SQLITE_MISMATCH) rather than at compile time. Exercise it directly.
    let storage = new_storage().await;
    storage
        .upsert_anime(1, &titles("Sousou no Frieren 3rd Season", None), 12, None, 1000)
        .await
        .unwrap();
    storage.set_anime_derived_english(1, "Frieren: Beyond Journey's End Season 3").await.unwrap();
    storage
        .append_watch_history(1, 1, Some("C:/anime/ep1.mkv"), Some("mpc"), "manual", 5000)
        .await
        .unwrap();

    let by_derived = storage.search_watch_history("Beyond Journey", 20, 0).await.unwrap();
    assert_eq!(by_derived.len(), 1);
    assert_eq!(by_derived[0].anime_title, "Frieren: Beyond Journey's End Season 3");

    let by_romaji = storage.search_watch_history("Sousou", 20, 0).await.unwrap();
    assert_eq!(by_romaji.len(), 1, "the romaji must stay searchable too");

    assert!(storage.search_watch_history("Cowboy Bebop", 20, 0).await.unwrap().is_empty());
}

#[tokio::test]
async fn library_search_matches_the_title_the_user_actually_sees() {
    let storage = new_storage().await;
    storage
        .upsert_anime(
            1,
            &titles("Boku no Kokoro no Yabai Yatsu 3rd Season", None),
            12,
            None,
            1000,
        )
        .await
        .unwrap();
    storage.upsert_list_entry_full(1, "watching", 0, None, "", 1000, 1000).await.unwrap();

    // Nothing in the romaji contains "Dangers", so without the derived title
    // being searchable the user cannot find the row by its displayed name.
    let before = storage.search_library("Dangers", None, 20, 0).await.unwrap();
    assert!(before.is_empty());

    storage.set_anime_derived_english(1, "The Dangers in My Heart Season 3").await.unwrap();

    let after = storage.search_library("Dangers", None, 20, 0).await.unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].title, "The Dangers in My Heart Season 3");
}

#[tokio::test]
async fn candidates_are_limited_to_library_rows() {
    let storage = new_storage().await;
    storage
        .upsert_anime(1, &titles("Sousou no Frieren 3rd Season", None), 12, None, 1000)
        .await
        .unwrap();
    storage
        .upsert_anime(2, &titles("Boku no Kokoro no Yabai Yatsu 3rd Season", None), 12, None, 1000)
        .await
        .unwrap();
    in_library(&storage, 2).await;

    let ids: Vec<i64> = storage
        .anime_missing_english_title(50)
        .await
        .unwrap()
        .iter()
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(ids, vec![2]);
}

#[tokio::test]
async fn recently_checked_rows_leave_the_window_until_the_check_goes_stale() {
    let storage = new_storage().await;
    storage
        .upsert_anime(1, &titles("Sousou no Frieren 3rd Season", None), 12, None, 1000)
        .await
        .unwrap();
    in_library(&storage, 1).await;

    storage.mark_derived_title_checked(&[1], now()).await.unwrap();
    assert!(storage.anime_missing_english_title(50).await.unwrap().is_empty());

    storage
        .mark_derived_title_checked(&[1], now() - 8 * 24 * 60 * 60)
        .await
        .unwrap();
    assert_eq!(storage.anime_missing_english_title(50).await.unwrap().len(), 1);
}

#[tokio::test]
async fn backfill_marks_rows_it_could_not_decide_so_the_window_advances() {
    use anivault_core::engine::anilist::import::backfill_derived_titles;
    let storage = new_storage().await;
    // Passes the romaji gate but has no relation or AniDB source here.
    storage
        .upsert_anime(1, &titles("Sousou no Frieren 3rd Season", None), 12, None, 1000)
        .await
        .unwrap();
    // Fails the gate: the romaji is already the English name.
    storage
        .upsert_anime(2, &titles("Madlax", None), 26, None, 1000)
        .await
        .unwrap();
    in_library(&storage, 1).await;
    in_library(&storage, 2).await;

    let derived = backfill_derived_titles(&storage, None, None, 100).await.unwrap();
    assert_eq!(derived, 0);
    assert!(
        storage.anime_missing_english_title(50).await.unwrap().is_empty(),
        "attempted rows must not occupy the window on the next cycle"
    );
}

#[tokio::test]
async fn anilist_upserts_keep_the_derived_title_and_check_marker() {
    let storage = new_storage().await;
    storage
        .upsert_anime(1, &titles("Sousou no Frieren 3rd Season", None), 12, None, 1000)
        .await
        .unwrap();
    storage
        .set_anime_derived_english(1, "Frieren: Beyond Journey's End Season 3")
        .await
        .unwrap();
    storage.mark_derived_title_checked(&[1], 4242).await.unwrap();

    storage
        .upsert_anime_full(
            1,
            &titles("Sousou no Frieren 3rd Season", None),
            12,
            None,
            Some("synopsis"),
            Some("TV"),
            Some("RELEASING"),
            2000,
        )
        .await
        .unwrap();
    let row = storage.fetch_anime(1).await.unwrap().unwrap();
    assert_eq!(
        derived_of(&row.titles_json).as_deref(),
        Some("Frieren: Beyond Journey's End Season 3")
    );
    let v: serde_json::Value = serde_json::from_str(&row.titles_json).unwrap();
    assert_eq!(v["english_derived_checked"], 4242);

    storage
        .upsert_anime(1, &titles("Sousou no Frieren 3rd Season", None), 12, None, 3000)
        .await
        .unwrap();
    let row = storage.fetch_anime(1).await.unwrap().unwrap();
    assert_eq!(
        derived_of(&row.titles_json).as_deref(),
        Some("Frieren: Beyond Journey's End Season 3")
    );
}

#[tokio::test]
async fn an_upsert_of_a_row_without_a_derived_title_adds_no_null_keys() {
    let storage = new_storage().await;
    storage
        .upsert_anime_full(1, &titles("Madlax", None), 26, None, None, None, None, 1000)
        .await
        .unwrap();
    storage
        .upsert_anime_full(1, &titles("Madlax", None), 26, None, None, None, None, 2000)
        .await
        .unwrap();
    let row = storage.fetch_anime(1).await.unwrap().unwrap();
    let v: serde_json::Value = serde_json::from_str(&row.titles_json).unwrap();
    assert!(v.get("english_derived").is_none(), "got {v}");
}

#[tokio::test]
async fn anidb_pass_runs_without_an_anilist_client() {
    use anivault_core::engine::anilist::import::backfill_derived_titles;
    let storage = new_storage().await;
    storage
        .upsert_anime(100, &titles("Sekai Saikyou no Majo, Hajimemashita", None), 12, None, 1000)
        .await
        .unwrap();
    in_library(&storage, 100).await;
    let dir = DumpDir::new("anidb-pass", DUMP);

    let derived = backfill_derived_titles(&storage, None, Some(&dir.0), 100)
        .await
        .unwrap();

    assert_eq!(derived, 1);
    let row = storage.fetch_anime(100).await.unwrap().unwrap();
    assert_eq!(derived_of(&row.titles_json).as_deref(), Some("The Strongest Witch Begins"));
}

#[tokio::test]
async fn the_anidb_dump_is_parsed_once_and_then_served_from_memory() {
    use anivault_core::engine::anidb_titles::load_or_refresh;
    let storage = new_storage().await;
    let dir = DumpDir::new("anidb-cache", DUMP);

    let first = load_or_refresh(&storage, &dir.0).await.expect("cached dump loads");
    let second = load_or_refresh(&storage, &dir.0).await.expect("cached dump loads");
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    assert_eq!(
        first.english_for("Sekai Saikyou no Majo, Hajimemashita"),
        Some("The Strongest Witch Begins")
    );
}
