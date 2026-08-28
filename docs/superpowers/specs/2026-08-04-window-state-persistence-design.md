# Window State Persistence — Design

**Date:** 2026-08-04
**Status:** Approved

## Problem

The main window opens at the fixed 1280x820 size from `tauri.conf.json` at whatever
position the window manager picks, every launch. Users who prefer a different size
or a specific monitor must resize and reposition the window on every start.

## Goal

Remember the main window's size, position, and maximized state across restarts, so
reopening the app restores the geometry it had when it was last closed.

## Approach

Use the official `tauri-plugin-window-state` rather than hand-rolling persistence on
the existing settings store. The plugin already solves the parts that are fiddly to
get right:

- debouncing the continuous stream of resize events Windows emits during a drag
- tracking pre-maximize bounds separately, so un-maximizing returns to the last
  explicit size rather than the maximized one
- clamping the window back onto a visible monitor when it was last positioned on a
  display that is no longer connected

A hand-rolled implementation on `get_setting`/`set_setting` would be roughly 80 lines
of Rust reimplementing the above. It would only be preferable if geometry needed to
travel with the app's other settings, which nothing here requires.

## Design

### Dependency and registration

Add to `next/src-tauri/Cargo.toml`:

```toml
tauri-plugin-window-state = "2"
```

Register in `next/src-tauri/src/lib.rs`, after the single-instance plugin (which must
remain the first plugin registered) and alongside the dialog and notification plugins:

```rust
.plugin(
    tauri_plugin_window_state::Builder::default()
        .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE)
        .build(),
)
```

`StateFlags::all()` covers size, position, maximized, and fullscreen.

**Clearing `VISIBLE` is load-bearing.** This app hides the window on close instead of
exiting (`CloseRequested` calls `window.hide()` and `api.prevent_close()`). With the
default flags the plugin would persist `visible: false` on every close-to-tray, and
the next launch would restore a hidden window that never appears — the app would look
like it failed to start. Visibility stays under the existing manual control in
`setup()`.

### Restore path

The plugin restores geometry at window creation, which happens before `setup()` runs.
The existing `window.show()` in `setup()` therefore shows a window that is already at
the remembered geometry. Because the window is configured `visible: false` in
`tauri.conf.json`, there is no visible flash of a default-sized window snapping into
place.

The `minWidth` (960) and `minHeight` (640) constraints in `tauri.conf.json` continue to
apply to the restored size, so a corrupt or absurd saved value cannot produce an
unusable window.

The `--minimized` start-in-tray path needs no changes. Geometry is restored at window
creation as usual; the window simply stays hidden until the user opens it from the
tray, at which point it appears at the remembered geometry.

### Save path

The plugin writes `.window-state.json` to the app config directory on application
exit. That covers the tray "Quit" path, which calls `app.exit()`.

However, the app's ordinary close does not exit the process — it hides to tray, and
the process may later be terminated without a clean shutdown. To make the geometry
durable at the moment the user closes the window, add an explicit save to the existing
`CloseRequested` handler, before `window.hide()`:

```rust
let _ = window
    .app_handle()
    .save_window_state(StateFlags::all() & !StateFlags::VISIBLE);
```

This requires importing `AppHandleExt` and `StateFlags` from
`tauri_plugin_window_state` in `lib.rs`.

## Out of scope

- **Frontend changes.** The plugin is driven entirely from Rust.
- **Capability entries.** The plugin's JS commands are never invoked from the
  frontend, so `next/src-tauri/capabilities/default.json` is unchanged.
- **A settings toggle** for the behavior. Restoring window geometry is the standard
  desktop expectation, not a preference worth surfacing.

## Verification

`npm run verify` in `next/` covers compilation (typecheck, vitest, `cargo check
--tests`).

The behavior itself depends on window-manager state and is not unit-testable. It is
verified manually:

1. Resize and move the window, close it, reopen — window returns to that size and
   position.
2. Maximize, close, reopen — window reopens maximized; un-maximizing returns to the
   last non-maximized size and position.
3. Close to tray, then reopen from the tray "Show AniVault" item — window appears at
   the remembered geometry.
4. Launch with `--minimized`, then open from the tray — window appears at the
   remembered geometry.

## Release

This is a user-facing change, so it ships with a patch version bump to 1.0.17 across
`next/package.json`, `next/package-lock.json`, `next/src-tauri/Cargo.toml`, and
`next/src-tauri/tauri.conf.json`. No build, push, or release happens without explicit
user confirmation.
