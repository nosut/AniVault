use crate::engine::migration::MigrationReport;

#[derive(Debug, Clone, serde::Serialize)]
pub struct EngineStatus {
    pub ok: bool,
    pub database: &'static str,
}

#[tauri::command]
pub fn get_engine_status() -> EngineStatus {
    EngineStatus {
        ok: true,
        database: "uninitialized",
    }
}

#[tauri::command]
pub fn preview_migration_report() -> MigrationReport {
    MigrationReport::default()
}
