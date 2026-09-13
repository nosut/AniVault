use anivault_core::engine::anilist::auth::{delete_token, load_token, store_token};
use anivault_core::engine::storage::Storage;

async fn new_storage() -> Storage {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage
}

#[tokio::test]
async fn token_roundtrip_encrypt_decrypt() {
    let storage = new_storage().await;
    store_token(&storage, "secret-token").await.unwrap();
    let loaded = load_token(&storage).await.unwrap();
    assert_eq!(loaded, Some("secret-token".to_string()));
}

#[tokio::test]
async fn no_token_returns_none() {
    let storage = new_storage().await;
    let loaded = load_token(&storage).await.unwrap();
    assert_eq!(loaded, None);
}

#[tokio::test]
async fn delete_token_removes_it() {
    let storage = new_storage().await;
    store_token(&storage, "secret-token").await.unwrap();
    delete_token(&storage).await.unwrap();
    let loaded = load_token(&storage).await.unwrap();
    assert_eq!(loaded, None);
}

#[tokio::test]
async fn invalid_token_flag_hides_the_token_until_a_new_one_is_stored() {
    use anivault_core::engine::anilist::auth::{is_token_invalid, mark_token_invalid, token_stored};
    let storage = new_storage().await;
    store_token(&storage, "old").await.unwrap();

    mark_token_invalid(&storage).await.unwrap();
    assert!(is_token_invalid(&storage).await.unwrap());
    assert_eq!(load_token(&storage).await.unwrap(), None);
    // The ciphertext is still there, so progress keeps being queued for later.
    assert!(token_stored(&storage).await.unwrap());

    store_token(&storage, "fresh").await.unwrap();
    assert!(!is_token_invalid(&storage).await.unwrap());
    assert_eq!(load_token(&storage).await.unwrap(), Some("fresh".to_string()));
}
