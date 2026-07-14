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
