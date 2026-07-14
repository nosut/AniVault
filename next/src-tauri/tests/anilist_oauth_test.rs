use anivault_core::engine::anilist::oauth::generate_state;

#[test]
fn generate_state_produces_distinct_unpredictable_values() {
    let a = generate_state();
    let b = generate_state();
    assert_ne!(a, b, "two consecutive nonces must not collide");
    assert!(a.len() >= 16, "nonce should be long enough to resist guessing");
}
