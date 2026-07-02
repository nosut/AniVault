use std::path::PathBuf;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct V1DataPaths {
    pub sqlite_path: Option<String>,
    pub history_xml_path: Option<String>,
    pub anime_xml_path: Option<String>,
    pub list_xml_path: Option<String>,
    pub data_dir: Option<String>,
    pub found: bool,
}

fn candidate_data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Portable: next to exe
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("data"));
        }
    }

    // Installed: %APPDATA%/Taiga/data
    if let Ok(appdata) = std::env::var("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("Taiga").join("data"));
    }

    // Also try %LOCALAPPDATA%
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Taiga").join("data"));
    }

    dirs
}

fn glob_list_xml(v1_user_dir: &PathBuf) -> Option<String> {
    let entries = std::fs::read_dir(v1_user_dir).ok()?;
    for entry in entries {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_dir() {
            let list_xml = path.join("anime.xml");
            if list_xml.exists() {
                return Some(list_xml.to_string_lossy().to_string());
            }
            // Also check nested: user/{name}@{service}/anime.xml
            if let Ok(nested) = std::fs::read_dir(&path) {
                for sub in nested {
                    let sub = sub.ok()?;
                    let sub_path = sub.path();
                    if sub_path.is_dir() {
                        let xml = sub_path.join("anime.xml");
                        if xml.exists() {
                            return Some(xml.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn discover_v1_data() -> V1DataPaths {
    for dir in candidate_data_dirs() {
        let sqlite = dir.join("media.sqlite");
        if sqlite.exists() {
            let history = dir.join("history.xml");
            let anime_xml = dir.join("v1").join("db").join("anime.xml");
            let v1_user = dir.join("v1").join("user");
            let list_xml = glob_list_xml(&v1_user);

            return V1DataPaths {
                sqlite_path: Some(sqlite.to_string_lossy().to_string()),
                history_xml_path: history
                    .exists()
                    .then(|| history.to_string_lossy().to_string()),
                anime_xml_path: anime_xml
                    .exists()
                    .then(|| anime_xml.to_string_lossy().to_string()),
                list_xml_path: list_xml,
                data_dir: Some(dir.to_string_lossy().to_string()),
                found: true,
            };
        }
    }
    V1DataPaths::default()
}
