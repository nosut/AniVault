use taiga_next::engine::anilist::client::AniListClient;

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
