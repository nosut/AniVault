# M5 Tray and Background Behavior — Design

## Purpose

Make AniVault behave like a desktop tracker: lives in tray, tracks in background, doesn't quit on window close, notifies on progress, and respects user pause.

## Architecture

All tray/background logic lives in Rust. Frontend changes are minimal — a Settings toggle for launch-on-startup only. Session pause is tray-controlled, not surfaced in UI.

## Rust State Changes

New field on `EngineState`:

```rust
pub tracking_paused: std::sync::atomic::AtomicBool,
```

Initialized to `false`. Session-only — resets on app restart. Existing tracker checks this before each scan tick:

```rust
if self.state.tracking_paused.load(Ordering::Relaxed) {
    return;
}
```

## Tray Menu

Built via `tauri::tray::TrayIconBuilder` in `lib.rs` `.setup()`.

```
AniVault
├── Show AniVault          (only visible when window hidden)
├── ──────────
├── Pause Tracking         (label toggles: Pause / Resume)
├── ──────────
├── Quit
```

- Icon: existing `Icon.png` in project root
- "Show" restores hidden window and focuses it
- "Pause/Resume" flips `tracking_paused` AtomicBool, updates menu item label
- "Quit" triggers confirmation dialog; on confirm: stop tracker, close DB, `app.exit(0)`
- Tray double-click: same as "Show"

## Window Lifecycle

**Close (X button):** Intercept `WindowEvent::CloseRequested` via `.on_window_event()`. Hide window instead of destroying. App and tracking continue.

**Show:** On tray "Show" or double-click: `window.show().focus()`.

**Quit:** On tray "Quit" confirmed: `app.exit(0)`. No window close prompt needed.

## Session Pause

- Tray menu item starts as "Pause Tracking"
- On click: sets `tracking_paused = true`, updates label to "Resume Tracking"
- On click again: sets `tracking_paused = false`, updates label to "Pause Tracking"
- Separate from `tracking.enabled` setting. Pause is temporary, reset on app restart.
- Notification check: don't notify if paused

## New Tauri Commands

| Command | Args | Returns | Purpose |
|---------|------|---------|---------|
| `get_session_state` | none | `{ tracking_paused: bool }` | Current session pause state |
| `toggle_pause_tracking` | none | `{ tracking_paused: bool }` | Toggle pause, return new state |
| `get_launch_on_startup` | none | `bool` | Read registry key |
| `set_launch_on_startup` | `enabled: bool` | `void` | Write registry + setting |

## Launch on Startup

- UI toggle in Settings → new "General" tab (separate from existing Tracking/AniList/About tabs)
- Reads/writes setting `startup.launch_on_startup` via existing `getSetting`/`setSetting`
- On enable: write registry key `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\AniVault` pointing to `std::env::current_exe()`
- On disable: delete registry key
- Registry write happens in the `set_launch_on_startup` command handler (Rust side)
- Settings-only — no tray menu item for this

## Notifications

Windows native notifications via `tauri-plugin-notification`.

Trigger: when `EngineEvent::ProgressAdvanced` fires (episode completed).

- Title: anime title extracted from `titles_json`
- Body: "Episode {new_episode} watched"
- Gate: skip if `tracking_paused` is true
- Click: no action (informational only)

Implementation: add notification call inside existing event publishing path, after successful DB write.

## Frontend Changes

- **SettingsView.svelte**: Add "General" tab. Contains launch-on-startup toggle (uses `getSetting`/`setSetting` for `startup.launch_on_startup`). Keep existing "Tracking", "AniList", "About" tabs.
- No other frontend changes for M5.

## Files Changed

| File | Action | Notes |
|------|--------|-------|
| `next/src-tauri/src/lib.rs` | Modify | Tray setup, window lifecycle, command registration |
| `next/src-tauri/src/engine/runtime.rs` | Modify | Add `tracking_paused` to EngineState, pass to tracker |
| `next/src-tauri/src/engine/tracker.rs` | Modify | Check pause flag in scan loop |
| `next/src-tauri/src/commands.rs` | Modify | 4 new commands + wrappers |
| `next/src-tauri/Cargo.toml` | Modify | Add `tauri-plugin-notification` |
| `next/src/lib/SettingsView.svelte` | Modify | Add General tab with startup toggle |
| `next/src/lib/api.ts` | Modify | Add types + wrappers for new commands |
| `next/src/lib/api.test.ts` | Modify | Add tests for new wrappers |
| `next/src-tauri/tests/tray_commands_test.rs` | Create | Integration tests for new commands |

## Out of Scope

- Single-instance enforcement
- Notification click handling (open detail view)
- Pause state surfaced in UI beyond tray menu
- Custom notification sounds or icons
- Auto-pause on fullscreen detection

## Dependencies

- M1 (tracking lifecycle) — completed
- M4 (settings UI for startup toggle) — completed
- `tauri-plugin-notification` — external crate, needs `Cargo.toml` addition

## Acceptance Criteria

1. Close button hides window but app keeps running (check Task Manager)
2. Tray icon visible, menu items work (Show, Pause/Resume, Quit)
3. Pause stops tracking; resume restarts it
4. Quit shows confirmation, fully exits app
5. Progress detects fire native Windows notification
6. Launch-on-startup toggle writes/removes registry key
7. App auto-starts on login when enabled
8. All tests pass (Rust + TypeScript)
