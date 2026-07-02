#[test]
fn backoff_delay_increases() {
    use anivault_core::engine::sync_worker::backoff_delay;
    assert_eq!(backoff_delay(0), 1);
    assert_eq!(backoff_delay(1), 2);
    assert_eq!(backoff_delay(2), 4);
    assert_eq!(backoff_delay(3), 4); // capped
}
