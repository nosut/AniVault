use crate::engine::events::{AnimeIdentified, EngineEvent};
use crate::engine::runtime::EngineState;
use crate::engine::scanner::ScanResult;

#[derive(Debug, Clone)]
pub struct ActivePlayback {
    pub anime_info: Option<AnimeIdentified>,
    pub last_episode: i32,
    pub started_at: i64,
    pub last_seen_at: i64,
    pub player_name: String,
    pub file_path: Option<String>,
    pub window_title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WatchSession {
    pub active: Option<ActivePlayback>,
}

pub fn guess_episode(file_path: Option<&str>, window_title: Option<&str>) -> Option<i32> {
    let text = window_title.unwrap_or("").to_string()
        + " "
        + file_path.unwrap_or("");

    // Simple heuristic: find " - " or " S01E" or " EP" patterns
    for pattern in &[" - ", " s01e", " ep", " episode "] {
        if let Some(pos) = text.to_lowercase().find(pattern) {
            let after = &text[pos + pattern.len()..];
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(num) = digits.parse::<i32>() {
                if num > 0 && num <= 2000 {
                    return Some(num);
                }
            }
        }
    }
    None
}

pub async fn process_scan_result(state: &EngineState, result: ScanResult) -> anyhow::Result<()> {
    let episode_guess = guess_episode(result.file_path.as_deref(), result.window_title.as_deref());

    state.events.publish(EngineEvent::PlaybackDetected {
        player_name: result.player_name,
        file_path: result.file_path,
        window_title: result.window_title,
        episode_guess,
        detected_at_unix: result.detected_at_unix,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guess_episode_from_window_title_with_dash() {
        assert_eq!(guess_episode(None, Some("Show - 01")), Some(1));
    }

    #[test]
    fn guess_episode_from_window_title_no_match() {
        assert_eq!(guess_episode(None, Some("Show Episode")), None);
    }

    #[test]
    fn guess_episode_from_file_path() {
        assert_eq!(guess_episode(Some("Show - 05.mkv"), None), Some(5));
    }

    #[test]
    fn guess_episode_out_of_range() {
        assert_eq!(guess_episode(None, Some("Show - 9999")), None);
    }

    #[test]
    fn guess_episode_s01e_pattern() {
        assert_eq!(guess_episode(None, Some("[Subs] Show S01E03")), Some(3));
    }
}
