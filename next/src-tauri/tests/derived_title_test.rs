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
