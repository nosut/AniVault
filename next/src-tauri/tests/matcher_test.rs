use anivault_core::engine::recognition::matcher::{build_fts_index, search_local};
use anivault_core::engine::recognition::parser::parse_filename;
use anivault_core::engine::storage::Storage;

#[tokio::test]
async fn match_exact_romaji_title() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.ensure_fts_index().await.unwrap();
    storage.insert_minimal_anime(1, "Spy x Family").await.unwrap();
    build_fts_index(&storage).await.unwrap();

    let parsed = parse_filename("[SubsPlease] Spy x Family - 17.mkv");
    let result = search_local(&storage, &parsed).await.unwrap().unwrap();

    assert_eq!(result.anime_id, 1);
    assert_eq!(result.title, "Spy x Family");
    assert!(result.confidence >= 85);
    assert_eq!(result.source, "local_exact");
}

#[tokio::test]
async fn match_synonym_title() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.ensure_fts_index().await.unwrap();
    storage
        .insert_minimal_anime_with_synonyms(2, "Kusuriya no Hitorigoto", &["The Apothecary Diaries"])
        .await
        .unwrap();
    build_fts_index(&storage).await.unwrap();

    let parsed = parse_filename("The Apothecary Diaries - 24.mkv");
    let result = search_local(&storage, &parsed).await.unwrap().unwrap();

    assert_eq!(result.anime_id, 2);
    assert_eq!(result.title, "Kusuriya no Hitorigoto");
    assert!(result.confidence >= 70);
}

#[tokio::test]
async fn no_match_unknown_title() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.ensure_fts_index().await.unwrap();
    build_fts_index(&storage).await.unwrap();

    let parsed = parse_filename("Nonexistent Anime - 01.mkv");
    let result = search_local(&storage, &parsed).await.unwrap();

    assert!(result.is_none());
}
