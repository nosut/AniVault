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
    let mut cmd = std::process::Command::new("tasklist");
    cmd.args(["/V", "/FO", "CSV", "/NH"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let Ok(output) = cmd.output() else {
        return Vec::new();
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(parse_tasklist_line)
        .filter(|info| is_known_player(&info.process_name))
        .collect()
}

fn parse_tasklist_line(line: &str) -> Option<PlayerInfo> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.is_empty() {
        return None;
    }
    let process_name = parts.first()?.trim_matches('"').trim().to_string();
    if process_name.is_empty() {
        return None;
    }
    let window_title = parts.get(8)
        .map(|s| s.trim_matches('"').trim().to_string())
        .filter(|s| !s.is_empty() && s != "N/A");
    Some(PlayerInfo { process_name, window_title })
}
