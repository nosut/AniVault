use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

/// Initialize tracing: file-based subscriber writing to the app data directory.
/// Attempts a registry-based subscriber first; falls back to fmt().init() which
/// panics on duplicate subscriber so we surface the failure loudly.
pub fn init_logging(log_dir: &std::path::Path) {
    // Ensure directory exists
    let _ = std::fs::create_dir_all(log_dir);

    let file_appender = tracing_appender::rolling::daily(log_dir, "anivault.log");

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,anivault_core=debug"));

    // Use a Layer approach — add both filter and file layer on a registry.
    // try_init won't panic if a subscriber is already registered.
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_appender)
        .with_target(false)
        .with_ansi(false);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .try_init();

    // Also log a test message to confirm it works
    tracing::info!("Logging initialized. Logs at: {}", log_dir.display());
}
