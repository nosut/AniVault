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
    // Search and slice the same lowercased string: lowercasing can change byte
    // lengths for some Unicode characters, so an offset found in the lowercased
    // text must never be used to index the original.
    let text = (window_title.unwrap_or("").to_string() + " " + file_path.unwrap_or(""))
        .to_lowercase();

    // Simple heuristic: find " - " or " S01E" or " EP" patterns
    for pattern in &[" - ", " s01e", " ep", " episode "] {
        if let Some(pos) = text.find(pattern) {
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
    let fp = result.file_path.as_deref().unwrap_or("");
    let recognition = recognize_file(fp, result.window_title.as_deref(), &state.storage).await?;

    // Auto-confirm without user interaction when the match is confident: a file
    // already mapped (known_file), or a fresh candidate at/above the threshold.
    const AUTO_CONFIRM_THRESHOLD: u8 = 80;
    let confident = recognition
        .candidates
        .first()
        .filter(|c| recognition.known_file || c.confidence >= AUTO_CONFIRM_THRESHOLD)
        .map(|c| (c.anime_id, c.confidence));

    if let Some((anime_id, confidence)) = confident {
        // Episode number: prefer the parsed value, else parse the filename/title.
        let episode = recognition
            .parsed
            .as_ref()
            .map(|p| p.episode_number)
            .filter(|e| *e > 0)
            .or_else(|| {
                let name = std::path::Path::new(fp)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .or(result.window_title.as_deref())
                    .unwrap_or(fp);
                crate::engine::parser::parse_filename(name, None)
                    .map(|p| p.episode_number)
                    .filter(|e| *e > 0)
            })
            .unwrap_or(0);

        if episode > 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            // Remember a fresh (not-yet-known) file so it's known next time — but
            // only if `fp` is a real path. mpv/VLC provide only the window title,
            // which must never be stored as a file key (it would shadow the real
            // mapping and pin progress to the wrong entry).
            if !recognition.known_file && crate::engine::matcher::looks_like_path(fp) {
                let _ = state
                    .storage
                    .upsert_file_index(fp, Some(anime_id), episode, confidence as i32, now)
                    .await;
            }

            state.events.publish(EngineEvent::AnimeIdentified(AnimeIdentified {
                anime_id,
                episode,
                confidence,
                evidence: format!("auto match: {fp}"),
            }));

            let old_episode = state
                .storage
                .get_list_entry(anime_id)
                .await
                .ok()
                .flatten()
                .map(|e| e.watched_episodes)
                .unwrap_or(0);

            if episode > old_episode {
                let _ = state
                    .storage
                    .upsert_list_entry_progress(anime_id, "watching", episode, now)
                    .await;
                // Record the watch in history, mirroring the manual mark path.
                // The `episode > old_episode` guard means this fires once per
                // newly-watched episode, not once per scan tick.
                let hist_path = if crate::engine::matcher::looks_like_path(fp) {
                    Some(fp)
                } else {
                    None
                };
                let _ = state
                    .storage
                    .append_watch_history(
                        anime_id,
                        episode,
                        hist_path,
                        Some(result.player_name.as_str()),
                        now,
                    )
                    .await;
                // Auto-complete when playback reaches the episode cap.
                let _ = state.storage.auto_complete_if_capped(anime_id).await;
                // Push status + progress back to AniList.
                crate::engine::sync_worker::enqueue_anilist_sync(state, anime_id).await;

                state.events.publish(EngineEvent::ProgressAdvanced {
                    anime_id,
                    old_episode,
                    new_episode: episode,
                    source: "auto-detect".to_string(),
                });

                notify_progress(state, anime_id, episode).await;
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

/// Show a desktop toast when playback auto-advances an episode. Respects the
/// pause toggle and is a no-op without an app handle (e.g. in tests).
async fn notify_progress(state: &EngineState, anime_id: i64, episode: i32) {
    use tauri_plugin_notification::NotificationExt;

    if state
        .tracking_paused
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return;
    }
    let handle = match &state.app_handle {
        Some(h) => h,
        None => return,
    };

    let title = state
        .storage
        .anime_detail(anime_id)
        .await
        .ok()
        .and_then(|d| serde_json::from_str::<serde_json::Value>(&d.titles_json).ok())
        .and_then(|v| {
            v.get("english")
                .and_then(|e| e.as_str())
                .filter(|s| !s.is_empty())
                .or_else(|| v.get("romaji").and_then(|r| r.as_str()))
                .map(String::from)
        })
        .unwrap_or_else(|| format!("Anime #{anime_id}"));

    let _ = handle
        .notification()
        .builder()
        .title(format!("▶ {title}"))
        .body(format!("Episode {episode} watched"))
        .show();
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
