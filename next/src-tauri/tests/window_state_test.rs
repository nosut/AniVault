use anivault_core::window_state_flags;
use tauri_plugin_window_state::StateFlags;

/// Closing the window hides it to the tray. If visibility were persisted, every
/// close would save "hidden" and the next launch would restore a window that
/// never appears.
#[test]
fn window_state_flags_exclude_visibility() {
    assert!(!window_state_flags().contains(StateFlags::VISIBLE));
}

#[test]
fn window_state_flags_cover_geometry_and_maximized() {
    let flags = window_state_flags();
    assert!(flags.contains(StateFlags::SIZE));
    assert!(flags.contains(StateFlags::POSITION));
    assert!(flags.contains(StateFlags::MAXIMIZED));
}
