use std::sync::{Arc, Mutex};

use crate::engine::events::EngineEvent;

/// Hard cap on buffered-but-undrained events. Prevents unbounded memory
/// growth if the frontend stops polling (minimized/backgrounded window,
/// frontend crash) while the tracking loop keeps publishing.
const MAX_BUFFERED_EVENTS: usize = 1000;

#[derive(Debug, Clone, Default)]
pub struct EventBus {
    events: Arc<Mutex<Vec<EngineEvent>>>,
}

impl EventBus {
    pub fn publish(&self, event: EngineEvent) {
        let mut events = self.events.lock().expect("event bus poisoned");
        // The tracker repeats these every tick and the UI only reads the
        // latest, so each replaces the last one waiting. Otherwise a window
        // left hidden fills the buffer with them and the cap evicts the events
        // that matter (sync failures, status changes, playback ended).
        if is_repeating(&event) {
            let kind = std::mem::discriminant(&event);
            events.retain(|e| std::mem::discriminant(e) != kind);
        }
        events.push(event);
        if events.len() > MAX_BUFFERED_EVENTS {
            let excess = events.len() - MAX_BUFFERED_EVENTS;
            events.drain(0..excess);
        }
    }

    pub fn drain(&self) -> Vec<EngineEvent> {
        let mut events = self.events.lock().expect("event bus poisoned");
        std::mem::take(&mut *events)
    }
}

fn is_repeating(event: &EngineEvent) -> bool {
    matches!(event, EngineEvent::PlaybackDetected { .. } | EngineEvent::MediaDetected(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playback(n: i64) -> EngineEvent {
        EngineEvent::PlaybackDetected {
            player_name: "mpv".into(),
            file_path: None,
            window_title: None,
            episode_guess: None,
            candidates: vec![],
            detected_at_unix: n,
        }
    }

    #[test]
    fn repeating_events_never_crowd_out_the_ones_that_matter() {
        let bus = EventBus::default();
        bus.publish(EngineEvent::SyncFailed { service: "anilist".into(), anime_id: 1, message: "x".into() });
        for n in 0..5000 {
            bus.publish(playback(n));
        }
        let events = bus.drain();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], EngineEvent::SyncFailed { .. }));
        assert!(matches!(events[1], EngineEvent::PlaybackDetected { detected_at_unix: 4999, .. }));
    }
}
