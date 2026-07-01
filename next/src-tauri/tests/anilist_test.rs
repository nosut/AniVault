use anivault_core::engine::anilist::{parse_anilist_search_response, score_anilist_match, AniListSearchResult};
use anivault_core::engine::recognition::matcher::build_fts_index;
use anivault_core::engine::storage::Storage;

const MOCK_ANILIST_RESPONSE: &str = r#"{
    "data": {
        "Page": {
            "media": [
                {
                    "id": 140960,
                    "title": {
                        "romaji": "Dandadan",
                        "english": null
                    },
                "synonyms": [],
                "episodes": 12,
                "coverImage": {
                    "large": "https://example.com/dandadan.jpg"
                }
                }
            ]
        }
    }
}"#;

#[test]
fn parses_anilist_search_response() {
    let results = parse_anilist_search_response(MOCK_ANILIST_RESPONSE).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].anilist_id, 140960);
    assert_eq!(results[0].title_romaji, "Dandadan");
    assert_eq!(results[0].episode_count, Some(12));
    assert_eq!(results[0].image_url.as_deref(), Some("https://example.com/dandadan.jpg"));
}

#[test]
fn parses_empty_anilist_response() {
    let empty = r#"{"data":{"Page":{"media":[]}}}"#;
    let results = parse_anilist_search_response(empty).unwrap();
    assert!(results.is_empty());
}

#[test]
fn exact_romaji_match_scores_high() {
    let result = AniListSearchResult {
        anilist_id: 1,
        title_romaji: "Spy x Family".into(),
        title_english: None,
        synonyms: vec![],
        episode_count: None,
        image_url: None,
    };

    let score = score_anilist_match("Spy x Family", &result);
    assert!(score >= 85, "exact romaji match should be >=85, got {score}");
}

#[test]
fn english_title_match_scores_medium() {
    let result = AniListSearchResult {
        anilist_id: 2,
        title_romaji: "Kusuriya no Hitorigoto".into(),
        title_english: Some("The Apothecary Diaries".into()),
        synonyms: vec![],
        episode_count: None,
        image_url: None,
    };

    let score = score_anilist_match("The Apothecary Diaries", &result);
    assert!(score >= 70, "english match should be >=70, got {score}");
    assert!(score < 85, "english match should be <85, got {score}");
}

#[test]
fn synonym_match_scores_low() {
    let result = AniListSearchResult {
        anilist_id: 3,
        title_romaji: "Oshi no Ko".into(),
        title_english: None,
        synonyms: vec!["My Star".into()],
        episode_count: None,
        image_url: None,
    };

    let score = score_anilist_match("My Star", &result);
    assert!(score >= 50, "synonym match should be >=50, got {score}");
    assert!(score < 70, "synonym match should be <70, got {score}");
}

#[test]
fn unrelated_match_scores_under_50() {
    let result = AniListSearchResult {
        anilist_id: 99,
        title_romaji: "Naruto".into(),
        title_english: None,
        synonyms: vec![],
        episode_count: None,
        image_url: None,
    };

    let score = score_anilist_match("Spy x Family", &result);
    assert!(score < 50, "unrelated match should be <50, got {score}");
}

#[tokio::test]
async fn auto_add_inserts_anime_and_rebuilds_fts() {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage.ensure_fts_index().await.unwrap();

    let result = AniListSearchResult {
        anilist_id: 42,
        title_romaji: "Cowboy Bebop".into(),
        title_english: None,
        synonyms: vec![],
        episode_count: Some(26),
        image_url: None,
    };

    let anime_id = anivault_core::engine::anilist::auto_add_anime(&storage, &result)
        .await
        .unwrap();
    assert_eq!(anime_id, 42);

    let row = storage.anime_by_id(42).await.unwrap().unwrap();
    assert_eq!(row.1, "Cowboy Bebop");

    // Verify FTS index rebuilt after insert
    build_fts_index(&storage).await.unwrap();
    let parsed = anivault_core::engine::recognition::parser::parse_filename("Cowboy Bebop - 03.mkv");
    let matched = anivault_core::engine::recognition::matcher::search_local(&storage, &parsed)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(matched.anime_id, 42);
    assert_eq!(matched.title, "Cowboy Bebop");
}
