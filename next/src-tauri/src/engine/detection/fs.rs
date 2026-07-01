use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCandidate {
    pub path: String,
    pub modified_at_unix: i64,
}

pub fn scan_media_files(folders: &[String], known: &mut HashMap<PathBuf, SystemTime>) -> Vec<FileCandidate> {
    let mut candidates = Vec::new();
    for folder in folders {
        scan_folder(Path::new(folder), known, &mut candidates);
    }
    candidates
}

fn scan_folder(path: &Path, known: &mut HashMap<PathBuf, SystemTime>, candidates: &mut Vec<FileCandidate>) {
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_folder(&path, known, candidates);
            continue;
        }
        if !is_media_file(&path) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };

        let is_new_or_changed = known.get(&path).is_none_or(|last| *last != modified);
        known.insert(path.clone(), modified);

        if is_new_or_changed {
            candidates.push(FileCandidate {
                path: path.to_string_lossy().to_string(),
                modified_at_unix: modified
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            });
        }
    }
}

fn is_media_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "mkv" | "mp4" | "avi"))
        .unwrap_or(false)
}
