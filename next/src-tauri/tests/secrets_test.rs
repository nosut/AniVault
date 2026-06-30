use anivault_core::engine::secrets::{protect_secret, unprotect_secret};

#[test]
fn dpapi_round_trips_secret() {
    let encrypted = protect_secret("sonarr-api-key-123").unwrap();
    assert_ne!(encrypted, "sonarr-api-key-123");

    let decrypted = unprotect_secret(&encrypted).unwrap();
    assert_eq!(decrypted, "sonarr-api-key-123");
}
