use anivault_core::engine::anilist::client::AniListHttpError;
use anivault_core::engine::sync_worker::{backoff_delay, is_terminal_sync_error, is_token_error};

#[test]
fn backoff_delay_grows_in_minutes_and_caps_at_six_hours() {
    assert_eq!(backoff_delay(0), 60);
    assert_eq!(backoff_delay(1), 5 * 60);
    assert_eq!(backoff_delay(2), 15 * 60);
    assert_eq!(backoff_delay(3), 60 * 60);
    assert_eq!(backoff_delay(4), 6 * 60 * 60);
    assert_eq!(backoff_delay(500), 6 * 60 * 60); // capped
}

fn http(status: u16, body: &str) -> anyhow::Error {
    anyhow::Error::new(AniListHttpError {
        status,
        body: body.to_string(),
    })
}

#[test]
fn client_errors_other_than_rate_limit_are_terminal() {
    assert!(is_terminal_sync_error(&http(
        400,
        r#"{"errors":[{"message":"validation"}]}"#
    )));
    assert!(is_terminal_sync_error(&http(404, "not found")));
    assert!(is_terminal_sync_error(&http(422, "")));
}

#[test]
fn rate_limit_server_and_network_errors_are_not_terminal() {
    assert!(!is_terminal_sync_error(&http(429, "slow down")));
    assert!(!is_terminal_sync_error(&http(500, "oops")));
    assert!(!is_terminal_sync_error(&http(503, "")));
    assert!(!is_terminal_sync_error(&anyhow::anyhow!(
        "error sending request: connection refused"
    )));
}

#[test]
fn token_errors_are_detected_and_do_not_block_rows() {
    let unauthorized = http(401, "");
    let invalid = http(
        400,
        r#"{"errors":[{"message":"Invalid token","status":400}]}"#,
    );
    assert!(is_token_error(&unauthorized));
    assert!(is_token_error(&invalid));
    // A bad token must not block the queued rows: they go through after reconnect.
    assert!(!is_terminal_sync_error(&unauthorized));
    assert!(!is_terminal_sync_error(&invalid));
    assert!(!is_token_error(&http(404, "not found")));
    assert!(!is_token_error(&anyhow::anyhow!("connection reset")));
}

#[test]
fn http_error_display_keeps_the_status_and_body() {
    assert_eq!(http(404, "nope").to_string(), "AniList HTTP 404: nope");
}

#[tokio::test]
async fn an_unreadable_saved_login_is_reported_and_clears_once_fixed() {
    let state = anivault_core::engine::runtime::fresh_test_state().await;
    // Not something this Windows account encrypted (and plain text, so
    // secret scanners don't mistake it for a real token).
    state.storage.set_setting("anilist_access_token", "not-encrypted-on-this-account", 1).await.unwrap();

    let err = anivault_core::engine::sync_worker::drain_and_report(&state).await.unwrap_err();
    assert!(err.to_string().contains("saved login couldn't be read"));
    let problems = state.problems.current();
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].summary, "Your changes aren't reaching AniList.");

    state.storage.delete_setting("anilist_access_token").await.unwrap();
    anivault_core::engine::sync_worker::drain_and_report(&state).await.unwrap();
    assert!(state.problems.current().is_empty());
}

#[tokio::test]
async fn a_change_that_cant_be_queued_is_reported() {
    let state = anivault_core::engine::runtime::fresh_test_state().await;
    state.storage.set_setting("anilist_access_token", "\"stored\"", 1).await.unwrap();
    state.storage.upsert_anime_full(1, r#"{"romaji":"Show"}"#, 12, None, None, None, None, 1).await.unwrap();
    state.storage.upsert_list_entry_full(1, "watching", 1, None, "", 1, 1).await.unwrap();
    anivault_core::engine::storage::Tests::exec(&state.storage, "DROP TABLE sync_queue").await;

    anivault_core::engine::sync_worker::enqueue_anilist_sync(&state, 1).await;

    let problems = state.problems.current();
    assert_eq!(problems.len(), 1);
    assert!(problems[0].summary.contains("couldn't be queued for AniList"));
}
