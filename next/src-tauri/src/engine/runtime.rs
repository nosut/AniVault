use std::path::{Path, PathBuf};

use crate::engine::event_bus::EventBus;
use crate::engine::storage::Storage;

#[derive(Clone)]
pub struct EngineState {
    pub storage: Storage,
    pub events: EventBus,
    pub database_path: PathBuf,
}

pub fn sqlite_url_for_path(path: &Path) -> String {
    format!("sqlite:///{}", path.to_string_lossy().replace('\\', "/"))
}

pub async fn initialize_engine_at(database_path: PathBuf) -> anyhow::Result<EngineState> {
    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let database_url = sqlite_url_for_path(&database_path);
    let storage = Storage::connect(&database_url).await?;
    storage.migrate().await?;

    Ok(EngineState {
        storage,
        events: EventBus::default(),
        database_path,
    })
}
