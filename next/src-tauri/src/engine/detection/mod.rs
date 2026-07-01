pub mod fs;
pub mod process;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::engine::event_bus::EventBus;
use crate::engine::events::{EngineEvent, MediaDetected};
use crate::engine::models::DetectionConfig;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DetectionKey {
    FilePath(String),
    WindowTitle(String),
}

#[derive(Debug, Clone)]
pub struct DetectionDeduper {
    window_secs: i64,
    seen: HashMap<DetectionKey, i64>,
}

impl DetectionDeduper {
    pub fn new(window_secs: i64) -> Self {
        Self {
            window_secs,
            seen: HashMap::new(),
        }
    }

    pub fn should_emit(&mut self, key: &DetectionKey, now_unix: i64) -> bool {
        let should_emit = self
            .seen
            .get(key)
            .is_none_or(|last| now_unix.saturating_sub(*last) > self.window_secs);
        if should_emit {
            self.seen.insert(key.clone(), now_unix);
        }
        should_emit
    }
}

pub struct DetectionManager {
    running: Arc<AtomicBool>,
}

impl DetectionManager {
    pub fn start(bus: EventBus, config: DetectionConfig) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let deduper = Arc::new(Mutex::new(DetectionDeduper::new(60)));

        start_process_loop(&bus, &config, &running, &deduper);
        start_file_loop(&bus, &config, &running, &deduper);

        Self { running }
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

fn start_process_loop(
    bus: &EventBus,
    config: &DetectionConfig,
    running: &Arc<AtomicBool>,
    deduper: &Arc<Mutex<DetectionDeduper>>,
) {
    let bus = bus.clone();
    let running = running.clone();
    let deduper = deduper.clone();
    let poll_interval = Duration::from_millis(config.poll_interval_ms);

    tokio::spawn(async move {
        while running.load(Ordering::Relaxed) {
            for player in process::scan_players() {
                if let Some(title) = player.window_title.clone() {
                    emit_if_new(
                        &bus,
                        &deduper,
                        DetectionKey::WindowTitle(title.clone()),
                        MediaDetected {
                            player_name: player.process_name,
                            file_path: None,
                            window_title: Some(title),
                            detected_at_unix: now_unix(),
                        },
                    );
                }
            }
            tokio::time::sleep(poll_interval).await;
        }
    });
}

fn start_file_loop(
    bus: &EventBus,
    config: &DetectionConfig,
    running: &Arc<AtomicBool>,
    deduper: &Arc<Mutex<DetectionDeduper>>,
) {
    let bus = bus.clone();
    let running = running.clone();
    let deduper = deduper.clone();
    let folders = config.folders.clone();
    let poll_interval = Duration::from_millis(config.poll_interval_ms);

    tokio::spawn(async move {
        let mut known: HashMap<PathBuf, SystemTime> = HashMap::new();
        while running.load(Ordering::Relaxed) {
            for candidate in fs::scan_media_files(&folders, &mut known) {
                emit_if_new(
                    &bus,
                    &deduper,
                    DetectionKey::FilePath(candidate.path.clone()),
                    MediaDetected {
                        player_name: "file_watcher".to_string(),
                        file_path: Some(candidate.path),
                        window_title: None,
                        detected_at_unix: candidate.modified_at_unix,
                    },
                );
            }
            tokio::time::sleep(poll_interval).await;
        }
    });
}

fn emit_if_new(
    bus: &EventBus,
    deduper: &Arc<Mutex<DetectionDeduper>>,
    key: DetectionKey,
    detected: MediaDetected,
) {
    if deduper
        .lock()
        .expect("detection deduper poisoned")
        .should_emit(&key, detected.detected_at_unix)
    {
        bus.publish(EngineEvent::MediaDetected(detected));
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
