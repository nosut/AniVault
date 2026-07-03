use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

/// Initialize tracing: file-based subscriber writing to the app data directory.
pub fn init_logging(log_dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(log_dir);
    let file_appender = tracing_appender::rolling::daily(log_dir, "anivault.log");
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,anivault_core=debug"));

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_appender)
        .with_target(false)
        .with_ansi(false);

    match tracing_subscriber::registry().with(filter).with(file_layer).try_init() {
        Ok(_) => tracing::info!("Logging to: {}", log_dir.display()),
        Err(_) => {
            eprintln!("[AniVault] tracing already set — file logging disabled. Logs location: {}", log_dir.display());
        }
    }
}
