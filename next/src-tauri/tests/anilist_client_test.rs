use anivault_core::engine::anilist::client::AniListClient;

#[tokio::test]
async fn client_constructs_with_token() {
    let client = AniListClient::new("t".into());
    assert_eq!(client.token, "t");
}

#[tokio::test]
async fn push_progress_errors_on_bad_token() {
    let client = AniListClient::new("bad".into());
    assert!(client.push_progress(1, 5).await.is_err());
}

#[tokio::test]
async fn fetch_season_anime_errors_on_bad_token() {
    // The paginated season fetch still surfaces an auth error on the first
    // page instead of looping or panicking.
    let client = AniListClient::new("bad".into());
    assert!(client
        .fetch_season_anime("WINTER", 2026, None)
        .await
        .is_err());
}

#[test]
fn a_null_synonym_is_skipped_instead_of_failing_the_whole_response() {
    use anivault_core::engine::anilist::client::{Media, SearchAnimeResult};
    let media: Media =
        serde_json::from_str(r#"{"id":1,"synonyms":["Foo",null,"Bar"]}"#).expect("deserializes");
    assert_eq!(media.synonyms, Some(vec!["Foo".to_string(), "Bar".to_string()]));

    let search: SearchAnimeResult =
        serde_json::from_str(r#"{"id":2,"synonyms":[null]}"#).expect("deserializes");
    assert_eq!(search.synonyms, Some(vec![]));

    let absent: Media = serde_json::from_str(r#"{"id":3}"#).unwrap();
    assert_eq!(absent.synonyms, None);
    let null: Media = serde_json::from_str(r#"{"id":4,"synonyms":null}"#).unwrap();
    assert_eq!(null.synonyms, None);
}

#[test]
fn media_reads_format() {
    let m: anivault_core::engine::anilist::client::Media =
        serde_json::from_str(r#"{"id":1,"format":"MOVIE"}"#).unwrap();
    assert_eq!(m.format.as_deref(), Some("MOVIE"));
    let m: anivault_core::engine::anilist::client::Media = serde_json::from_str(r#"{"id":2}"#).unwrap();
    assert_eq!(m.format, None);
}
