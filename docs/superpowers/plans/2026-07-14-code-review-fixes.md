# Code Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the security, correctness, and performance findings from the full-codebase review (Rust/Tauri backend + Svelte frontend of AniVault), and roll out the same state-persistence fix already applied to `SearchView.svelte` to the other views that lose state on back-navigation.

**Architecture:** No architectural changes. Each task is a targeted fix in an existing file, following the file's existing conventions (e.g. the `loadPref`/`persistPref` localStorage pattern already used in `LibraryView.svelte`, the `bind:`-lifted-to-`App.svelte` pattern already used for `SearchView.svelte`, the `Tests::new_in_memory()` pattern already used in `next/src-tauri/tests/*.rs`).

**Tech Stack:** Rust (Tauri 2, sqlx/SQLite, reqwest, tokio), Svelte 5 + TypeScript, vitest, cargo test.

## Global Constraints

- Never run `git push`, `cargo publish`, or any destructive git command — this plan only touches the working tree.
- Rust changes: after each task, run `cargo build` (or `cargo test <specific test>` per step) from `next/src-tauri`. Full `cargo test` may be run at the end of each task.
- Svelte/TS changes: after each task, run `npm run check` (tsc) from `next`. Component-level behavior for `.svelte` files has no automated test harness in this repo (no `@testing-library/svelte`), so those steps are verified by `npm run check` plus a manual read-through, not by an automated test — this is called out explicitly in each such step rather than faked.
- Commit after each task with a `fix:`/`perf:`/`test:` message as appropriate. Do not bundle unrelated tasks into one commit.
- **Explicitly out of scope for this plan** (pure refactors with real regression risk and no corresponding safety net, deferred rather than silently dropped — flag to the user if they want these picked up separately):
  - Extracting a shared `PosterCard.svelte` component (`LibraryView`/`SeasonView`/`DashboardView` markup duplication).
  - Deduplicating the four `UPDATE file_index SET anime_id = NULL, confidence = 0 ...` call sites in `storage.rs` (each has slightly different accompanying logic — `ignored=1` on two of them, transactional batching on two of them).
  - Splitting up `get_calendar_inner` / `deep_match_via_anilist_inner` in `commands.rs`.
  - A shared error-normalization layer / `ApiError` type across `api.ts`.
  - Deduplicating HTTP-error-handling boilerplate inside `anilist/client.rs` / `sonarr/client.rs`.
  - Runtime validation of `getSetting<T>`'s stored JSON shape.
  - Full test coverage of all ~30 untested `api.ts` exports (Task 18 adds a representative sample instead).

---

### Task 1: Fix command injection in `open_file` (library_scanner.rs)

**Files:**
- Modify: `next/src-tauri/src/engine/library_scanner.rs:499-505`

**Interfaces:**
- Consumes: the `open` crate (already a dependency, `open = "5"` in `next/src-tauri/Cargo.toml:25`), already used identically in `next/src-tauri/src/engine/anilist/oauth.rs:65` (`open::that(url)`).
- Produces: `pub fn open_file(path: &str) -> anyhow::Result<()>` — signature unchanged, so `commands.rs:806-808` (`open_episode_file`) needs no changes.

**Root cause:** `open_file` spawns `cmd /c start "" <path>`. Even though Rust's `Command::args` quotes each argv element for `CreateProcess`, `cmd.exe` re-parses the whole command line itself and treats `&`, `|`, `^` as its own operators regardless of that quoting. A file named e.g. `Show - 01 & calc.exe &.mkv`, once scanned into the library, runs an injected command when the user clicks "Play".

- [ ] **Step 1: Replace the cmd.exe shell-out with `open::that`**

```rust
/// Open a file with the default system application (plays video files).
pub fn open_file(path: &str) -> anyhow::Result<()> {
    open::that(path)?;
    Ok(())
}
```

- [ ] **Step 2: Verify it builds**

Run: `cd next/src-tauri && cargo build`
Expected: builds with no errors or warnings about unused imports (no imports need removing — `std::process::Command` is still used by `open_containing_folder` in the same file).

There is no existing automated test for `open_file` (it launches a real OS process/handler, which isn't something to invoke in CI) — this is a **pre-existing** coverage gap, not one introduced here. Verification is by code inspection: `open::that` calls `ShellExecuteW` on Windows directly with the path as a single parameter, never invoking `cmd.exe`'s command-line parser, so shell metacharacters in the path can no longer be reinterpreted as operators.

- [ ] **Step 3: Commit**

```bash
git add next/src-tauri/src/engine/library_scanner.rs
git commit -m "fix: stop shelling out through cmd.exe to open files

A filename containing &, |, or ^ (legal on Windows) let cmd.exe's own
command-line parser treat part of the filename as a second command
when the user clicked Play. open::that() uses ShellExecuteW directly
with the path as a single parameter, closing the injection."
```

---

### Task 2: Add CSRF `state` parameter to the AniList OAuth flow

**Files:**
- Modify: `next/src-tauri/src/engine/anilist/oauth.rs`
- Modify: `next/src-tauri/Cargo.toml` (add `rand` dependency)
- Test: `next/src-tauri/tests/anilist_auth_test.rs`

**Interfaces:**
- Consumes: new `rand = "0.8"` dependency.
- Produces: `start_oauth_flow` keeps its existing signature `pub async fn start_oauth_flow(client_id: &str, client_secret: &str) -> anyhow::Result<String>` — no caller changes needed.

**Root cause:** The authorize URL carries no `state` nonce, and `wait_for_callback` accepts the `code` from whichever connection reaches the fixed loopback port first, with no way to tell it apart from an attacker's connection. This is the standard OAuth login-CSRF gap.

- [ ] **Step 1: Add the `rand` dependency**

In `next/src-tauri/Cargo.toml`, add to `[dependencies]` (alphabetically near `regex`):

```toml
rand = "0.8"
```

- [ ] **Step 2: Write a failing test for state-mismatch rejection**

Add to `next/src-tauri/tests/anilist_auth_test.rs` (check the top of that file first for its existing imports/helpers and match its style — the function below assumes `wait_for_callback_with_state` will be a new, testable, non-`pub(crate)`-restricted helper):

```rust
use anivault_core::engine::anilist::oauth::generate_state;

#[test]
fn generate_state_produces_distinct_unpredictable_values() {
    let a = generate_state();
    let b = generate_state();
    assert_ne!(a, b, "two consecutive nonces must not collide");
    assert!(a.len() >= 16, "nonce should be long enough to resist guessing");
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cd next/src-tauri && cargo test generate_state_produces_distinct_unpredictable_values`
Expected: FAIL — `generate_state` does not exist yet (compile error).

- [ ] **Step 4: Implement the state nonce and wire it into the flow**

Replace the full contents of `next/src-tauri/src/engine/anilist/oauth.rs` with:

```rust
use anyhow::anyhow;
use rand::Rng;
use serde::Deserialize;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;

const ANILIST_AUTH_URL: &str = "https://anilist.co/api/v2/oauth/authorize";
const ANILIST_TOKEN_URL: &str = "https://anilist.co/api/v2/oauth/token";
const OAUTH_PORT: u16 = 35789;

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}

/// Generate an unpredictable nonce for the OAuth `state` parameter, used to
/// verify the callback came from the authorization request we made (not an
/// attacker racing our fixed loopback port).
pub fn generate_state() -> String {
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| {
            const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
            CHARS[rng.gen_range(0..CHARS.len())] as char
        })
        .collect()
}

/// Start the OAuth flow: open browser and wait for the redirect callback.
/// Returns the access token on success.
pub async fn start_oauth_flow(client_id: &str, client_secret: &str) -> anyhow::Result<String> {
    // Bind to fixed port for registered redirect_uri
    let addr = format!("127.0.0.1:{}", OAUTH_PORT);
    let listener = TcpListener::bind(&addr).await
        .map_err(|e| anyhow!("Port {} is in use. Close other AniVault instances or change OAUTH_PORT. Error: {}", OAUTH_PORT, e))?;
    let redirect_uri = format!("http://{}", addr);
    let expected_state = generate_state();

    // Build auth URL
    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&state={}",
        ANILIST_AUTH_URL,
        urlencoding::encode(client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&expected_state),
    );

    // Open browser
    open_browser(&auth_url);

    // Wait for the callback (2 minute timeout). Keeps accepting connections
    // until one presents the matching `state`, so a stray/racing local
    // connection can't hijack the flow.
    let code = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        wait_for_matching_callback(listener, &expected_state),
    )
    .await
    .map_err(|_| anyhow!("OAuth timed out after 2 minutes"))??;

    // Exchange code for token
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let resp: TokenResponse = client
        .post(ANILIST_TOKEN_URL)
        .json(&serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": client_id,
            "client_secret": client_secret,
            "redirect_uri": redirect_uri,
            "code": code,
        }))
        .send()
        .await?
        .json()
        .await?;

    Ok(resp.access_token)
}

fn open_browser(url: &str) {
    // Try `open` crate first, fall back to cmd /c start on Windows
    if open::that(url).is_err() {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", url])
            .spawn();
    }
}

/// Accept connections until one carries `code` and a `state` matching
/// `expected_state`; anything else is rejected and we keep listening.
async fn wait_for_matching_callback(
    listener: TcpListener,
    expected_state: &str,
) -> anyhow::Result<String> {
    loop {
        let (stream, _) = listener.accept().await?;
        match handle_callback_connection(stream, expected_state).await? {
            Some(code) => return Ok(code),
            None => continue,
        }
    }
}

/// Handle a single connection: parse the callback request, and return the
/// code only if `state` matches. Returns `Ok(None)` for a non-matching or
/// malformed request so the caller keeps waiting for the real callback.
async fn handle_callback_connection(
    stream: tokio::net::TcpStream,
    expected_state: &str,
) -> anyhow::Result<Option<String>> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await?;

    let path = match request_line.split_whitespace().nth(1) {
        Some(p) => p,
        None => return Ok(None),
    };

    let code = parse_query_param(path, "code");
    let state = parse_query_param(path, "state");

    use tokio::io::AsyncWriteExt;
    let matched = code.is_some() && state.as_deref() == Some(expected_state);
    let (status_line, body) = if matched {
        ("200 OK", "<h1>Connected!</h1><p>AniVault has received your authorization. You may close this window.</p>")
    } else {
        ("400 Bad Request", "<h1>Invalid request</h1><p>This request did not match the expected AniVault sign-in. You can close this window.</p>")
    };
    let response_html = format!(
        "HTTP/1.1 {status_line}\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n\
        <!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>AniVault</title>\
        <style>body{{background:#080a0f;color:#f4f7fb;font-family:system-ui,sans-serif;\
        display:flex;align-items:center;justify-content:center;height:100vh;margin:0}}\
        .box{{text-align:center;padding:2rem;border:1px solid rgba(143,183,255,0.2);\
        border-radius:16px}}h1{{font-size:1.4rem;color:#8fb7ff}}p{{color:#9aa6b8}}\
        </style></head><body><div class=\"box\">{body}</div></body></html>"
    );
    let mut stream = reader.into_inner();
    let _ = stream.write_all(response_html.as_bytes()).await;

    if matched {
        Ok(code)
    } else {
        Ok(None)
    }
}

fn parse_query_param(path: &str, key: &str) -> Option<String> {
    let query = path.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        let k = parts.next()?;
        let v = parts.next().unwrap_or("");
        if k == key {
            return Some(urlencoding::decode(v).ok()?.to_string());
        }
    }
    None
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cd next/src-tauri && cargo test generate_state_produces_distinct_unpredictable_values`
Expected: PASS

- [ ] **Step 6: Run the full test suite for this module**

Run: `cd next/src-tauri && cargo test anilist_auth`
Expected: all existing tests in `anilist_auth_test.rs` still PASS (they exercise other parts of the auth flow; confirm none hardcoded the old query-string shape without `state`).

- [ ] **Step 7: Commit**

```bash
git add next/src-tauri/Cargo.toml next/src-tauri/src/engine/anilist/oauth.rs next/src-tauri/tests/anilist_auth_test.rs
git commit -m "fix: add CSRF state parameter to AniList OAuth flow

The loopback callback accepted a code from whichever connection hit
the fixed port first, with no way to verify it was the browser we
launched. Add a random state nonce to the auth URL and keep listening
until a connection presents a matching state, rejecting anything else."
```

---

### Task 3: Validate `restore_database` input and take a pre-restore safety backup

**Files:**
- Modify: `next/src-tauri/src/engine/migration/backup.rs`
- Test: add `#[cfg(test)] mod tests` cases to the same file (it already has one)

**Interfaces:**
- Consumes: `Storage::database_path(&self) -> &str` (already exists — used at `backup.rs:12`).
- Produces: `restore_database` keeps its signature `pub async fn restore_database(storage: &Storage, backup_path: &str) -> anyhow::Result<String>` — `commands.rs:975` needs no changes.

**Root cause:** `restore_database` only checks the target path exists, then unconditionally copies it over the live DB with no check it's an actual SQLite file, and no safety backup of the *current* DB is taken first — so a wrong path or a corrupt file destroys the working database irrecoverably.

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)] mod tests` block at the bottom of `next/src-tauri/src/engine/migration/backup.rs` (after the existing `export_then_import_roundtrip` test):

```rust
    #[tokio::test]
    async fn restore_rejects_non_sqlite_file() {
        let storage = Tests::new_in_memory().await;
        let bogus_path = std::env::temp_dir().join(format!(
            "anivault-test-bogus-{}.db",
            std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::write(&bogus_path, b"not a sqlite database").unwrap();

        let result = restore_database(&storage, bogus_path.to_str().unwrap()).await;

        std::fs::remove_file(&bogus_path).ok();
        assert!(result.is_err(), "restoring a non-SQLite file must be rejected");
    }

    #[tokio::test]
    async fn restore_takes_a_safety_backup_of_the_current_db_first() {
        // Use a real file-backed database (not :memory:) so backup/restore's
        // file-copy logic has an actual file to operate on.
        let db_path = std::env::temp_dir().join(format!(
            "anivault-test-restore-{}.db",
            std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let db_url = format!("sqlite:{}?mode=rwc", db_path.to_str().unwrap());
        let storage = Storage::connect(&db_url).await.unwrap();
        storage.migrate().await.unwrap();
        storage.insert_minimal_anime(1, "Original DB").await.unwrap();

        // Make a valid backup file to restore *from*.
        let good_backup_path = backup_database(&storage).await.unwrap();

        // Mutate the live DB so we can tell a safety backup captured the
        // pre-restore state.
        storage.insert_minimal_anime(2, "Changed before restore").await.unwrap();

        restore_database(&storage, &good_backup_path).await.unwrap();

        // A safety backup of the pre-restore state must exist on disk,
        // distinct from the backup we restored from.
        let db_dir = db_path.parent().unwrap();
        let stem = db_path.file_name().unwrap().to_str().unwrap();
        let safety_backups: Vec<_> = std::fs::read_dir(db_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with(stem) && name.contains(".pre-restore.")
            })
            .collect();
        assert!(
            !safety_backups.is_empty(),
            "expected a .pre-restore. safety backup file next to {}",
            db_path.display()
        );

        // Cleanup
        std::fs::remove_file(&db_path).ok();
        std::fs::remove_file(&good_backup_path).ok();
        for f in safety_backups {
            std::fs::remove_file(f.path()).ok();
        }
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd next/src-tauri && cargo test --lib restore_rejects_non_sqlite_file restore_takes_a_safety_backup_of_the_current_db_first`
Expected: FAIL — `restore_rejects_non_sqlite_file` fails because the bogus file is currently accepted; `restore_takes_a_safety_backup_of_the_current_db_first` fails because no `.pre-restore.` file is created.

- [ ] **Step 3: Implement validation and the safety backup**

Replace `restore_database` in `next/src-tauri/src/engine/migration/backup.rs:27-55`:

```rust
/// Restore the database from a backup file.
/// This will close the current pool, replace the DB file, and requires app restart.
/// Returns the backup path that was restored.
pub async fn restore_database(
    storage: &Storage,
    backup_path: &str,
) -> anyhow::Result<String> {
    let db_path = storage.database_path().to_owned();

    // Verify backup exists and is actually a SQLite database before touching
    // the live DB — a wrong or corrupt path must not destroy working data.
    if !std::path::Path::new(backup_path).exists() {
        anyhow::bail!("Backup file not found: {}", backup_path);
    }
    verify_sqlite_file(backup_path)?;

    // Safety net: back up the *current* (pre-restore) database so a restore
    // from the wrong backup, or a change of mind, is itself reversible.
    let pre_restore_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let pre_restore_path = format!("{}.pre-restore.{}", db_path, pre_restore_timestamp);
    storage.wal_checkpoint().await?;
    std::fs::copy(&db_path, &pre_restore_path)?;

    // Close pool, then replace DB file.
    storage.close().await;

    // Replace DB file. Remove stale WAL/SHM sidecars so leftover journal pages
    // from the old database can't be replayed over the restored file.
    std::fs::copy(backup_path, &db_path)?;
    let _ = std::fs::remove_file(format!("{db_path}-wal"));
    let _ = std::fs::remove_file(format!("{db_path}-shm"));

    Ok(format!(
        "Database restored from {}. Previous database saved to {}. Restart required.",
        backup_path, pre_restore_path
    ))
}

/// Check the file starts with SQLite's 16-byte magic header, rejecting
/// anything that clearly isn't a SQLite database before we copy it over the
/// live DB.
fn verify_sqlite_file(path: &str) -> anyhow::Result<()> {
    const SQLITE_HEADER: &[u8] = b"SQLite format 3\0";
    let mut file = std::fs::File::open(path)?;
    let mut header = [0u8; 16];
    use std::io::Read;
    file.read_exact(&mut header)
        .map_err(|_| anyhow::anyhow!("{} is too small to be a SQLite database", path))?;
    if header != SQLITE_HEADER {
        anyhow::bail!("{} does not look like a SQLite database file", path);
    }
    Ok(())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd next/src-tauri && cargo test --lib restore_rejects_non_sqlite_file restore_takes_a_safety_backup_of_the_current_db_first`
Expected: PASS

- [ ] **Step 5: Run the rest of the module's tests**

Run: `cd next/src-tauri && cargo test --lib migration::backup`
Expected: all PASS, including the pre-existing `export_empty_database` / `export_then_import_roundtrip`.

- [ ] **Step 6: Commit**

```bash
git add next/src-tauri/src/engine/migration/backup.rs
git commit -m "fix: validate restore_database input and take a pre-restore safety backup

restore_database only checked the target path existed, then
unconditionally overwrote the live DB — a wrong or corrupt path
destroyed the working database with no way back. Verify the SQLite
file header before restoring, and always snapshot the current DB to
a .pre-restore.<timestamp> file first."
```

---

### Task 4: Guard `map_folder_to_anime` against missing folders and filesystem roots

**Files:**
- Modify: `next/src-tauri/src/commands.rs:1662-1686`
- Test: `next/src-tauri/tests/library_commands_test.rs` (check existing style first; add new test functions there)

**Interfaces:**
- Consumes: `library_scanner::find_video_files` (unchanged from Task 7 caller's perspective — signature stays `find_video_files(dir: &Path, files: &mut Vec<PathBuf>, errors: &mut Vec<String>)`).
- Produces: `map_folder_to_anime_inner` keeps its signature `pub async fn map_folder_to_anime_inner(folder: &str, anime_id: i64, state: &EngineState) -> anyhow::Result<usize>`.

**Root cause:** `map_folder_to_anime_inner` is intentionally allowed to map *any* folder the user picks via the native OS folder dialog to an anime (this is a real feature, not a bug — restricting it to "configured library roots" would break it). But there's no guard against an empty/nonexistent path or against accidentally recursively scanning an entire drive (e.g. a stray `C:\` from a mis-clicked dialog or a manually-crafted call), which would be a serious, slow, accidental mass-scan.

- [ ] **Step 1: Write failing tests**

First read the top ~20 lines of `next/src-tauri/tests/library_commands_test.rs` to match its existing `test_state()`/setup helper, then add:

```rust
#[tokio::test]
async fn map_folder_to_anime_rejects_nonexistent_folder() {
    let state = test_state().await;
    state.storage.insert_minimal_anime(1, "Test Anime").await.unwrap();

    let result = map_folder_to_anime_inner("D:/this/path/does/not/exist", 1, &state).await;

    assert!(result.is_err(), "mapping a nonexistent folder must fail");
}

#[tokio::test]
async fn map_folder_to_anime_rejects_filesystem_root() {
    let state = test_state().await;
    state.storage.insert_minimal_anime(1, "Test Anime").await.unwrap();

    // Whichever root exists on this machine — on Windows CI this is a drive
    // root, on Unix it's "/". Either way it has no parent.
    let root = std::path::Path::new("/")
        .ancestors()
        .last()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let result = map_folder_to_anime_inner(&root, 1, &state).await;

    assert!(result.is_err(), "mapping a filesystem root must be refused");
}
```

(Use the exact `test_state()` helper name already present in that file — adjust the two lines above if the file's actual helper has a different name, e.g. `fresh_test_state().await` as seen in `matcher_test.rs`.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd next/src-tauri && cargo test map_folder_to_anime_rejects`
Expected: FAIL — both folders are currently accepted (an empty file list is returned successfully rather than an error).

- [ ] **Step 3: Implement the guard**

Modify `map_folder_to_anime_inner` in `next/src-tauri/src/commands.rs:1662-1686`:

```rust
pub async fn map_folder_to_anime_inner(
    folder: &str,
    anime_id: i64,
    state: &EngineState,
) -> anyhow::Result<usize> {
    let path = std::path::Path::new(folder);
    if !path.is_dir() {
        anyhow::bail!("Folder does not exist: {folder}");
    }
    if path.parent().is_none() {
        anyhow::bail!("Refusing to scan a filesystem root: {folder}");
    }

    let mut files: Vec<std::path::PathBuf> = Vec::new();
    let mut errs: Vec<String> = Vec::new();
    library_scanner::find_video_files(path, &mut files, &mut errs);

    let mappings: Vec<(String, i64, i32)> = files
        .iter()
        .map(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            let episode = crate::engine::parser::parse_filename(name, None)
                .map(|pf| pf.episode_number)
                .filter(|e| *e > 0)
                .unwrap_or(0);
            (p.to_string_lossy().to_string(), anime_id, episode)
        })
        .collect();

    let now = unix_now_inner()?;
    state.storage.upsert_file_mappings(&mappings, now).await?;
    Ok(mappings.len())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd next/src-tauri && cargo test map_folder_to_anime_rejects`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add next/src-tauri/src/commands.rs next/src-tauri/tests/library_commands_test.rs
git commit -m "fix: guard map_folder_to_anime against missing folders and drive roots

No validation meant a bad path silently mapped zero files, and a
mis-picked drive root would recursively walk the entire drive."
```

---

### Task 5: Abort migration when the pre-migration backup fails

**Files:**
- Modify: `next/src-tauri/src/commands.rs:948-963`
- Test: `next/src-tauri/tests/migration_test.rs` (check existing helpers/imports first)

**Interfaces:**
- Produces: `run_migration_inner` keeps its signature `pub async fn run_migration_inner(state: &EngineState, strategy: DuplicateStrategy) -> Result<MigrationReport, String>`.

**Root cause:** `if let Err(e) = backup::backup_database(...) { tracing::warn!(...); }` logs and swallows a backup failure, then runs the (merging/importing) migration anyway — so a disk-full or locked-file backup failure leaves the user with no safety net during a destructive operation, silently.

- [ ] **Step 1: Implement the fix**

Replace `next/src-tauri/src/commands.rs:948-963`:

```rust
pub async fn run_migration_inner(
    state: &EngineState,
    strategy: DuplicateStrategy,
) -> Result<MigrationReport, String> {
    let paths = discovery::discover_v1_data();
    if !paths.found {
        return Err("No v1 data found. Cannot run migration.".to_string());
    }
    // Backup first — abort rather than run a destructive import with no
    // safety net if the backup itself couldn't be taken.
    backup::backup_database(&state.storage)
        .await
        .map_err(|e| format!("Pre-migration backup failed, migration aborted: {e}"))?;
    importer::live_import(&state.storage, &paths, strategy)
        .await
        .map_err(command_error)
}
```

- [ ] **Step 2: Write a regression test for the happy path**

There is no existing test exercising `run_migration_inner` at all. Add to `next/src-tauri/tests/migration_test.rs` (match its existing imports/helpers — it likely already imports from `anivault_core::engine::migration` and `anivault_core::engine::runtime`):

```rust
#[tokio::test]
async fn run_migration_inner_returns_error_when_no_v1_data_found() {
    let state = fresh_test_state().await;
    // discover_v1_data() looks at real OS paths; on a clean test machine with
    // no legacy v1 install this returns found: false, which is exactly the
    // early-return branch this test locks in.
    let result = run_migration_inner(&state, DuplicateStrategy::Skip).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("No v1 data found"));
}
```

(Use whichever test-state constructor the rest of that file already uses — `fresh_test_state()` per `matcher_test.rs`, or `Tests::new_in_memory()` — match the file's existing convention rather than introducing a second one.)

- [ ] **Step 3: Run it**

Run: `cd next/src-tauri && cargo test run_migration_inner_returns_error_when_no_v1_data_found`
Expected: PASS (this exercises the pre-existing early-return branch, unchanged by this task — it's a regression guard for the function overall, since a full backup-failure-injection test would require unreliable filesystem-fault simulation not worth the flakiness).

- [ ] **Step 4: Commit**

```bash
git add next/src-tauri/src/commands.rs next/src-tauri/tests/migration_test.rs
git commit -m "fix: abort migration instead of continuing when the pre-migration backup fails

A failed backup was logged and swallowed, then the destructive import
ran anyway with no safety net. Propagate the error and abort instead."
```

---

### Task 6: Detect ambiguous filename matches instead of silently guessing

**Files:**
- Modify: `next/src-tauri/src/engine/storage.rs:620-647`
- Test: `next/src-tauri/tests/storage_test.rs` (check existing style first)

**Interfaces:**
- Produces: `get_file_index_by_filename` keeps its signature `pub async fn get_file_index_by_filename(&self, filename: &str) -> anyhow::Result<Option<FileIndexRow>>` — callers (`matcher.rs:146`) need no changes; `None` already means "fall through to full parse+search", which is exactly the safe behavior an ambiguous match should get.

**Root cause:** Two different shows (or two seasons of the same show) with an identically-named episode file — common with generic fansub numbering like `01.mkv` — collide on the basename `LIKE` lookup. The query already does `ORDER BY confidence DESC, indexed_at DESC LIMIT 1`, silently picking a winner instead of surfacing that the match is ambiguous.

- [ ] **Step 1: Write a failing test**

Add to `next/src-tauri/tests/storage_test.rs` (match its existing `Tests::new_in_memory()`/setup pattern seen in `matcher_test.rs` and `backup.rs`'s own tests):

```rust
#[tokio::test]
async fn get_file_index_by_filename_returns_none_when_ambiguous() {
    let storage = Tests::new_in_memory().await;
    storage.insert_minimal_anime(1, "Show A Season 1").await.unwrap();
    storage.insert_minimal_anime(2, "Show A Season 2").await.unwrap();

    // Two different shows, each with an episode file that happens to share
    // the exact same basename — a real-world case with generic numbering.
    storage
        .upsert_file_index("D:/Anime/Show A S1/01.mkv", Some(1), 1, 100, 1_782_769_000)
        .await
        .unwrap();
    storage
        .upsert_file_index("D:/Anime/Show A S2/01.mkv", Some(2), 1, 100, 1_782_769_001)
        .await
        .unwrap();

    let result = storage.get_file_index_by_filename("01.mkv").await.unwrap();

    assert!(
        result.is_none(),
        "an ambiguous basename match (two different anime_ids) must not silently pick a winner, got {:?}",
        result
    );
}

#[tokio::test]
async fn get_file_index_by_filename_still_resolves_unambiguous_match() {
    let storage = Tests::new_in_memory().await;
    storage.insert_minimal_anime(1, "Cowboy Bebop").await.unwrap();
    storage
        .upsert_file_index("D:/Anime/Cowboy Bebop - 01.mkv", Some(1), 1, 100, 1_782_769_000)
        .await
        .unwrap();

    let result = storage
        .get_file_index_by_filename("Cowboy Bebop - 01.mkv")
        .await
        .unwrap();

    assert_eq!(result.map(|r| r.anime_id), Some(Some(1)));
}
```

- [ ] **Step 2: Run tests to verify the ambiguity test fails**

Run: `cd next/src-tauri && cargo test get_file_index_by_filename`
Expected: `get_file_index_by_filename_returns_none_when_ambiguous` FAILS (currently returns `Some` with whichever row has higher confidence/recency); `get_file_index_by_filename_still_resolves_unambiguous_match` already PASSES.

- [ ] **Step 3: Implement the ambiguity check**

Replace `get_file_index_by_filename` in `next/src-tauri/src/engine/storage.rs:620-647`:

```rust
    /// Look up a mapped file by its filename (basename) rather than full path.
    /// Players like mpv only surface the filename in their window title, so the
    /// absolute-path index lookup misses; this matches on the trailing filename.
    /// Returns `None` (falling through to full parse+search) rather than
    /// guessing when two different anime share an identically-named episode
    /// file — generic fansub numbering like "01.mkv" makes this a real case,
    /// not just a theoretical one.
    pub async fn get_file_index_by_filename(
        &self,
        filename: &str,
    ) -> anyhow::Result<Option<FileIndexRow>> {
        // Escape LIKE metacharacters so titles with % or _ match literally.
        let escaped = filename
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped}");
        let rows = sqlx::query(
            "SELECT file_path, anime_id, episode, confidence, indexed_at, ignored \
             FROM file_index \
             WHERE file_path LIKE ?1 ESCAPE '\\' AND anime_id IS NOT NULL AND ignored = 0 \
             ORDER BY confidence DESC, indexed_at DESC",
        )
        .bind(pattern)
        .fetch_all(&self.pool)
        .await?;

        let distinct_anime: std::collections::HashSet<Option<i64>> =
            rows.iter().map(|r| r.get::<Option<i64>, _>("anime_id")).collect();
        if distinct_anime.len() > 1 {
            return Ok(None);
        }

        Ok(rows.into_iter().next().map(|row| FileIndexRow {
            file_path: row.get("file_path"),
            anime_id: row.get("anime_id"),
            episode: row.get("episode"),
            confidence: row.get("confidence"),
            indexed_at: row.get("indexed_at"),
            ignored: row.get::<i64, _>("ignored") != 0,
        }))
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd next/src-tauri && cargo test get_file_index_by_filename`
Expected: both PASS

- [ ] **Step 5: Run the matcher tests to confirm no regression**

Run: `cd next/src-tauri && cargo test matcher`
Expected: all PASS — `recognize_known_file_skips_matching` in `matcher_test.rs` uses a single unambiguous file, unaffected.

- [ ] **Step 6: Commit**

```bash
git add next/src-tauri/src/engine/storage.rs next/src-tauri/tests/storage_test.rs
git commit -m "fix: don't silently guess when a filename match is ambiguous

Two different shows with an identically-named episode file (common
with generic fansub numbering) collided on the basename lookup,
silently mis-attributing watch progress to whichever row won on
confidence/recency. Fall through to full title matching instead."
```

---

### Task 7: Prevent unbounded recursion / directory-junction loops in the library scan

**Files:**
- Modify: `next/src-tauri/src/engine/library_scanner.rs:467-487`
- Test: `next/src-tauri/tests/scanner_test.rs` or a new file — check existing scan tests first (`library_scan_prune_test.rs` looks like the closest existing coverage; read its setup helpers first)

**Interfaces:**
- Produces: `pub fn find_video_files(dir: &Path, files: &mut Vec<PathBuf>, errors: &mut Vec<String>)` — signature unchanged, so `commands.rs:1669` (Task 4) and any other caller need no changes.

**Root cause:** `find_video_files` recurses via `path.is_dir()`, which follows symlinks/junctions, with no cycle detection or depth cap. A Windows directory junction loop inside a library folder (not uncommon with backup tools) causes unbounded recursion and a stack-overflow crash on every scan.

- [ ] **Step 1: Write a regression test for normal nested scanning**

First read `next/src-tauri/tests/library_scan_prune_test.rs` in full to find its temp-directory setup helper (it almost certainly creates real files under `std::env::temp_dir()` already, given it tests pruning of removed files). Reuse that same helper pattern. Add to that file (or `scanner_test.rs` if that's the more appropriate home per the file's existing scope):

```rust
#[test]
fn find_video_files_still_finds_nested_files_after_cycle_guard() {
    let base = std::env::temp_dir().join(format!(
        "anivault-scan-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let nested = base.join("Season 1").join("Sub");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("01.mkv"), b"").unwrap();
    std::fs::write(base.join("00.mkv"), b"").unwrap();

    let mut files = Vec::new();
    let mut errors = Vec::new();
    anivault_core::engine::library_scanner::find_video_files(&base, &mut files, &mut errors);

    std::fs::remove_dir_all(&base).ok();

    assert_eq!(files.len(), 2, "expected both nested and top-level video files, got {:?}", files);
    assert!(errors.is_empty());
}
```

(This is a **regression** test for the refactor below, not a reproduction of the cycle bug itself — creating a real Windows directory junction requires `mklink /J` or the `junction`-style Win32 APIs, which needs privileges/extra setup not justified here. The cycle-guard logic is verified by code inspection: every directory's canonicalized path is inserted into a `visited` set before recursing, and a second visit to the same canonical path is skipped.)

- [ ] **Step 2: Run it to verify it passes against current behavior first**

Run: `cd next/src-tauri && cargo test find_video_files_still_finds_nested_files_after_cycle_guard`
Expected: PASS already (this test only locks in existing correct behavior before the refactor).

- [ ] **Step 3: Implement the cycle guard and depth cap**

Replace `find_video_files` in `next/src-tauri/src/engine/library_scanner.rs:467-487`:

```rust
const MAX_SCAN_DEPTH: u32 = 64;

/// Recursively find video files under a directory.
/// Collects errors for unreadable directories instead of silently skipping.
/// Guards against directory-junction/symlink cycles (each directory's
/// canonicalized path is visited at most once) and against runaway depth.
pub fn find_video_files(dir: &Path, files: &mut Vec<std::path::PathBuf>, errors: &mut Vec<String>) {
    let mut visited = std::collections::HashSet::new();
    find_video_files_inner(dir, files, errors, &mut visited, 0);
}

fn find_video_files_inner(
    dir: &Path,
    files: &mut Vec<std::path::PathBuf>,
    errors: &mut Vec<String>,
    visited: &mut std::collections::HashSet<std::path::PathBuf>,
    depth: u32,
) {
    if depth > MAX_SCAN_DEPTH {
        errors.push(format!("Max scan depth ({MAX_SCAN_DEPTH}) exceeded at {}", dir.display()));
        return;
    }

    let canonical = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if !visited.insert(canonical) {
        // Already visited this real directory — a symlink/junction cycle.
        return;
    }

    match std::fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    find_video_files_inner(&path, files, errors, visited, depth + 1);
                } else if path.is_file() && is_video_file(&path) {
                    files.push(path);
                }
            }
        }
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                errors.push(format!("Cannot read {}: {}", dir.display(), e));
            }
        }
    }
}
```

- [ ] **Step 4: Run the regression test again**

Run: `cd next/src-tauri && cargo test find_video_files_still_finds_nested_files_after_cycle_guard`
Expected: PASS

- [ ] **Step 5: Run the full scanner/scan-prune test suites**

Run: `cd next/src-tauri && cargo test scan`
Expected: all PASS (in particular anything in `library_scan_prune_test.rs` that scans real nested temp directories).

- [ ] **Step 6: Commit**

```bash
git add next/src-tauri/src/engine/library_scanner.rs next/src-tauri/tests/library_scan_prune_test.rs
git commit -m "fix: guard library scan against directory-junction cycles

find_video_files followed symlinks/junctions with no cycle detection,
so a junction loop in a watched folder crashed every scan via stack
overflow. Track visited canonical paths and cap recursion depth."
```

(Adjust the `git add` target in Step 6 to whichever test file Step 1 actually landed the test in.)

---

### Task 8: Add timeouts to all outbound HTTP clients

**Files:**
- Modify: `next/src-tauri/src/engine/anilist/client.rs:136-141`
- Modify: `next/src-tauri/src/engine/sonarr/client.rs:100-108`
- (`next/src-tauri/src/engine/anilist/oauth.rs` token-exchange client is already covered by Task 2's rewrite, which already builds with `.timeout(30s)`.)

**Interfaces:** No signature changes — `AniListClient::new(token: String)` and `SonarrClient::new(url: String, api_key: String)` are unaffected.

**Root cause:** All three `reqwest::Client::new()` call sites use reqwest's default of no request timeout. An unreachable or slow Sonarr instance (very plausible for a self-hosted server behind VPN/sleep) hangs the calling Tauri command — and therefore the UI awaiting it — indefinitely.

- [ ] **Step 1: Fix the AniList client**

In `next/src-tauri/src/engine/anilist/client.rs`, replace lines 134-141:

```rust
impl AniListClient {
    /// Create a new client with the given access token.
    pub fn new(token: String) -> Self {
        Self {
            token,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build reqwest client"),
        }
    }
```

- [ ] **Step 2: Fix the Sonarr client**

In `next/src-tauri/src/engine/sonarr/client.rs`, replace lines 100-108:

```rust
impl SonarrClient {
    pub fn new(url: String, api_key: String) -> Self {
        let url = url.trim_end_matches('/').to_string();
        Self {
            url,
            api_key,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build reqwest client"),
        }
    }
```

- [ ] **Step 3: Verify it builds and existing client tests still pass**

Run: `cd next/src-tauri && cargo test anilist_client sonarr_client`
Expected: all PASS — these tests mock the HTTP transport or hit a local mock server, so a client-level timeout doesn't change their behavior.

- [ ] **Step 4: Commit**

```bash
git add next/src-tauri/src/engine/anilist/client.rs next/src-tauri/src/engine/sonarr/client.rs
git commit -m "perf: add 30s timeout to AniList and Sonarr HTTP clients

reqwest's default client has no request timeout, so an unreachable
or slow Sonarr/AniList endpoint hung the calling Tauri command (and
the UI awaiting it) indefinitely."
```

---

### Task 9: Add a missing index on `file_index.anime_id`

**Files:**
- Create: `next/src-tauri/migrations/0006_file_index_anime_id_idx.sql`
- Test: `next/src-tauri/tests/storage_test.rs`

**Root cause:** `file_index_by_anime` (`storage.rs:649-667`) filters `WHERE anime_id = ?1` on every anime-detail page load and every "Rescan" click, but `file_index` has no index on `anime_id` — full table scan every time.

- [ ] **Step 1: Add the migration**

Create `next/src-tauri/migrations/0006_file_index_anime_id_idx.sql`:

```sql
CREATE INDEX IF NOT EXISTS idx_file_index_anime_id ON file_index(anime_id);
```

- [ ] **Step 2: Write a test confirming the migration applies cleanly and the index exists**

Add to `next/src-tauri/tests/storage_test.rs`:

```rust
#[tokio::test]
async fn file_index_anime_id_index_exists_after_migration() {
    let storage = Tests::new_in_memory().await;
    let row = sqlx::query("SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'idx_file_index_anime_id'")
        .fetch_optional(storage.pool_for_test())
        .await
        .unwrap();
    assert!(row.is_some(), "expected idx_file_index_anime_id to exist after migrate()");
}
```

This requires a small test-only accessor since `Storage::pool` is private. Add to the `impl Tests` block in `next/src-tauri/src/engine/storage.rs:1882-1888`:

```rust
impl Tests {
    pub async fn new_in_memory() -> Storage {
        let storage = Storage::connect("sqlite::memory:").await.unwrap();
        storage.migrate().await.unwrap();
        storage
    }
}

impl Storage {
    #[cfg(any(test, feature = "test-utils"))]
    pub fn pool_for_test(&self) -> &SqlitePool {
        &self.pool
    }
}
```

Check first whether `next/src-tauri/tests/*.rs` integration tests can even see `#[cfg(test)]`-gated items across the crate boundary — integration tests in `tests/` compile the crate *without* `cfg(test)` from the crate's own perspective (only unit tests inside `src/` get `cfg(test)`). If `cfg(any(test, feature = "test-utils"))` doesn't work for integration tests in this project (check how other `tests/*.rs` files access otherwise-private state — e.g. grep for an existing `#[cfg(test)]` pub accessor pattern already used by another integration test), instead gate it as plain `#[doc(hidden)] pub fn pool_for_test(&self) -> &SqlitePool { &self.pool }` with no `cfg` at all, matching whatever convention the codebase already uses to expose test-only internals to `tests/*.rs`.

- [ ] **Step 3: Run it to verify it fails first (if adding the accessor before the migration, otherwise skip to Step 4)**

Run: `cd next/src-tauri && cargo test file_index_anime_id_index_exists_after_migration`
Expected: FAIL before Step 1's migration file exists; once both Step 1 and Step 2 are in place together, skip straight to Step 4.

- [ ] **Step 4: Run it to verify it passes**

Run: `cd next/src-tauri && cargo test file_index_anime_id_index_exists_after_migration`
Expected: PASS

- [ ] **Step 5: Run the full storage test suite**

Run: `cd next/src-tauri && cargo test storage`
Expected: all PASS — an added index changes no query results, only their cost.

- [ ] **Step 6: Commit**

```bash
git add next/src-tauri/migrations/0006_file_index_anime_id_idx.sql next/src-tauri/src/engine/storage.rs next/src-tauri/tests/storage_test.rs
git commit -m "perf: add missing index on file_index.anime_id

file_index_by_anime is hit on every anime-detail page load and every
Rescan click, filtering on anime_id with no supporting index — a full
table scan of file_index each time."
```

---

### Task 10: Cap the in-memory event bus buffer

**Files:**
- Modify: `next/src-tauri/src/engine/event_bus.rs`
- Test: `next/src-tauri/tests/event_bus_test.rs`

**Interfaces:** `EventBus::publish(&self, event: EngineEvent)` and `EventBus::drain(&self) -> Vec<EngineEvent>` keep their signatures.

**Root cause:** `EventBus` is a plain unbounded `Vec` drained only by a frontend poll. Nothing caps its size or pushes back if the UI stops polling (minimized/backgrounded window), so a long idle session accumulates events without bound.

- [ ] **Step 1: Write a failing test**

Add to `next/src-tauri/tests/event_bus_test.rs`:

```rust
#[test]
fn event_bus_caps_buffered_events_and_keeps_the_newest() {
    let bus = EventBus::default();

    for i in 0..1100 {
        bus.publish(EngineEvent::SyncFailed {
            service: "anilist".to_string(),
            anime_id: i,
            message: "test".to_string(),
        });
    }

    let events = bus.drain();
    assert!(events.len() <= 1000, "expected the buffer to be capped, got {} events", events.len());

    // The newest events (highest anime_id) must be the ones kept, not the
    // oldest — a cap that drops from the wrong end would silently discard
    // exactly the events the frontend most needs.
    let last = events.last().unwrap();
    assert!(matches!(last, EngineEvent::SyncFailed { anime_id: 1099, .. }));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cd next/src-tauri && cargo test event_bus_caps_buffered_events_and_keeps_the_newest`
Expected: FAIL — currently `events.len()` is 1100, uncapped.

- [ ] **Step 3: Implement the cap**

Replace the contents of `next/src-tauri/src/engine/event_bus.rs`:

```rust
use std::sync::{Arc, Mutex};

use crate::engine::events::EngineEvent;

/// Hard cap on buffered-but-undrained events. Prevents unbounded memory
/// growth if the frontend stops polling (minimized/backgrounded window,
/// frontend crash) while the tracking loop keeps publishing.
const MAX_BUFFERED_EVENTS: usize = 1000;

#[derive(Debug, Clone, Default)]
pub struct EventBus {
    events: Arc<Mutex<Vec<EngineEvent>>>,
}

impl EventBus {
    pub fn publish(&self, event: EngineEvent) {
        let mut events = self.events.lock().expect("event bus poisoned");
        events.push(event);
        if events.len() > MAX_BUFFERED_EVENTS {
            let excess = events.len() - MAX_BUFFERED_EVENTS;
            events.drain(0..excess);
        }
    }

    pub fn drain(&self) -> Vec<EngineEvent> {
        let mut events = self.events.lock().expect("event bus poisoned");
        std::mem::take(&mut *events)
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd next/src-tauri && cargo test event_bus_caps_buffered_events_and_keeps_the_newest`
Expected: PASS

- [ ] **Step 5: Run the rest of the event bus tests**

Run: `cd next/src-tauri && cargo test event_bus`
Expected: all PASS, including the pre-existing `event_bus_records_published_events_in_order`.

- [ ] **Step 6: Commit**

```bash
git add next/src-tauri/src/engine/event_bus.rs next/src-tauri/tests/event_bus_test.rs
git commit -m "fix: cap the event bus buffer to prevent unbounded growth

Nothing capped the buffer if the frontend stopped polling (minimized
window, frontend crash) while the tracking loop kept publishing.
Drop the oldest events once the buffer exceeds 1000."
```

---

### Task 11: Reuse `score_titles_json` in `recognize_file` instead of duplicating the scoring loop

**Files:**
- Modify: `next/src-tauri/src/engine/matcher.rs:176-215`
- Test: `next/src-tauri/tests/matcher_test.rs`

**Root cause:** `recognize_file`'s candidate loop re-implements the romaji/english/japanese/synonym scoring already centralized in `score_titles_json` (used by the library scanner), so any future change to scoring must be applied in two places or the two callers silently diverge.

- [ ] **Step 1: Write a regression test for current scoring behavior**

Add to `next/src-tauri/tests/matcher_test.rs`:

```rust
#[tokio::test]
async fn recognize_file_ranks_candidates_by_score_including_synonyms() {
    let state = test_state().await;
    let titles = serde_json::json!({
        "romaji": "Koe no Katachi",
        "english": "A Silent Voice",
        "japanese": null,
        "synonyms": ["Silent Voice"]
    })
    .to_string();
    state
        .storage
        .upsert_anime(1, &titles, 1, None, 0)
        .await
        .unwrap();

    // Matches only via the synonym, not romaji/english — this exercises the
    // synonym branch that a naive port of the old inline scoring could drop.
    let result = recognize_file("D:/Anime/Silent Voice - 01.mkv", None, &state.storage)
        .await
        .unwrap();

    assert!(
        result.candidates.iter().any(|c| c.anime_id == 1 && c.confidence >= 80),
        "expected a high-confidence synonym match, got {:?}",
        result.candidates
    );
}
```

(Match this test's `test_state()`/`upsert_anime` calls to whatever helpers the rest of `matcher_test.rs` actually uses — read the file's existing tests first, since `upsert_anime`'s exact parameter order isn't confirmed here.)

- [ ] **Step 2: Run it to verify it passes against current behavior first**

Run: `cd next/src-tauri && cargo test recognize_file_ranks_candidates_by_score_including_synonyms`
Expected: PASS already (locks in existing correct behavior before the refactor).

- [ ] **Step 3: Replace the inline scoring with `score_titles_json`**

In `next/src-tauri/src/engine/matcher.rs`, replace the loop body at lines 176-215:

```rust
    for anime in &matches {
        if !seen_ids.insert(anime.id) {
            continue;
        }
        let titles: serde_json::Value =
            serde_json::from_str(&anime.titles_json).unwrap_or_default();
        let romaji = titles["romaji"].as_str().unwrap_or("");

        let confidence = score_titles_json(&parsed.cleaned_title, &anime.titles_json);

        if confidence >= 20 {
            candidates.push(MatchCandidate {
                anime_id: anime.id,
                title: romaji.to_string(),
                confidence,
                match_source: "title_match".to_string(),
            });
        }
    }
```

- [ ] **Step 4: Run the test to verify it still passes**

Run: `cd next/src-tauri && cargo test recognize_file_ranks_candidates_by_score_including_synonyms`
Expected: PASS

- [ ] **Step 5: Run the full matcher test suite**

Run: `cd next/src-tauri && cargo test matcher`
Expected: all PASS, including the pre-existing `recognize_known_file_skips_matching` and `recognize_new_file_parses_and_searches`.

- [ ] **Step 6: Commit**

```bash
git add next/src-tauri/src/engine/matcher.rs next/src-tauri/tests/matcher_test.rs
git commit -m "refactor: reuse score_titles_json in recognize_file

recognize_file re-implemented the romaji/english/japanese/synonym
scoring loop that score_titles_json already centralizes for the
library scanner, risking silent divergence between the two callers."
```

---

### Task 12: Persist SeasonView's genre filter across navigation

**Files:**
- Modify: `next/src/lib/SeasonView.svelte:20-36,94`

**Root cause:** `season`/`year` are persisted to `localStorage` (line 94), but `genre` is not — navigating away and back resets the genre filter while season/year survive, an inconsistency within the same component.

- [ ] **Step 1: Extend the persisted state shape**

In `next/src/lib/SeasonView.svelte`, replace lines 20-36:

```js
  function loadSeasonState(): { season: string; year: number; genre: string } {
    try {
      const saved = localStorage.getItem('anivault-season-state');
      if (saved) {
        const parsed = JSON.parse(saved);
        return { season: parsed.season, year: parsed.year, genre: parsed.genre ?? '' };
      }
    } catch {}
    return { ...getCurrentSeason(), genre: '' };
  }

  function saveSeasonState(s: string, y: number, g: string) {
    try { localStorage.setItem('anivault-season-state', JSON.stringify({ season: s, year: y, genre: g })); }
    catch {}
  }

  let initial = loadSeasonState();
  let season = initial.season;
  let year = initial.year;
  let genre: string = initial.genre;
  let entries: SeasonAnimeEntry[] = [];
  let loading = true;
  let error: string | null = null;
  let libraryIds = new Set<number>();
```

- [ ] **Step 2: Update the persist call site**

Replace line 94:

```js
  $: saveSeasonState(season, year, genre);
```

- [ ] **Step 3: Verify it typechecks**

Run: `cd next && npm run check`
Expected: no new errors.

- [ ] **Step 4: Manual verification (no component test harness exists in this repo)**

Start `npm run dev`, go to Season, pick a non-default genre, navigate to another tab, navigate back to Season — genre selection should still be applied. This is a manual check, not an automated one; state it as such rather than claiming test coverage that doesn't exist.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/SeasonView.svelte
git commit -m "fix: persist SeasonView genre filter across navigation

season/year were already persisted to localStorage but genre wasn't,
so navigating away and back silently reset the genre filter while
everything else survived."
```

---

### Task 13: Persist LibraryView's search query across navigation

**Files:**
- Modify: `next/src/lib/LibraryView.svelte:29,89-90`

**Root cause:** `statusFilter`, `sortKey`/`sortDir`, `viewMode`, and `compact` are all persisted via the existing `loadPref`/`persistPref` helpers (lines 21-27, 86-90), but the free-text `query` (line 29) is not — the search box is the one control that resets on navigation.

- [ ] **Step 1: Load and persist `query` using the existing helpers**

In `next/src/lib/LibraryView.svelte`, replace line 29:

```js
  let query = loadPref('anivault-library-query', '');
```

Add alongside the existing persist calls at lines 89-90:

```js
  $: persistPref('anivault-library-viewmode', viewMode);
  $: persistPref('anivault-library-compact', compact ? 'true' : 'false');
  $: persistPref('anivault-library-query', query);
```

- [ ] **Step 2: Verify it typechecks**

Run: `cd next && npm run check`
Expected: no new errors.

- [ ] **Step 3: Manual verification**

`npm run dev`, go to Library, type a search query, navigate away and back — query and results should still be there (the existing `onMount(() => { void load(); ...})` at line 551-554 already calls `load()`, which reads the now-restored `query`, so no further wiring is needed).

- [ ] **Step 4: Commit**

```bash
git add next/src/lib/LibraryView.svelte
git commit -m "fix: persist LibraryView search query across navigation

Every other Library filter (status, sort, view mode, compact) was
already persisted to localStorage; the free-text search box was the
one control that silently reset on navigation."
```

---

### Task 14: Persist CalendarView's viewed month across navigation

**Files:**
- Modify: `next/src/lib/CalendarView.svelte:13,17-23`

**Root cause:** `viewMode` (month/agenda) is persisted via `localStorage` (lines 18-23), but `viewDate` (line 13, driving which month is displayed) is not — navigating three months ahead, then away and back, silently resets to the current month.

- [ ] **Step 1: Add load/save for `viewDate`, mirroring the existing `viewMode` pattern**

In `next/src/lib/CalendarView.svelte`, replace line 13:

```js
  function loadViewDate(): Date {
    try {
      const raw = localStorage.getItem('anivault-calendar-date');
      if (raw) {
        const d = new Date(raw);
        if (!isNaN(d.getTime())) return d;
      }
    } catch {}
    return new Date();
  }
  let viewDate = loadViewDate(); // current month being viewed
```

Add immediately after the existing `$: try { localStorage.setItem('anivault-calendar-view', viewMode); } catch {}` line (line 23):

```js
  $: try { localStorage.setItem('anivault-calendar-date', viewDate.toISOString()); } catch {}
```

- [ ] **Step 2: Verify it typechecks**

Run: `cd next && npm run check`
Expected: no new errors.

- [ ] **Step 3: Manual verification**

`npm run dev`, go to Calendar, navigate forward a few months, navigate away and back — the same month should still be shown.

- [ ] **Step 4: Commit**

```bash
git add next/src/lib/CalendarView.svelte
git commit -m "fix: persist CalendarView's viewed month across navigation

viewMode (month/agenda) was already persisted; viewDate (which month
is displayed) wasn't, so navigating forward a few months and back
silently reset to the current month."
```

---

### Task 15: Lift HistoryView state to App.svelte so back-navigation preserves it

**Files:**
- Modify: `next/src/lib/HistoryView.svelte:1-39`
- Modify: `next/src/App.svelte`

**Interfaces:**
- Produces: `HistoryView` gains `export let entries`, `export let query`, `export let offset`, `export let hasMore` (bindable), matching the pattern already used for `SearchView.svelte`'s `query`/`entries`/`hasSearched`.

**Root cause:** Same class of bug as `SearchView` (already fixed): `App.svelte`'s `{#if currentView === 'history'}` chain destroys `HistoryView` on nav-away and creates a fresh instance on return, wiping its component-local `entries`/`query`/`offset`/`hasMore`. Unlike `SeasonView`/`LibraryView`/`CalendarView`'s scalar preferences, this state is a fetched, potentially-multi-page result set — not a good fit for `localStorage` (size, staleness, serialization cost) — so it's lifted to `App.svelte` in memory instead, exactly like `SearchView`.

- [ ] **Step 1: Make the state bindable in HistoryView**

In `next/src/lib/HistoryView.svelte`, replace lines 5-39:

```js
  export let entries: WatchHistoryEntry[] = [];
  export let query = '';
  export let offset = 0;
  export let hasMore = true;
  let loading = true;
  let error: string | null = null;
  const pageSize = 50;

  async function load(reset = false) {
    if (reset) { offset = 0; entries = []; hasMore = true; }
    loading = true;
    error = null;
    try {
      const newEntries = await getWatchHistory(query || undefined, pageSize, offset);
      if (reset) entries = newEntries;
      else entries = [...entries, ...newEntries];
      hasMore = newEntries.length === pageSize;
      offset += newEntries.length;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function formatDate(unix: number): string {
    const d = new Date(unix * 1000);
    return d.toLocaleDateString() + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }

  function handleSearch() {
    load(true);
  }

  // Only fetch on the very first mount — after that, entries/offset/query
  // are restored from App.svelte's lifted state, so a remount from
  // navigating back must not blow that away with a fresh page-1 fetch.
  onMount(() => { if (entries.length === 0) load(true); });
```

- [ ] **Step 2: Lift the state into App.svelte**

In `next/src/App.svelte`, add near the existing `searchQuery`/`searchEntries`/`searchHasSearched` declarations:

```js
  let historyEntries: WatchHistoryEntry[] = [];
  let historyQuery = '';
  let historyOffset = 0;
  let historyHasMore = true;
```

Add `WatchHistoryEntry` to the existing `import { drainEngineEvents, type EngineEvent, type SeasonAnimeEntry } from './lib/api';` line:

```js
  import { drainEngineEvents, type EngineEvent, type SeasonAnimeEntry, type WatchHistoryEntry } from './lib/api';
```

Update the `<HistoryView />` render call:

```svelte
    {:else if currentView === 'history'}
      <HistoryView bind:entries={historyEntries} bind:query={historyQuery} bind:offset={historyOffset} bind:hasMore={historyHasMore} />
```

- [ ] **Step 3: Verify `WatchHistoryEntry` is actually exported from api.ts**

Run: `cd next && npm run check`
Expected: no errors. If `WatchHistoryEntry` isn't the exact exported type name, grep `next/src/lib/api.ts` for the real name of `getWatchHistory`'s return element type and use that instead.

- [ ] **Step 4: Manual verification**

`npm run dev`, go to History, search for a title, load a second page via "Load more" (if that control exists — check the rendered markup below line 50 in `HistoryView.svelte` first), navigate away and back — filter, loaded entries, and pagination state should all still be there.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/HistoryView.svelte next/src/App.svelte
git commit -m "fix: persist HistoryView state across back-navigation

Same bug class as SearchView: App.svelte's {#if currentView} chain
destroys HistoryView on nav-away, wiping its filter, loaded pages,
and scroll-relevant offset. Lift the state to App.svelte and bind it
down, same pattern as the SearchView fix."
```

---

### Task 16: Fix DetailView's stale-response race condition and duplicate initial fetch

**Files:**
- Modify: `next/src/lib/DetailView.svelte:78-82,183-297`

**Root cause (two bugs, same area):**
1. `onMount(() => load())` (line 289-291) and the reactive `$: if (animeId) { load(); loadAiring(); }` (line 293-297) both fire on component init — Svelte reactive statements run once during initialization, independent of `onMount` — doubling every network call on first visit.
2. None of `load`/`loadSonarr`/`loadEpisodeFiles`/`loadRelations`/`loadAiring` guard against `animeId` having changed by the time their fetch resolves. Clicking a related-anime row (`DetailView.svelte:744`, same component instance, only `animeId` prop changes) before the previous fetch resolves lets a slow, stale response overwrite the newer anime's page.

- [ ] **Step 1: Remove the redundant `onMount`**

In `next/src/lib/DetailView.svelte`, delete lines 289-291:

```js
  onMount(() => {
    load();
  });
```

(The reactive `$: if (animeId) { load(); loadAiring(); }` immediately below already runs on init, so this becomes the sole trigger — for both initial mount and subsequent `animeId` prop changes.)

- [ ] **Step 2: Add a stale-response guard to `load`**

Replace `load` at lines 183-199:

```js
  async function load() {
    const requestedId = animeId;
    loading = true;
    error = null;
    saveOk = null;
    try {
      const d = await fetchAnimeDetail(requestedId);
      if (requestedId !== animeId) return; // a newer anime is now showing
      detail = d;
      setDraftsFromDetail(d);
      loadSonarr();
      loadEpisodeFiles();
      loadRelations();
    } catch (e) {
      if (requestedId !== animeId) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (requestedId === animeId) loading = false;
    }
  }
```

- [ ] **Step 3: Add the same guard to `loadSonarr`, `loadEpisodeFiles`, `loadRelations`**

Replace lines 201-224:

```js
  async function loadSonarr() {
    const requestedId = animeId;
    sonarrLoading = true;
    try {
      const avail = await getSonarrAvailability(requestedId);
      if (requestedId !== animeId) return;
      sonarrAvail = avail;
    } catch {
      if (requestedId === animeId) sonarrAvail = null;
    } finally {
      if (requestedId === animeId) sonarrLoading = false;
    }
  }

  async function loadEpisodeFiles() {
    const requestedId = animeId;
    episodeFilesLoading = true;
    try {
      const files = await getEpisodeFiles(requestedId);
      if (requestedId !== animeId) return;
      episodeFiles = files;
    } catch {
      if (requestedId === animeId) episodeFiles = [];
    } finally {
      if (requestedId === animeId) episodeFilesLoading = false;
    }
  }

  async function loadRelations() {
    const requestedId = animeId;
    relationsLoading = true;
    try {
      const r = await getAnimeRelations(requestedId);
      if (requestedId !== animeId) return;
      relations = r;
    } catch {
      if (requestedId === animeId) relations = [];
    } finally {
      if (requestedId === animeId) relationsLoading = false;
    }
  }
```

- [ ] **Step 4: Add the same guard to `loadAiring`**

Replace lines 78-82:

```js
  async function loadAiring() {
    const requestedId = animeId;
    nextAiring = null;
    try {
      const na = await getNextAiring(requestedId);
      if (requestedId === animeId) nextAiring = na;
    } catch {
      if (requestedId === animeId) nextAiring = null;
    }
  }
```

- [ ] **Step 5: Verify it typechecks**

Run: `cd next && npm run check`
Expected: no new errors.

- [ ] **Step 6: Manual verification**

`npm run dev`, open an anime's detail page, immediately click a related-anime link before it finishes loading, repeat a few times with different network conditions (throttle in devtools if needed) — the page should always end up showing data for the anime currently selected, never a flash of a previous anime's stale data. Also confirm (via the Network tab) that opening a detail page fires each fetch (`fetch_anime_detail`, `get_sonarr_availability`, `get_episode_files`, `get_anime_relations`) exactly once, not twice.

- [ ] **Step 7: Commit**

```bash
git add next/src/lib/DetailView.svelte
git commit -m "fix: stop DetailView double-fetching and racing on rapid navigation

onMount and the \$: if (animeId) reactive block both fired on init,
doubling every request. None of the load* functions guarded against
animeId changing before their fetch resolved, so clicking a related
anime before the previous page finished loading could let a stale
response overwrite the new page."
```

---

### Task 17: Tighten `runMigration`'s strategy type; document `restoreDatabase`'s restart behavior

**Files:**
- Modify: `next/src/lib/api.ts:242-252`
- Modify: `next/src/lib/SettingsView.svelte:112`

**Root cause:**
1. `runMigration(strategy: string, ...)` accepts any string, but the backend enum only accepts `"Skip"`/`"Merge"` — a typo fails opaquely at the IPC layer instead of at compile time.
2. `restore_database`'s backend command calls `app.restart()` immediately after a successful restore (`commands.rs:1391`), so `restoreDatabase`'s success branch can never actually be observed by the caller in real usage — worth documenting so nobody writes follow-up logic assuming it runs.

- [ ] **Step 1: Tighten the `runMigration` type**

In `next/src/lib/api.ts`, replace line 242:

```ts
export function runMigration(strategy: 'Skip' | 'Merge', invokeFn: InvokeFn = tauriInvoke): Promise<MigrationReport> {
  return invokeFn<MigrationReport>('run_migration', { strategy });
}
```

- [ ] **Step 2: Update the caller's type to match**

In `next/src/lib/SettingsView.svelte`, replace line 112:

```js
  let migrationStrategy: 'Skip' | 'Merge' = 'Skip';
```

- [ ] **Step 3: Document `restoreDatabase`'s restart behavior**

In `next/src/lib/api.ts`, replace lines 250-252:

```ts
// The backend restarts the app immediately after a successful restore
// (see commands.rs's restore_database), so this promise's success branch
// is never actually observed by the caller in practice — only rejection
// (a validation or file-system error before the restart) is. Don't add
// .then() logic here expecting to run after a successful restore.
export function restoreDatabase(backupPath: string, invokeFn: InvokeFn = tauriInvoke): Promise<string> {
  return invokeFn<string>('restore_database', { backupPath });
}
```

- [ ] **Step 4: Verify it typechecks**

Run: `cd next && npm run check`
Expected: no new errors — `migrationStrategy` is only ever assigned `'Skip'`/`'Merge'` via the two `<option value="...">` in `SettingsView.svelte:816-817`, both literal matches.

- [ ] **Step 5: Update and run the existing test**

Check `next/src/lib/api.test.ts:75-80`'s `runs migration through invoke` test still compiles against the tightened type (it already calls `runMigration('Skip', invoke)`, a literal that satisfies `'Skip' | 'Merge'`, so no test changes should be needed — confirm by running).

Run: `cd next && npm test -- api.test`
Expected: all PASS, no type errors.

- [ ] **Step 6: Commit**

```bash
git add next/src/lib/api.ts next/src/lib/SettingsView.svelte
git commit -m "fix: type runMigration's strategy as 'Skip' | 'Merge'

The backend enum only accepts these two exact strings; a typo
previously compiled fine in TS and failed opaquely at the Tauri IPC
layer at runtime. Also document that restoreDatabase's success branch
is unobservable in practice since the backend restarts immediately."
```

---

### Task 18: Add test coverage for a representative sample of untested `api.ts` functions

**Files:**
- Modify: `next/src/lib/api.test.ts`

**Root cause:** Roughly half of `api.ts`'s ~65 exports have zero test coverage, including functions with real logic beyond a passthrough call. Full coverage of all of them is out of scope for this plan (see Global Constraints); this task covers the ones with the most "real logic" risk — exactly the kind of function where a silent param-name mismatch with the Rust side would only surface at runtime.

**Interfaces:** No production code changes — test-only.

- [ ] **Step 1: Read `getWatchHistory`'s current implementation**

Read `next/src/lib/api.ts` around `getWatchHistory` (grep for `export function getWatchHistory` first) to get its exact parameter names/defaults and the Rust command name it calls, so the test asserts the real invoke args rather than guessed ones.

- [ ] **Step 2: Write the test for `getWatchHistory`**

Add to `next/src/lib/api.test.ts` (add `getWatchHistory` and any other functions used below to the existing `import { ... } from './api';` block at the top):

```ts
  it('gets watch history through invoke with default paging', async () => {
    const entries = [{ id: 1, anime_id: 2, anime_title: 'Test', episode: 1, file_path: null, player: null, watched_at: 1000, source: 'manual' }];
    const invoke = vi.fn().mockResolvedValue(entries);
    await expect(getWatchHistory(undefined, 50, 0, invoke)).resolves.toEqual(entries);
    expect(invoke).toHaveBeenCalledWith('get_watch_history', { query: undefined, limit: 50, offset: 0 });
  });
```

(If Step 1 reveals different parameter names/order or different nullish-coalescing defaults than assumed here, adjust the test body to match the real signature exactly — do not guess without checking.)

- [ ] **Step 3: Write the test for `mapFolderToAnime`**

Read its signature in `api.ts` first (grep `export function mapFolderToAnime`), then add:

```ts
  it('maps a folder to an anime through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(12);
    await expect(mapFolderToAnime('D:/Anime/Show', 42, invoke)).resolves.toBe(12);
    expect(invoke).toHaveBeenCalledWith('map_folder_to_anime', { folder: 'D:/Anime/Show', animeId: 42 });
  });
```

- [ ] **Step 4: Write the test for `deepMatchViaAnilist`**

Read its signature first (grep `export function deepMatchViaAnilist`), then add a test following the same `vi.fn().mockResolvedValue(...)` / `toHaveBeenCalledWith(...)` pattern used throughout the file, using its real return shape (check the `DeepMatchReport`-equivalent TS type exported near it).

- [ ] **Step 5: Write the test for `setKnownFileMappings`**

Read its signature first (grep `export function setKnownFileMappings`), then add a test covering the nested `FileMappingInput[]` payload shape it sends, since this was specifically flagged as a case where nested struct field names (unlike top-level command params) are NOT auto-camelCase-converted by Tauri — the test should assert the exact snake_case field names sent.

- [ ] **Step 6: Run the new tests**

Run: `cd next && npm test -- api.test`
Expected: all PASS, including the four new tests and every pre-existing test in the file.

- [ ] **Step 7: Commit**

```bash
git add next/src/lib/api.test.ts
git commit -m "test: add coverage for getWatchHistory, mapFolderToAnime, deepMatchViaAnilist, setKnownFileMappings

These were among api.ts's untested exports with real parameter-shape
logic — exactly where a silent Tauri param-name mismatch would only
surface at runtime. Full coverage of every untested export is a
larger, separate effort (see plan's Global Constraints)."
```

---

## Self-Review Notes

- **Spec coverage:** All 20 numbered findings from the review are addressed by Tasks 1-18, except the six items explicitly listed as deferred in Global Constraints (poster-grid dedup, storage.rs clear-match dedup, oversized command handlers, api.ts error-normalization layer, anilist/sonarr client error-boilerplate dedup, getSetting runtime validation, and full api.ts test coverage) — each deferred with a stated reason (regression risk vs. value, no test harness, or disproportionate scope).
- **File overlap / execution order:** `commands.rs` is touched by Tasks 4 and 5 (disjoint line ranges); `storage.rs` by Tasks 6 and 9 (disjoint); `library_scanner.rs` by Tasks 1 and 7 (disjoint); `oauth.rs`/`Cargo.toml` by Task 2 only (Task 8 only touches `anilist/client.rs` and `sonarr/client.rs`, not `oauth.rs`, since Task 2's full-file rewrite already includes the timeout). Given this overlap, **run these tasks sequentially in one session rather than dispatching them to independent parallel subagents** — a subagent-per-task on files with disjoint-but-nearby line ranges risks stale-diff conflicts even though the changes themselves don't conflict semantically.
- **Type consistency:** `find_video_files(dir, files, errors)`'s public signature is unchanged by Task 7, so Task 4's call site needs no follow-up edit. `get_file_index_by_filename`'s `Option<FileIndexRow>` return type is unchanged by Task 6. `EventBus::publish`/`drain` signatures unchanged by Task 10. Confirmed no task introduces a signature a later task assumes differently.
