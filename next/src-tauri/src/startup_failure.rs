//! What the user sees when AniVault cannot start. Release builds have no
//! console and the window is created hidden, so without this a failed
//! database open would end the process with no sign anything happened.

use std::path::PathBuf;
use std::sync::OnceLock;

/// Where this run logs, once setup has resolved it, so the message can point
/// the user at the technical details.
pub static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// The plain explanation for a startup failure, with the raw error last.
pub fn message(raw: &str, log_dir: Option<&std::path::Path>) -> String {
    let lower = raw.to_ascii_lowercase();
    let advice = if lower.contains("database is locked") || lower.contains("database is busy") {
        "Another program may be using it. Close any other copy of AniVault (check the system tray), then start AniVault again."
    } else if lower.contains("disk is full") || lower.contains("not enough space") || lower.contains("os error 112") {
        "The drive is full. Free up some space, then start AniVault again."
    } else if lower.contains("readonly") || lower.contains("read-only") || lower.contains("access is denied") || lower.contains("os error 5)") {
        "Windows didn't allow AniVault to open it. Check that the AniVault folder in AppData isn't read-only or blocked by security software."
    } else if lower.contains("migration") {
        "Its data was saved by a different version of AniVault. Install the latest version, or restore a backup."
    } else if lower.contains("unable to open database file") {
        "The database file couldn't be opened. Check that the AniVault folder in AppData still exists and can be written to."
    } else {
        "Start AniVault again. If this keeps happening, the details below explain what went wrong."
    };
    let database = ["database", "sqlite", "migration", "disk is full"].iter().any(|w| lower.contains(w));
    let headline = if database {
        "AniVault couldn't start because it couldn't open its database."
    } else {
        "AniVault couldn't start."
    };
    let mut text = format!("{headline}\n\n{advice}");
    if let Some(dir) = log_dir {
        text.push_str(&format!("\n\nThe log is in:\n{}", dir.display()));
    }
    text.push_str(&format!("\n\nDetails: {raw}"));
    text
}

/// Set once setup has finished; the panic hook only speaks up before that.
static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The result of a startup step, or the message box and exit. Setup errors
/// must be handled here: Tauri turns an `Err` returned from `setup` into a
/// panic ("Failed to setup app"), never into an error `run` returns, so
/// nothing after `run` would see it.
pub fn require<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(e) => {
            report(&e.to_string());
            std::process::exit(1);
        }
    }
}

/// Setup finished: from here on a panic is not a failure to start.
pub fn mark_started() {
    STARTED.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Backstop for anything else that panics before setup finishes. Release
/// builds have no console, so the panic text alone reaches no one. After
/// startup it only chains to the default hook; workers have their own
/// supervisor.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if !STARTED.load(std::sync::atomic::Ordering::SeqCst) {
            let text = match info.payload().downcast_ref::<String>() {
                Some(s) => s.clone(),
                None => info.payload().downcast_ref::<&str>().map(|s| s.to_string()).unwrap_or_default(),
            };
            report(&text);
        }
        previous(info);
    }));
}

/// Log the failure and show it in a native message box. Uses the Win32 API
/// directly because the Tauri event loop (and so its dialog plugin) never
/// started.
pub fn report(raw: &str) {
    tracing::error!("AniVault failed to start: {raw}");
    let text = message(raw, LOG_DIR.get().map(|p| p.as_path()));
    show_box(&text);
}

#[cfg(windows)]
fn show_box(text: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let body = wide(text);
    let title = wide("AniVault couldn't start");
    unsafe {
        MessageBoxW(None, PCWSTR(body.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONERROR);
    }
}

#[cfg(not(windows))]
fn show_box(text: &str) {
    eprintln!("{text}");
}

#[cfg(test)]
mod tests {
    use super::message;

    #[test]
    fn explains_common_causes_in_plain_words_and_keeps_the_raw_error() {
        let locked = message("error returned from database: (code: 5) database is locked", None);
        assert!(locked.starts_with("AniVault couldn't start because it couldn't open its database."));
        assert!(locked.contains("Close any other copy of AniVault"));
        assert!(locked.ends_with("Details: error returned from database: (code: 5) database is locked"));

        assert!(message("(code: 13) database or disk is full", None).contains("The drive is full"));
        assert!(message("(code: 8) attempt to write a readonly database", None).contains("didn't allow"));
        assert!(message("migration 20260101 was previously applied but has been modified", None).contains("different version"));
        assert!(message("something else", None).contains("Start AniVault again"));
    }

    #[test]
    fn a_failure_unrelated_to_the_database_does_not_blame_it() {
        let text = message("failed to create webview: something", None);
        assert!(text.starts_with("AniVault couldn't start.\n\n"));
    }

    #[test]
    fn points_at_the_log() {
        let text = message("x", Some(std::path::Path::new(r"C:\logs")));
        assert!(text.contains("The log is in:\nC:\\logs"));
    }
}
