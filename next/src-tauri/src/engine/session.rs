use crate::engine::events::{AnimeIdentified, EngineEvent};
use crate::engine::matcher::recognize_file;
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
    let recognition = recognize_file(
        result.file_path.as_deref().unwrap_or(""),
        result.window_title.as_deref(),
        &state.storage,
    )
    .await?;

    // Auto-confirm high-confidence matches (confidence >= 60)
    if !recognition.known_file {
        if let Some(top) = recognition.candidates.first() {
            if top.confidence >= 60 {
                if let Some(parsed) = &recognition.parsed {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;

                    let fp = result.file_path.as_deref().unwrap_or("");

                    state
                        .storage
                        .upsert_file_index(
                            fp,
                            top.anime_id,
                            parsed.episode_number,
                            top.confidence as i32,
                            now,
                        )
                        .await?;

                    state.events.publish(EngineEvent::AnimeIdentified(
                        AnimeIdentified {
                            anime_id: top.anime_id,
                            episode: parsed.episode_number,
                            confidence: top.confidence,
                            evidence: format!("auto match: {fp}"),
                        },
                    ));

                    // Auto-advance progress if detected episode is ahead of stored
                    let old_episode = state
                        .storage
                        .get_list_entry(top.anime_id)
                        .await
                        .ok()
                        .flatten()
                        .map(|e| e.watched_episodes)
                        .unwrap_or(0);

                    if parsed.episode_number > old_episode {
                        let _ = state
                            .storage
                            .upsert_list_entry_progress(
                                top.anime_id,
                                "Watching",
                                parsed.episode_number,
                                now,
                            )
                            .await;

                        state.events.publish(EngineEvent::ProgressAdvanced {
                            anime_id: top.anime_id,
                            old_episode,
                            new_episode: parsed.episode_number,
                            source: "auto-detect".to_string(),
                        });
                    }
                }
            }
        }
    }

    let episode_guess = recognition
        .parsed
        .as_ref()
        .map(|p| p.episode_number)
        .or_else(|| recognition.candidates.first().map(|_| 0_i32));

    state.events.publish(EngineEvent::PlaybackDetected {
        player_name: result.player_name,
        file_path: result.file_path,
        window_title: result.window_title,
        episode_guess,
        candidates: recognition.candidates,
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
