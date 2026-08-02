use anivault_core::engine::storage::Storage;

#[tokio::test]
async fn search_anime_by_title_exact() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();
    storage.insert_minimal_anime(2, "Cowboy Bebop: The Movie").await.unwrap();

    let results = storage.search_anime_by_title("Cowboy Bebop", 10).await.unwrap();
    assert!(results.len() >= 1);
    assert_eq!(results[0].id, 1);
}

#[tokio::test]
async fn search_anime_by_partial_title() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.insert_minimal_anime(1, "Attack on Titan").await.unwrap();
    storage.insert_minimal_anime(2, "Fullmetal Alchemist").await.unwrap();

    let results = storage.search_anime_by_title("Alchemist", 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 2);
}

/// A player's window title carries the episode title too, so the query is a long
/// bag of common words. Rows matching only one of them must not crowd the real
/// show out of the candidate pool: the pool is capped, so it has to be ordered by
/// how much of the query a row matches, not by id.
#[tokio::test]
async fn search_anime_by_title_ranks_by_token_matches_not_by_id() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();

    // 60 low-id shows that each match exactly one common token ("world").
    for id in 1..=60 {
        storage
            .insert_minimal_anime(id, &format!("Some Other World Story {id}"))
            .await
            .unwrap();
    }
    // The real show, with a high AniList id — it matches four tokens.
    storage
        .insert_minimal_anime(152_137, "No Longer Allowed in Another World")
        .await
        .unwrap();

    // The cleaned title mpv's window title reduces to, episode title included.
    let query = "No Longer Allowed in Another World Will You Sentence Me to Death Again";
    let results = storage.search_anime_by_title(query, 10).await.unwrap();

    assert!(
        results.iter().any(|a| a.id == 152_137),
        "the show matching four query tokens must survive the pool cap, but the pool held ids {:?}",
        results.iter().map(|a| a.id).collect::<Vec<_>>()
    );
    assert_eq!(
        results[0].id, 152_137,
        "the row matching the most query tokens should rank first"
    );
}
