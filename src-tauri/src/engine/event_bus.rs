use std::sync::{Arc, Mutex};

use crate::engine::events::EngineEvent;

#[derive(Debug, Clone, Default)]
pub struct EventBus {
    events: Arc<Mutex<Vec<EngineEvent>>>,
}

impl EventBus {
    pub fn publish(&self, event: EngineEvent) {
        self.events.lock().expect("event bus poisoned").push(event);
    }

    pub fn drain(&self) -> Vec<EngineEvent> {
        let mut events = self.events.lock().expect("event bus poisoned");
        std::mem::take(&mut *events)
    }
}
