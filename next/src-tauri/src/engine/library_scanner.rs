use crate::engine::parser::parse_filename;
use crate::engine::storage::Storage;

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
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    for folder in &folders {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !VIDEO_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }

            found += 1;
            let file_path = path.to_string_lossy().to_string();

            // Skip if already indexed
            if storage.get_file_index(&file_path).await?.is_some() {
                continue;
            }

            // Parse filename to find episode info
            let parsed = parse_filename(&file_path, None);
            let episode = parsed.as_ref().and_then(|p| {
                if p.episode_number > 0 {
                    Some(p.episode_number)
                } else {
                    None
                }
            });

            // Try to match to an anime via title search
            let anime_id = if let Some(ref p) = parsed {
                let candidates = storage.search_anime_by_title(&p.cleaned_title, 3).await?;
                candidates.first().map(|c| c.id)
            } else {
                None
            };

            storage
                .upsert_file_index(
                    &file_path,
                    anime_id.unwrap_or(0),
                    episode.unwrap_or(0),
                    if anime_id.is_some() { 60 } else { 0 },
                    now,
                )
                .await?;

            indexed += 1;
        }
    }

    Ok(LibraryScanReport { found, indexed })
}

/// Get all episode files for a specific anime, ordered by episode number.
pub async fn get_episode_files(
    storage: &Storage,
    anime_id: i64,
) -> anyhow::Result<Vec<crate::engine::storage::FileIndexRow>> {
    storage.file_index_by_anime(anime_id).await
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LibraryScanReport {
    pub found: i64,
    pub indexed: i64,
}

/// Open a file with the default system application (plays video files).
pub fn open_file(path: &str) -> anyhow::Result<()> {
    std::process::Command::new("cmd")
        .args(["/c", "start", "", path])
        .spawn()?;
    Ok(())
}
