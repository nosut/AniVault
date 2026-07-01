#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInfo {
    pub process_name: String,
    pub window_title: Option<String>,
}

pub fn is_known_player(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "mpv.exe"
        || lower == "mpvnet.exe"
        || lower == "mpc-hc.exe"
        || lower == "mpc-hc64.exe"
        || lower == "vlc.exe"
        || lower == "potplayermini.exe"
        || lower == "potplayermini64.exe"
        || lower == "microsoft.media.player.exe"
        || lower.contains("potplayer")
}

pub fn scan_players() -> Vec<PlayerInfo> {
    let output = std::process::Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_tasklist_line)
        .filter(|name| is_known_player(name))
        .map(|process_name| PlayerInfo {
            process_name,
            window_title: None,
        })
        .collect()
}

fn parse_tasklist_line(line: &str) -> Option<String> {
    line.split(',')
        .next()
        .map(|name| name.trim_matches('"').trim().to_string())
        .filter(|name| !name.is_empty())
}
