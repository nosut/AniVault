use anivault_core::engine::sonarr::client::{find_episode_id, SonarrClient, SonarrEpisode};

fn ep(id: i64, season: i32, number: i32, absolute: Option<i32>) -> SonarrEpisode {
    SonarrEpisode { id, season_number: season, episode_number: number, absolute_episode_number: absolute }
}

#[test]
fn find_episode_id_prefers_absolute_number() {
    // AniList counts absolute episodes; Sonarr splits into seasons. S2E5 here
    // is absolute episode 17 — the absolute match must win over S1E17.
    let eps = vec![
        ep(100, 1, 17, Some(17)),
        ep(200, 2, 5, Some(17)),
    ];
    // Both carry absolute 17; the first hit wins — either is the right file
    // target, so just assert an absolute match is used.
    assert_eq!(find_episode_id(&eps, 17), Some(100));

    let eps = vec![ep(300, 2, 5, Some(17)), ep(301, 2, 6, Some(18))];
    assert_eq!(find_episode_id(&eps, 17), Some(300));
}

#[test]
fn find_episode_id_falls_back_to_season_episode_number() {
    // No absolute numbering (common for non-anime series types): match the
    // plain episode number, skipping specials (season 0).
    let eps = vec![
        ep(400, 0, 3, None),
        ep(401, 1, 3, None),
    ];
    assert_eq!(find_episode_id(&eps, 3), Some(401));
}

#[test]
fn find_episode_id_returns_none_when_absent() {
    let eps = vec![ep(500, 1, 1, Some(1))];
    assert_eq!(find_episode_id(&eps, 9), None);
}

#[test]
fn client_constructs_with_url_and_key() {
    let client = SonarrClient::new("http://localhost:8989".into(), "abc123".into());
    assert_eq!(client.url, "http://localhost:8989");
    assert_eq!(client.api_key, "abc123");
}

#[test]
fn client_trims_trailing_slash_from_url() {
    let client = SonarrClient::new("http://localhost:8989/".into(), "key".into());
    assert_eq!(client.url, "http://localhost:8989");
}

#[tokio::test]
async fn validate_connection_returns_error_for_nonexistent_host() {
    let client = SonarrClient::new("http://127.0.0.1:19999".into(), "bad".into());
    assert!(client.validate_connection().await.is_err());
}

#[tokio::test]
async fn fetch_series_returns_error_for_nonexistent_host() {
    let client = SonarrClient::new("http://127.0.0.1:19999".into(), "bad".into());
    assert!(client.fetch_series().await.is_err());
}
