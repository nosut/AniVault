use anivault_core::engine::oauth::{generate_code_verifier, code_challenge_from_verifier, OAuthState};

#[test]
fn pkce_verifier_is_43_to_128_url_safe_chars() {
    let verifier = generate_code_verifier();
    assert!(verifier.len() >= 43);
    assert!(verifier.len() <= 128);
    assert!(verifier.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~'));
}

#[test]
fn pkce_challenge_is_base64url_no_padding() {
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let challenge = code_challenge_from_verifier(verifier);
    assert!(!challenge.contains('='), "challenge must not have base64 padding");
    assert!(challenge.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
}

#[test]
fn pkce_challenge_is_deterministic() {
    let verifier = "test-verifier-43-chars-long-minimum-okok";
    let ch1 = code_challenge_from_verifier(verifier);
    let ch2 = code_challenge_from_verifier(verifier);
    assert_eq!(ch1, ch2);
    assert!(!ch1.is_empty());
}

#[test]
fn oauth_state_holds_pkce_and_redirect() {
    let state = OAuthState::new("http://localhost:1420/callback".into());
    assert_eq!(state.redirect_uri, "http://localhost:1420/callback");
    assert!(state.code_verifier.len() >= 43);
    assert!(!state.code_challenge.is_empty());
}

#[test]
fn oauth_state_stored_and_restored_via_settings() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let storage = anivault_core::engine::storage::Storage::connect("sqlite::memory:").await.unwrap();
        storage.migrate().await.unwrap();

        let state = OAuthState::new("http://localhost:1420/callback".into());
        anivault_core::engine::oauth::store_oauth_state(&storage, &state).await.unwrap();

        let restored = anivault_core::engine::oauth::load_oauth_state(&storage).await.unwrap().unwrap();
        assert_eq!(restored.code_verifier, state.code_verifier);
        assert_eq!(restored.redirect_uri, state.redirect_uri);
    });
}
