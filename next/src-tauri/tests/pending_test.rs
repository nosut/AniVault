use anivault_core::engine::anilist::AniListSearchResult;
use anivault_core::engine::storage::Storage;

#[tokio::test]
async fn pending_match_stored_and_listed() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let result = AniListSearchResult {
        anilist_id: 42,
        title_romaji: "Dandadan".into(),
        title_english: None,
        synonyms: vec![],
        episode_count: Some(12),
        image_url: None,
    };

    anivault_core::engine::pending::store_pending_match(&storage, &result, "Dandadan", 75)
        .await
        .unwrap();

    let pending = anivault_core::engine::pending::get_pending_matches(&storage).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].anilist_id, 42);
    assert_eq!(pending[0].confidence, 75);
}

#[tokio::test]
async fn confirm_match_adds_to_db_and_clears_pending() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.ensure_fts_index().await.unwrap();

    let result = AniListSearchResult {
        anilist_id: 99,
        title_romaji: "Mushishi".into(),
        title_english: None,
        synonyms: vec![],
        episode_count: Some(26),
        image_url: None,
    };

    anivault_core::engine::pending::store_pending_match(&storage, &result, "Mushishi", 70)
        .await
        .unwrap();

    anivault_core::engine::pending::confirm_pending_match(&storage, 99).await.unwrap();

    let pending = anivault_core::engine::pending::get_pending_matches(&storage).await.unwrap();
    assert!(pending.is_empty());

    let anime = storage.anime_by_id(99).await.unwrap().unwrap();
    assert_eq!(anime.1, "Mushishi");
}

#[tokio::test]
async fn reject_match_removes_from_pending() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    let result = AniListSearchResult {
        anilist_id: 7,
        title_romaji: "Wrong Match".into(),
        title_english: None,
        synonyms: vec![],
        episode_count: None,
        image_url: None,
    };

    anivault_core::engine::pending::store_pending_match(&storage, &result, "Query Title", 55)
        .await
        .unwrap();

    anivault_core::engine::pending::reject_pending_match(&storage, 7).await.unwrap();

    let pending = anivault_core::engine::pending::get_pending_matches(&storage).await.unwrap();
    assert!(pending.is_empty());
}
