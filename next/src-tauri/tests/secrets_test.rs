use anivault_core::engine::secrets::{protect_secret, unprotect_secret};

#[test]
fn dpapi_round_trips_secret() {
    let encrypted = protect_secret("round-trip-plaintext").unwrap();
    assert_ne!(encrypted, "round-trip-plaintext");

    let decrypted = unprotect_secret(&encrypted).unwrap();
    assert_eq!(decrypted, "round-trip-plaintext");
}
