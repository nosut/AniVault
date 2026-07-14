use crate::engine::parser::parse_filename;
use crate::engine::storage::Storage;
use std::path::Path;

const LIBRARY_FOLDERS_KEY: &str = "library.folders";

/// Supported video file extensions
const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "avi", "mov", "wmv", "flv", "webm", "m4v"];

/// Get configured library folders from settings
pub async fn get_library_folders(storage: &Storage) -> anyhow::Result<Vec<String>> {
    let raw = storage.get_setting(LIBRARY_FOLDERS_KEY).await?;
    match raw {
        Some(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
        None => Ok(vec![]),
    }
}

/// Save library folders to settings
pub async fn set_library_folders(storage: &Storage, folders: Vec<String>) -> anyhow::Result<()> {
    let json = serde_json::to_string(&folders)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    storage.set_setting(LIBRARY_FOLDERS_KEY, &json, now).await?;
    Ok(())
}

/// Minimum title-match score required to auto-attach a file to an anime.
/// Below this, the file is left unmatched (confidence 0) so it resurfaces for
/// manual mapping instead of asserting a bad guess.
const MATCH_THRESHOLD: u8 = 40;

/// Confidence recorded when a file inherits its anime from unanimous mapped
/// siblings in the same folder — below manual (100) so manual wins on display,
/// well above the title threshold so the scanner treats it as a real match.
const INHERITED_CONFIDENCE: i32 = 85;

/// Best-match result for a single file: (anime_id, confidence, episode).
pub type FileMatch = (Option<i64>, i32, Option<i32>);

/// Match a single file against the local library, scoring every candidate and
/// returning the best above [`MATCH_THRESHOLD`]. Shared by the library scanner
/// and the Re-match command so both rank identically and store real confidence
/// (never a hardcoded value or an unscored first-candidate pick).
pub async fn match_file(storage: &Storage, file_path: &Path) -> anyhow::Result<FileMatch> {
    let file_path_str = file_path.to_string_lossy().to_string();

    // Parse filename to find episode info — use just the filename, not the full path
    let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or(&file_path_str);
    let parsed = parse_filename(file_name, None);
    let episode = parsed.as_ref().and_then(|p| {
        if p.episode_number > 0 {
            Some(p.episode_number)
        } else {
            None
        }
    });

    // Build a set of title queries to try: the parsed filename title, plus the
    // parent ("Season 1") and grandparent (show folder) directory names. The show
    // folder is often the most reliable signal for well-organized libraries.
    let parent = file_path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).map(|s| s.to_string());
    let grandparent = file_path.parent().and_then(|p| p.parent()).and_then(|p| p.file_name()).and_then(|n| n.to_str()).map(|s| s.to_string());

    let mut queries: Vec<String> = Vec::new();
    if let Some(ref p) = parsed {
        if !p.cleaned_title.is_empty() {
            queries.push(p.cleaned_title.clone());
        }
    }
    for dir_name in [grandparent.as_deref(), parent.as_deref()].into_iter().flatten() {
        let cleaned = dir_name
            .replace(['[', ']', '(', ')', '_'], " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !cleaned.is_empty() {
            queries.push(cleaned);
        }
    }

    // Gather candidates across all queries and rank them by real title-match score
    // (shared with the live recognizer) rather than blindly taking the first DB row.
    let mut best_id: Option<i64> = None;
    let mut best_score: u8 = 0;
    for query in &queries {
        let candidates = storage.search_anime_by_title(query, 10).await?;
        for c in &candidates {
            let score = crate::engine::matcher::score_titles_json(query, &c.titles_json);
            if score > best_score {
                best_score = score;
                best_id = Some(c.id);
            }
        }
    }

    // If all searches fail, try word-wildcard matching (handles punctuation differences)
    if best_id.is_none() {
        for query in &queries {
            let cleaned: String = query.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
            let words: Vec<&str> = cleaned.split_whitespace().filter(|w| w.len() >= 3).collect();
            if words.len() < 2 { continue; }
            if let Ok(candidates) = storage.search_anime_by_words(&words, 3).await {
                for c in &candidates {
                    let score = crate::engine::matcher::score_titles_json(query, &c.titles_json);
                    if score > best_score {
                        best_score = score;
                        best_id = Some(c.id);
                    }
                }
            }
        }
    }

    // Require a minimum confidence to auto-attach. Below the threshold, fall
    // back to folder inheritance: if every mapped file in this directory agrees
    // on one anime and we parsed an episode number, adopt that anime — one
    // manual mapping then fixes the whole series. Otherwise leave unmatched (0)
    // so the file resurfaces on the next re-scan.
    let (anime_id, confidence) = if best_score >= MATCH_THRESHOLD {
        (best_id, best_score as i32)
    } else if let (Some(_), Some(dir)) = (
        episode,
        file_path.parent().and_then(|p| p.to_str()).filter(|d| !d.is_empty()),
    ) {
        let prefix = dir_prefix(dir);
        let mut rows = storage.mapped_files_under(&prefix).await?;
        // Never inherit from this file's own (stale) row.
        rows.retain(|(p, _)| p != &file_path_str);
        match unanimous_dir_anime(&rows, &prefix) {
            Some(id) => (Some(id), INHERITED_CONFIDENCE),
            None => (None, 0),
        }
    } else {
        (None, 0)
    };

    tracing::debug!(
        file = %file_name,
        episode = episode.unwrap_or(0),
        queries = ?queries,
        matched_anime_id = ?anime_id,
        score = best_score,
        "file match"
    );

    Ok((anime_id, confidence, episode))
}

/// Distinct parent directories of a set of file paths, in first-seen order.
pub fn parent_dirs(paths: &[String]) -> Vec<String> {
    let mut dirs: Vec<String> = Vec::new();
    for p in paths {
        if let Some(d) = Path::new(p).parent().and_then(|d| d.to_str()) {
            if !d.is_empty() && !dirs.iter().any(|x| x == d) {
                dirs.push(d.to_string());
            }
        }
    }
    dirs
}

/// Re-run matching for the unmatched, non-ignored files under the given
/// directories. Called after a manual mapping so siblings of the newly mapped
/// file inherit it immediately (see `unanimous_dir_anime`) instead of waiting
/// for the next scan. Only rows that gain a match are written; returns how many.
pub async fn rematch_unmatched_in_dirs(
    storage: &Storage,
    dirs: &[String],
) -> anyhow::Result<usize> {
    let now = unix_now();
    let mut updated = 0usize;
    for dir in dirs {
        let prefix = dir_prefix(dir);
        for path in storage.unmatched_files_under(&prefix).await? {
            // Sweep only direct siblings — inheritance is per-directory, and a
            // recursive sweep from a library root could stall the save command.
            match path.strip_prefix(&prefix) {
                Some(rest) if !rest.contains(['\\', '/']) => {}
                _ => continue,
            }
            let (anime_id, confidence, episode) = match_file(storage, Path::new(&path)).await?;
            if anime_id.is_some() {
                storage
                    .upsert_file_index(&path, anime_id, episode.unwrap_or(0), confidence, now)
                    .await?;
                updated += 1;
            }
        }
    }
    Ok(updated)
}

/// Current Unix time in seconds.
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Scan configured library folders for video files.
/// Parses filenames via parser, stores matches in file_index, and prunes rows
/// whose file has been deleted from disk. Returns a per-scan report.
pub async fn scan_library_folders(storage: &Storage) -> anyhow::Result<LibraryScanReport> {
    let folders = get_library_folders(storage).await?;
    scan_dirs(storage, &folders, true).await
}

/// Scan a specific set of directories: index new/changed files and prune
/// deleted ones under each. Used by the filesystem watcher for targeted scans;
/// a directory that no longer exists is silently skipped (never pruned under).
pub async fn scan_specific_dirs(
    storage: &Storage,
    dirs: &[String],
) -> anyhow::Result<LibraryScanReport> {
    scan_dirs(storage, dirs, false).await
}

/// Rescan only the folders that contain a single anime's indexed files — the
/// fast path behind the detail-page "Rescan" button. Picks up episodes added to
/// the show's folders and prunes ones deleted from disk, without walking the
/// whole library. Falls back to a full library scan when the anime has no
/// indexed files yet (so there's no folder to derive).
pub async fn rescan_anime_dirs(
    storage: &Storage,
    anime_id: i64,
) -> anyhow::Result<LibraryScanReport> {
    let rows = storage.file_index_by_anime(anime_id).await?;

    // This anime's *real-path* files (ignore any legacy window-title
    // pseudo-paths that have no directory component).
    let files: Vec<String> = rows
        .iter()
        .filter(|r| crate::engine::matcher::looks_like_path(&r.file_path))
        .map(|r| r.file_path.clone())
        .collect();

    if files.is_empty() {
        return scan_library_folders(storage).await;
    }

    let lib_folders = get_library_folders(storage).await?;
    let now = unix_now();
    let mut report = LibraryScanReport::default();

    // 1. Index new episodes from any of the show's parent folders that still
    //    exist. A deleted folder simply has nothing to add.
    let mut seen_dirs: Vec<String> = Vec::new();
    for f in &files {
        if let Some(parent) = Path::new(f).parent().and_then(|p| p.to_str()) {
            let d = parent.to_string();
            if !d.is_empty() && !seen_dirs.contains(&d) {
                seen_dirs.push(d);
            }
        }
    }
    for dir in &seen_dirs {
        let path = Path::new(dir);
        if path.exists() {
            index_new_files_in_dir(storage, path, now, &mut report).await?;
        }
    }

    // 2. Prune the show's files that are gone from disk. Unlike step 1 this does
    //    NOT require the immediate parent folder to exist — deleting the whole
    //    `Season 1` folder must still clear its episodes. The guard against an
    //    offline drive wiping the index is applied at the *library-root* level:
    //    a file is only pruned when the library folder containing it (or its
    //    drive) is currently reachable.
    let missing: Vec<String> = files
        .into_iter()
        .filter(|p| !Path::new(p).exists() && library_root_online(p, &lib_folders))
        .collect();
    if !missing.is_empty() {
        report.removed += missing.len() as i64;
        storage.delete_file_indexes(&missing).await?;
    }

    tracing::info!(
        anime_id,
        found = report.found,
        indexed = report.indexed,
        removed = report.removed,
        "anime rescan complete"
    );

    Ok(report)
}

/// Is the library storage holding `file_path` currently reachable? Prevents
/// pruning when a whole drive/library folder is offline (every file under it
/// would look "missing"). A file is prunable when the configured library folder
/// containing it exists; for files outside any library folder, when the path's
/// drive root is present.
fn library_root_online(file_path: &str, lib_folders: &[String]) -> bool {
    let mut under_a_library_folder = false;
    for folder in lib_folders {
        if file_path.starts_with(&dir_prefix(folder)) {
            under_a_library_folder = true;
            if Path::new(folder).exists() {
                return true;
            }
        }
    }
    if under_a_library_folder {
        // Under a known library folder, but that folder is offline → don't prune.
        return false;
    }
    // Outside any configured library folder (e.g. a manually-mapped path): fall
    // back to the filesystem/drive root.
    let path = Path::new(file_path);
    match path.components().next() {
        Some(std::path::Component::Prefix(prefix)) => {
            // Windows drive such as "Y:" → check that "Y:\" is mounted.
            let mut root = prefix.as_os_str().to_os_string();
            root.push(std::path::MAIN_SEPARATOR.to_string());
            Path::new(&root).exists()
        }
        _ => path.ancestors().last().map(|r| r.exists()).unwrap_or(false),
    }
}

/// Index the video files directly discoverable under `dir` (recursively) that
/// aren't already indexed with a confident match, accumulating into `report`.
async fn index_new_files_in_dir(
    storage: &Storage,
    dir: &Path,
    now: i64,
    report: &mut LibraryScanReport,
) -> anyhow::Result<()> {
    let mut video_files = Vec::new();
    find_video_files(dir, &mut video_files, &mut report.errors);

    for file_path in &video_files {
        let file_path_str = file_path.to_string_lossy().to_string();
        report.found += 1;

        // Skip if already indexed with a valid match; re-evaluate if unmatched
        // (confidence 0). Ignored files are tombstoned — never re-index them.
        let existing = storage.get_file_index(&file_path_str).await?;
        if let Some(ref ex) = existing {
            if ex.ignored || ex.confidence > 0 {
                report.skipped += 1;
                continue;
            }
        }

        let (anime_id, confidence, episode) = match_file(storage, file_path).await?;

        // An already-indexed unmatched file that stays unmatched isn't a change —
        // don't rewrite it, so `indexed` reports only real changes and periodic
        // auto-scans stay silent when nothing happened.
        if existing.is_some() && anime_id.is_none() {
            report.skipped += 1;
            continue;
        }

        storage
            .upsert_file_index(&file_path_str, anime_id, episode.unwrap_or(0), confidence, now)
            .await?;
        report.indexed += 1;
    }
    Ok(())
}

/// Scan a set of directories: index new/unmatched video files and prune index
/// rows whose file has been deleted from disk.
///
/// A directory that doesn't currently exist (e.g. an offline network drive) is
/// skipped entirely — never scanned and never pruned under — so an unavailable
/// folder can't wipe the index. `report_missing_dirs` records a nonexistent
/// directory as an error (full library scan) versus silently skipping it
/// (targeted rescan, where the show's folder may legitimately be gone).
async fn scan_dirs(
    storage: &Storage,
    dirs: &[String],
    report_missing_dirs: bool,
) -> anyhow::Result<LibraryScanReport> {
    let mut report = LibraryScanReport::default();
    let now = unix_now();

    for dir in dirs {
        let path = Path::new(dir);
        if !path.exists() {
            if report_missing_dirs {
                report.errors.push(format!("Folder not found: {}", dir));
            }
            // Offline/missing directory: do not prune anything under it.
            continue;
        }

        index_new_files_in_dir(storage, path, now, &mut report).await?;

        // Prune rows under this (accessible) directory whose file is gone from disk.
        let prefix = dir_prefix(dir);
        let indexed_paths = storage.file_paths_under(&prefix).await?;
        let missing: Vec<String> = indexed_paths
            .into_iter()
            .filter(|p| !Path::new(p).exists())
            .collect();
        if !missing.is_empty() {
            report.removed += missing.len() as i64;
            storage.delete_file_indexes(&missing).await?;
        }
    }

    tracing::info!(
        found = report.found,
        indexed = report.indexed,
        skipped = report.skipped,
        removed = report.removed,
        errors = report.errors.len(),
        "library scan complete"
    );

    Ok(report)
}

/// A directory path with a trailing separator, for prefix-matching the files
/// indexed beneath it. The separator guards against a folder `Anime` matching a
/// sibling `Anime2` when prefix-matching stored paths.
fn dir_prefix(dir: &str) -> String {
    let trimmed = dir.trim_end_matches(['\\', '/']);
    format!("{trimmed}{}", std::path::MAIN_SEPARATOR)
}

/// If every mapped file directly inside the directory `prefix` (which must end
/// with a path separator) agrees on one anime, return it. Rows in
/// subdirectories are ignored; disagreement or no direct siblings → None.
pub fn unanimous_dir_anime(rows: &[(String, i64)], prefix: &str) -> Option<i64> {
    let mut found: Option<i64> = None;
    for (path, anime_id) in rows {
        let Some(rest) = path.strip_prefix(prefix) else { continue };
        if rest.contains(['\\', '/']) {
            continue; // lives in a subdirectory, not a direct sibling
        }
        match found {
            None => found = Some(*anime_id),
            Some(a) if a == *anime_id => {}
            Some(_) => return None,
        }
    }
    found
}

/// Get all episode files for a specific anime, ordered by episode number.
pub async fn get_episode_files(
    storage: &Storage,
    anime_id: i64,
) -> anyhow::Result<Vec<crate::engine::storage::FileIndexRow>> {
    storage.file_index_by_anime(anime_id).await
}

/// Does this path have a recognized video-file extension?
pub fn is_video_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    VIDEO_EXTENSIONS.contains(&ext.as_str())
}

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

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct LibraryScanReport {
    pub found: i64,
    pub indexed: i64,
    pub skipped: i64,
    /// Rows pruned because their file no longer exists on disk.
    pub removed: i64,
    pub errors: Vec<String>,
}

/// Open a file with the default system application (plays video files).
pub fn open_file(path: &str) -> anyhow::Result<()> {
    open::that(path)?;
    Ok(())
}

/// Open the folder that contains the given file (the episode's folder) in
/// Windows Explorer. Opens the directory directly — `/select` is unreliable
/// because Rust quotes the whole argument, which Explorer then can't parse.
pub fn open_containing_folder(path: &str) -> anyhow::Result<()> {
    let p = std::path::Path::new(path);
    let dir = if p.is_dir() {
        p.to_path_buf()
    } else {
        p.parent().map(|x| x.to_path_buf()).unwrap_or_else(|| p.to_path_buf())
    };
    std::process::Command::new("explorer")
        .arg(dir.as_os_str())
        .spawn()?;
    Ok(())
}
