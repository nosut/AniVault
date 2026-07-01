pub mod anilist;
pub mod detection;
pub mod event_bus;
pub mod events;
pub mod migration;
pub mod models;
pub mod oauth;
pub mod orchestrator;
pub mod pending;
pub mod rate_limit;
pub mod recognition;
pub mod secrets;
pub mod settings;
pub mod sonarr;
pub mod storage;
pub mod sync;

pub fn database_url() -> anyhow::Result<String> {
    let app_data = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
    let directory = app_data.join("AniVault");
    std::fs::create_dir_all(&directory)?;
    let normalized = directory.join("anivault.db").to_string_lossy().replace('\\', "/");
    Ok(format!("sqlite:///{}", normalized))
}
