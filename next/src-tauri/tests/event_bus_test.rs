use anivault_core::engine::event_bus::EventBus;
use anivault_core::engine::events::{EngineEvent, MediaDetected};

#[test]
fn event_bus_records_published_events_in_order() {
    let bus = EventBus::default();

    bus.publish(EngineEvent::MediaDetected(MediaDetected {
        player_name: "mpv".to_string(),
        file_path: Some("D:/Anime/Episode 01.mkv".to_string()),
        window_title: Some("Episode 01".to_string()),
        detected_at_unix: 1_782_769_008,
    }));

    bus.publish(EngineEvent::SyncFailed {
        service: "anilist".to_string(),
        anime_id: 42,
        message: "network offline".to_string(),
    });

    let events = bus.drain();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], EngineEvent::MediaDetected(_)));
    assert!(matches!(events[1], EngineEvent::SyncFailed { .. }));
    assert!(bus.drain().is_empty());
}

#[test]
fn event_bus_caps_buffered_events_and_keeps_the_newest() {
    let bus = EventBus::default();

    for i in 0..1100 {
        bus.publish(EngineEvent::SyncFailed {
            service: "anilist".to_string(),
            anime_id: i,
            message: "test".to_string(),
        });
    }

    let events = bus.drain();
    assert!(events.len() <= 1000, "expected the buffer to be capped, got {} events", events.len());

    // The newest events (highest anime_id) must be the ones kept, not the
    // oldest — a cap that drops from the wrong end would silently discard
    // exactly the events the frontend most needs.
    let last = events.last().unwrap();
    assert!(matches!(last, EngineEvent::SyncFailed { anime_id: 1099, .. }));
}
