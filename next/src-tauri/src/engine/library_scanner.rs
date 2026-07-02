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

/// Scan configured folders for video files.
/// Parses filenames via parser, stores in file_index.
/// Returns number of files found and indexed.
pub async fn scan_library_folders(storage: &Storage) -> anyhow::Result<LibraryScanReport> {
    let folders = get_library_folders(storage).await?;
    let mut found = 0;
    let mut indexed = 0;
    let mut skip_count = 0i64;
    let mut error_msgs: Vec<String> = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    for folder in &folders {
        let mut video_files = Vec::new();
        let path = Path::new(folder);
        if !path.exists() {
            error_msgs.push(format!("Folder not found: {}", folder));
            continue;
        }
        find_video_files(path, &mut video_files, &mut error_msgs);

        for file_path in &video_files {
            let file_path_str = file_path.to_string_lossy().to_string();
            found += 1;

            // Skip if already indexed
            if storage.get_file_index(&file_path_str).await?.is_some() {
                skip_count += 1;
                continue;
            }

            // Parse filename to find episode info
            let parsed = parse_filename(&file_path_str, None);
            let episode = parsed.as_ref().and_then(|p| {
                if p.episode_number > 0 {
                    Some(p.episode_number)
                } else {
                    None
                }
            });

            // Try to match to an anime via title search
            let mut anime_id = if let Some(ref p) = parsed {
                let candidates = storage.search_anime_by_title(&p.cleaned_title, 3).await?;
                candidates.first().map(|c| c.id)
            } else {
                None
            };

            // If no match from filename, try parent directory names
            if anime_id.is_none() {
                let parent = file_path.parent().map(|p| p.to_string_lossy().to_string());
                if let Some(ref parent_dir) = parent {
                    let parsed_dir = parse_filename(parent_dir, None);
                    if let Some(ref p) = parsed_dir {
                        if !p.cleaned_title.is_empty() {
                            let candidates = storage.search_anime_by_title(&p.cleaned_title, 3).await?;
                            anime_id = candidates.first().map(|c| c.id);
                        }
                    }
                }
            }

            storage
                .upsert_file_index(
                    &file_path_str,
                    anime_id.unwrap_or(0),
                    episode.unwrap_or(0),
                    if anime_id.is_some() { 60 } else { 0 },
                    now,
                )
                .await?;

            indexed += 1;
        }
    }

    Ok(LibraryScanReport { found, indexed, skipped: skip_count, errors: error_msgs })
}

/// Get all episode files for a specific anime, ordered by episode number.
pub async fn get_episode_files(
    storage: &Storage,
    anime_id: i64,
) -> anyhow::Result<Vec<crate::engine::storage::FileIndexRow>> {
    storage.file_index_by_anime(anime_id).await
}

/// Recursively find video files under a directory.
/// Collects errors for unreadable directories instead of silently skipping.
fn find_video_files(dir: &Path, files: &mut Vec<std::path::PathBuf>, errors: &mut Vec<String>) {
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    find_video_files(&path, files, errors);
                } else if path.is_file() {
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if VIDEO_EXTENSIONS.contains(&ext.as_str()) {
                        files.push(path);
                    }
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct LibraryScanReport {
    pub found: i64,
    pub indexed: i64,
    pub skipped: i64,
    pub errors: Vec<String>,
}

/// Open a file with the default system application (plays video files).
pub fn open_file(path: &str) -> anyhow::Result<()> {
    std::process::Command::new("cmd")
        .args(["/c", "start", "", path])
        .spawn()?;
    Ok(())
}
