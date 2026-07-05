use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

/// Initialize tracing.
///
/// Writes a daily-rolling log file to `log_dir` (the file is named
/// `anivault.log.YYYY-MM-DD` — `tracing_appender` appends the date). In debug
/// builds it also mirrors logs to stdout so `cargo tauri dev` shows them live.
///
/// The default filter is `info` globally plus `debug` for the app crate. Override
/// with the `RUST_LOG` environment variable (e.g. `RUST_LOG=anivault_core=trace`).
pub fn init_logging(log_dir: &std::path::Path) {
    if let Err(e) = std::fs::create_dir_all(log_dir) {
        eprintln!(
            "[AniVault] could not create log dir {}: {e}",
            log_dir.display()
        );
    }

    let file_appender = tracing_appender::rolling::daily(log_dir, "anivault.log");
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,anivault_core=debug"));

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_appender)
        .with_target(false)
        .with_ansi(false);

    // Console layer only in debug builds (release uses windows_subsystem = "windows",
    // which has no attached console, so stdout would go nowhere).
    let stdout_layer = if cfg!(debug_assertions) {
        Some(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_writer(std::io::stdout),
        )
    } else {
        None
    };

    match tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stdout_layer)
        .try_init()
    {
        Ok(_) => {
            // Emitted through the freshly-installed subscriber (goes to the file) AND
            // to stderr so the resolved path is discoverable even before logs accumulate.
            tracing::info!("File logging active in {}", log_dir.display());
            eprintln!(
                "[AniVault] logging to {} (file: anivault.log.<date>)",
                log_dir.display()
            );
        }
        Err(e) => {
            eprintln!(
                "[AniVault] tracing already initialized — file logging disabled ({e}). Intended location: {}",
                log_dir.display()
            );
        }
    }
}
