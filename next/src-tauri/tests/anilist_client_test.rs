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
