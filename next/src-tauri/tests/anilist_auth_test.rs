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
