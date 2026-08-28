# Window State Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore the main window's size, position, and maximized state across app restarts.

**Architecture:** Register the official `tauri-plugin-window-state` in `lib.rs` with a
flag set that deliberately excludes `VISIBLE`, because this app hides its window to the
tray on close instead of exiting. The plugin restores geometry at window creation —
before the existing manual `show()` in `setup()` — and persists it on app exit, with an
extra explicit save in the `CloseRequested` handler so geometry survives an unclean
shutdown.

**Tech Stack:** Rust, Tauri 2.4, `tauri-plugin-window-state` 2.x, cargo integration tests.

**Spec:** `docs/superpowers/specs/2026-08-04-window-state-persistence-design.md`

## Global Constraints

- Active codebase is `next/`; Rust backend is `next/src-tauri`. Do not touch any
  top-level legacy code.
- Windows-only application. `pwsh` is not on PATH — use plain `powershell` if a
  script must be run directly.
- The single-instance plugin MUST remain the first plugin registered in
  `tauri::Builder`. Insert new plugins after it.
- The library crate is named `anivault_core` (see `[lib]` in `Cargo.toml`).
  Integration tests in `next/src-tauri/tests/` reference it by that name.
- Do NOT build the installer, push, or create a release. Task 3 (version bump) is
  gated on explicit user confirmation.
- If `cargo test` fails with an internal compiler error, it is a stale incremental
  cache, not the diff. Re-run with `CARGO_INCREMENTAL=0`.

---

### Task 1: Add the plugin and the state-flag helper

**Files:**
- Modify: `next/src-tauri/Cargo.toml:29-31` (dependency list)
- Modify: `next/src-tauri/src/lib.rs:12` (imports), `:16-25` (add helper below
  `update_tray_pause_label`), `:39-40` (plugin registration)
- Test: `next/src-tauri/tests/window_state_test.rs` (create)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `anivault_core::window_state_flags() -> tauri_plugin_window_state::StateFlags`
  — used by Task 2.

**Why the helper exists:** clearing `VISIBLE` is the load-bearing detail of this
feature. Extracting it into a named, tested function means a later "simplification"
back to `StateFlags::all()` fails a test instead of silently shipping an app whose
window never appears.

- [ ] **Step 1: Add the dependency**

In `next/src-tauri/Cargo.toml`, add to `[dependencies]`, keeping the existing
alphabetical grouping of the `tauri-plugin-*` entries:

```toml
tauri-plugin-window-state = "2"
```

The result should read:

```toml
tauri-plugin-dialog = "2"
tauri-plugin-notification = "2"
tauri-plugin-single-instance = "2"
tauri-plugin-window-state = "2"
```

- [ ] **Step 2: Write the failing test**

Create `next/src-tauri/tests/window_state_test.rs`:

```rust
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
```

- [ ] **Step 3: Run the test to verify it fails**

Run from `next/src-tauri`:

```bash
cargo test --test window_state_test
```

Expected: FAIL to compile with `unresolved import anivault_core::window_state_flags`
(the function does not exist yet).

- [ ] **Step 4: Add the import**

In `next/src-tauri/src/lib.rs`, add below the existing `tauri_plugin_dialog` import
on line 12:

```rust
use tauri_plugin_window_state::{AppHandleExt, StateFlags};
```

`AppHandleExt` is unused until Task 2 and will warn; that is expected and resolves
there. If the warning is escalated to an error by the build, complete Task 2 before
re-running.

- [ ] **Step 5: Add the helper**

In `next/src-tauri/src/lib.rs`, insert after `update_tray_pause_label` (after line 25,
before `pub fn run()`):

```rust
/// Window-state flags: everything the plugin can persist except visibility.
///
/// `CloseRequested` hides the window to the tray rather than exiting, so
/// persisting `VISIBLE` would save `hidden` on every close and restore a window
/// that never appears on the next launch. Visibility stays under the manual
/// control in `setup()`.
pub fn window_state_flags() -> StateFlags {
    StateFlags::all() & !StateFlags::VISIBLE
}
```

- [ ] **Step 6: Run the test to verify it passes**

Run from `next/src-tauri`:

```bash
cargo test --test window_state_test
```

Expected: PASS, 2 tests.

- [ ] **Step 7: Register the plugin**

In `next/src-tauri/src/lib.rs`, add after the `tauri_plugin_notification::init()` line
(line 40) and before `.on_window_event(...)`:

```rust
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(window_state_flags())
                .build(),
        )
```

Do not move the single-instance plugin — it must stay first.

- [ ] **Step 8: Verify it compiles**

Run from `next/src-tauri`:

```bash
cargo check --tests
```

Expected: success. An `unused import: AppHandleExt` warning is expected here and is
resolved by Task 2.

- [ ] **Step 9: Commit**

```bash
git add next/src-tauri/Cargo.toml next/src-tauri/Cargo.lock next/src-tauri/src/lib.rs next/src-tauri/tests/window_state_test.rs
git commit -m "feat: restore window size and position across restarts"
```

---

### Task 2: Persist geometry when the window is closed to tray

**Files:**
- Modify: `next/src-tauri/src/lib.rs:41-46` (the `on_window_event` handler)

**Interfaces:**
- Consumes: `window_state_flags()` from Task 1.
- Produces: nothing consumed by later tasks.

**Why:** the plugin persists state on `RunEvent::Exit`, which fires for the tray
"Quit" path (`app.exit()`). But the ordinary close does not exit — it hides, and the
process may later be terminated without a clean shutdown, losing the geometry. Saving
at close time makes it durable immediately.

- [ ] **Step 1: Add the explicit save**

In `next/src-tauri/src/lib.rs`, replace the existing handler:

```rust
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
```

with:

```rust
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Save geometry before hiding: closing hides to tray instead of
                // exiting, so the plugin's save-on-exit may never run if the
                // process is killed rather than quit from the tray.
                let _ = window.app_handle().save_window_state(window_state_flags());
                let _ = window.hide();
                api.prevent_close();
            }
        })
```

The save must come before `window.hide()`.

- [ ] **Step 2: Verify it compiles with no warnings**

Run from `next/src-tauri`:

```bash
cargo check --tests
```

Expected: success, and the `unused import: AppHandleExt` warning from Task 1 is gone.

- [ ] **Step 3: Run the full verification suite**

Run from `next/`:

```bash
npm run verify
```

Expected: typecheck, vitest, and `cargo check --tests` all pass.

- [ ] **Step 4: Manual verification**

Launch the app (`npm run tauri dev` from `next/`, or the built binary) and confirm all
four cases from the spec:

1. Resize and move the window, close it, reopen — returns to that size and position.
2. Maximize, close, reopen — reopens maximized; un-maximizing returns to the last
   non-maximized size and position.
3. Close to tray, then reopen via the tray "Show AniVault" item — appears at the
   remembered geometry.
4. Launch with `--minimized`, then open from the tray — appears at the remembered
   geometry.

Report the result of each. Do not proceed to Task 3 if any case fails.

- [ ] **Step 5: Commit**

```bash
git add next/src-tauri/src/lib.rs
git commit -m "fix: save window geometry when closing to tray"
```

---

### Task 3: Version bump — GATED, requires user confirmation

**Do not start this task until the user has explicitly confirmed they want a build.**
Per `CLAUDE.md`, the user batches several changes before cutting a build and decides
when. Ask first; if they have not said go, stop after Task 2 and report completion.

**Files:**
- Modify: `next/package.json`, `next/package-lock.json` (via npm)
- Modify: `next/src-tauri/Cargo.toml:3`
- Modify: `next/src-tauri/tauri.conf.json:4`
- Modify: `next/src-tauri/Cargo.lock` (refreshes on next build)

**Interfaces:**
- Consumes: nothing.
- Produces: version `1.0.17` across all four files.

- [ ] **Step 1: Bump package.json and package-lock.json**

Run from `next/`:

```bash
npm version 1.0.17 --no-git-tag-version
```

- [ ] **Step 2: Bump the Rust manifest**

In `next/src-tauri/Cargo.toml`, change line 3:

```toml
version = "1.0.17"
```

- [ ] **Step 3: Bump the Tauri config**

In `next/src-tauri/tauri.conf.json`, change line 4:

```json
  "version": "1.0.17",
```

- [ ] **Step 4: Verify all four versions match**

Run from the repo root:

```bash
grep -n '"version"' next/package.json | head -1
grep -n '^version' next/src-tauri/Cargo.toml
grep -n '"version"' next/src-tauri/tauri.conf.json
```

Expected: `1.0.17` in all three (package-lock.json is updated by npm in Step 1).

- [ ] **Step 5: Build the installer**

Run from `next/`:

```bash
npm run bundle
```

Expected output at
`next/src-tauri/target/release/bundle/nsis/AniVault_1.0.17_x64-setup.exe`.

- [ ] **Step 6: Commit the release**

```bash
git add next/package.json next/package-lock.json next/src-tauri/Cargo.toml next/src-tauri/Cargo.lock next/src-tauri/tauri.conf.json
git commit -m "chore: release 1.0.17"
git tag v1.0.17
```

- [ ] **Step 7: Push and release**

Only after the user confirms. Push branch and tag, then:

```bash
gh release create v1.0.17 \
  "next/src-tauri/target/release/bundle/nsis/AniVault_1.0.17_x64-setup.exe" \
  --title "AniVault v1.0.17" \
  --notes "> ⚠️ AI-generated project. Windows-only. Install over any previous version.

### Added
- The app now remembers its window size, position, and maximized state, and restores them on the next launch."
```
