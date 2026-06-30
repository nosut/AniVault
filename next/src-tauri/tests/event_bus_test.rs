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
