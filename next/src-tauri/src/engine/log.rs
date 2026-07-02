use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::EnvFilter;

/// Initialize tracing: file-based subscriber writing to the app data directory.
/// Falls back to stderr if the log file cannot be created.
pub fn init_logging(log_dir: &std::path::Path) {
    let file_appender = tracing_appender::rolling::daily(log_dir, "anivault.log");

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,anivault_core=debug")),
        )
        .with_span_events(FmtSpan::CLOSE)
        .with_writer(file_appender)
        .with_target(false);

    // Use try_init to avoid panic if called twice
    let _ = subscriber.try_init();
}
