use anivault_core::engine::storage::Storage;

async fn new_storage() -> Storage {
    let storage = Storage::connect("sqlite::memory:").await.unwrap();
    storage.migrate().await.unwrap();
    storage
}

fn sorted(mut ids: Vec<i64>) -> Vec<i64> {
    ids.sort_unstable();
    ids
}

#[tokio::test]
async fn an_unseen_season_has_no_ids() {
    let storage = new_storage().await;
    let ids = storage.season_seen_ids("FALL", 2026).await.unwrap();
    assert!(ids.is_empty());
}

#[tokio::test]
async fn recorded_ids_come_back() {
    let storage = new_storage().await;
    storage
        .record_season_seen("FALL", 2026, &[3, 1, 2], 1000)
        .await
        .unwrap();
    let ids = storage.season_seen_ids("FALL", 2026).await.unwrap();
    assert_eq!(sorted(ids), vec![1, 2, 3]);
}

#[tokio::test]
async fn seasons_are_keyed_independently() {
    let storage = new_storage().await;
    storage.record_season_seen("FALL", 2026, &[1], 1000).await.unwrap();
    storage.record_season_seen("FALL", 2027, &[2], 1000).await.unwrap();
    storage.record_season_seen("SUMMER", 2026, &[3], 1000).await.unwrap();

    assert_eq!(storage.season_seen_ids("FALL", 2026).await.unwrap(), vec![1]);
    assert_eq!(storage.season_seen_ids("FALL", 2027).await.unwrap(), vec![2]);
    assert_eq!(storage.season_seen_ids("SUMMER", 2026).await.unwrap(), vec![3]);
}

#[tokio::test]
async fn the_future_sentinel_is_just_another_key() {
    let storage = new_storage().await;
    storage
        .record_season_seen("__FUTURE__", 0, &[7], 1000)
        .await
        .unwrap();
    assert_eq!(
        storage.season_seen_ids("__FUTURE__", 0).await.unwrap(),
        vec![7]
    );
    assert!(storage.season_seen_ids("FALL", 2026).await.unwrap().is_empty());
}

#[tokio::test]
async fn re_recording_keeps_the_original_first_seen_at() {
    // first_seen_at is the "when did this show up" record. Re-recording an id on
    // every visit must not keep pushing it forward, or it stops meaning anything.
    let storage = new_storage().await;
    storage.record_season_seen("FALL", 2026, &[1], 1000).await.unwrap();
    storage.record_season_seen("FALL", 2026, &[1, 2], 5000).await.unwrap();

    let first = storage.season_first_seen_at("FALL", 2026, 1).await.unwrap();
    let second = storage.season_first_seen_at("FALL", 2026, 2).await.unwrap();
    assert_eq!(first, Some(1000), "an existing row is left alone");
    assert_eq!(second, Some(5000), "a new row gets the current time");
}

#[tokio::test]
async fn recording_nothing_is_a_no_op() {
    let storage = new_storage().await;
    storage.record_season_seen("FALL", 2026, &[], 1000).await.unwrap();
    assert!(storage.season_seen_ids("FALL", 2026).await.unwrap().is_empty());
}
