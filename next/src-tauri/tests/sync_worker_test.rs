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
