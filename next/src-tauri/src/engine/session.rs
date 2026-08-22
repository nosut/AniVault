use crate::engine::events::{AnimeIdentified, EngineEvent};
use crate::engine::matcher::recognize_file;
use crate::engine::runtime::EngineState;
use crate::engine::scanner::ScanResult;
use crate::engine::storage::MappingSource;

/// Identity of a playback session: which library episode is on screen.
///
/// Identity is `anime_id` + `episode` only — compare with [`same_session`],
/// never with `==`, so a field added here cannot silently join the identity.
///
/// `file_key` is carried metadata, not identity. The scanner has no real media
/// path to give: `scan_active_players` reports the window title as `file_path`
/// (falling back to the player executable), so this is in practice the window
/// title. Player titles mutate mid-playback — a position, a percentage, a
/// paused marker, a playlist index — and treating that churn as a new session
/// would end every session after one tick, leaving `watched_secs` at ~0 and the
/// minimum-watch gate rejecting every one of them.
#[derive(Debug, Clone)]
pub struct SessionKey {
    pub anime_id: i64,
    pub episode: i32,
    pub file_key: String,
}

/// Whether two keys name the same watch session: same anime, same episode.
///
/// Two different files of the same anime and episode collapse into one session,
/// which is harmless — the prompt only ever needs the anime and the episode.
pub fn same_session(a: &SessionKey, b: &SessionKey) -> bool {
    a.anime_id == b.anime_id && a.episode == b.episode
}

/// A playback session the tracker is currently observing.
#[derive(Debug, Clone)]
pub struct ActivePlayback {
    pub key: SessionKey,
    pub started_at: i64,
    pub last_seen_at: i64,
    pub missed_ticks: u8,
}

/// What a single tracker tick saw.
///
/// The two "nothing playing" cases are deliberately distinct. [`Unidentified`]
/// is a blip worth waiting out; [`NoPlayer`] is a fact worth acting on. Folding
/// them together is what used to make the Up Next prompt arrive seconds after
/// the player window disappeared: closing the player was made to serve out the
/// same grace window as a momentarily unreadable title.
///
/// [`Unidentified`]: Observation::Unidentified
/// [`NoPlayer`]: Observation::NoPlayer
#[derive(Debug, Clone)]
pub enum Observation {
    /// A library episode was identified on screen.
    Playing(SessionKey),
    /// A known media player is running, but this tick could not say what it is
    /// showing — a window title briefly reduced to the app name during a seek,
    /// a title that failed to match the library, or a scan that could not read
    /// the process list at all.
    Unidentified,
    /// No known media player is running. Only reported when the process list
    /// was actually read, so this genuinely means the player is gone.
    NoPlayer,
}

/// A session that just stopped being observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndedPlayback {
    pub anime_id: i64,
    pub episode: i32,
    pub file_key: String,
    pub watched_secs: i64,
}

impl ActivePlayback {
    fn end(&self) -> EndedPlayback {
        EndedPlayback {
            anime_id: self.key.anime_id,
            episode: self.key.episode,
            file_key: self.key.file_key.clone(),
            watched_secs: self.last_seen_at - self.started_at,
        }
    }
}

/// Advance the watch session by one tracker tick. Pure: no clock, no storage.
///
/// `observed` is what this tick saw (see [`Observation`]). A session survives up
/// to `grace_ticks` consecutive [`Unidentified`] ticks, so a tick that fails to
/// recognise a player that is still running — a title briefly reduced to the app
/// name during a seek, say — does not end it; `watched_secs` is measured to the
/// last tick the episode was actually seen, so grace never inflates it.
///
/// [`NoPlayer`] skips grace entirely: the player process is gone, so there is no
/// blip to wait out and waiting would buy nothing but latency before the Up Next
/// prompt. Returns the session that just ended, if any.
///
/// [`Unidentified`]: Observation::Unidentified
/// [`NoPlayer`]: Observation::NoPlayer
pub fn advance_session(
    session: &mut Option<ActivePlayback>,
    observed: Observation,
    now: i64,
    grace_ticks: u8,
) -> Option<EndedPlayback> {
    let key = match observed {
        Observation::Playing(key) => key,
        Observation::Unidentified => {
            let active = session.as_mut()?;
            active.missed_ticks = active.missed_ticks.saturating_add(1);
            if active.missed_ticks <= grace_ticks {
                return None;
            }
            return session.take().as_ref().map(ActivePlayback::end);
        }
        Observation::NoPlayer => return session.take().as_ref().map(ActivePlayback::end),
    };

    if let Some(active) = session.as_mut() {
        if same_session(&active.key, &key) {
            // The first-seen `file_key` is kept: it is only carried metadata, and
            // player window titles churn while the same episode keeps playing.
            active.last_seen_at = now;
            active.missed_ticks = 0;
            return None;
        }
    }

    let ended = session.take().as_ref().map(ActivePlayback::end);
    *session = Some(ActivePlayback {
        key,
        started_at: now,
        last_seen_at: now,
        missed_ticks: 0,
    });
    ended
}

/// Whether a finished session outlasted the configured minimum. `0` always passes.
pub fn passes_min_watch(watched_secs: i64, min_minutes: i64) -> bool {
    watched_secs >= min_minutes.max(0) * 60
}

/// Whether an ended session was immediately superseded by the session that
/// replaced it — the player advancing itself through a playlist or folder queue.
///
/// `advance_session` ends one session and opens the next in a single call, so
/// when a player rolls from episode 5 to episode 6 the tracker learns that
/// episode 5 ended while episode 6 is already on screen. Offering episode 6 then
/// is pointless: the user is watching it, and the prompt's Play button would
/// launch a second player.
///
/// Only a *later* episode of the *same* anime counts. Playback simply stopping
/// (`next` is `None`), switching to a different show, or going back to an
/// earlier episode all still deserve a prompt.
pub fn superseded_by(ended: &EndedPlayback, next: Option<&ActivePlayback>) -> bool {
    match next {
        Some(next) => next.key.anime_id == ended.anime_id && next.key.episode > ended.episode,
        None => false,
    }
}

pub fn guess_episode(file_path: Option<&str>, window_title: Option<&str>) -> Option<i32> {
    // Search and slice the same lowercased string: lowercasing can change byte
    // lengths for some Unicode characters, so an offset found in the lowercased
    // text must never be used to index the original.
    let text =
        (window_title.unwrap_or("").to_string() + " " + file_path.unwrap_or("")).to_lowercase();

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

pub async fn process_scan_result(
    state: &EngineState,
    result: ScanResult,
) -> anyhow::Result<Option<SessionKey>> {
    let fp = result.file_path.as_deref().unwrap_or("");
    let recognition = recognize_file(fp, result.window_title.as_deref(), &state.storage).await?;

    let mut session_key: Option<SessionKey> = None;

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

            // Metadata only — the session's identity is the anime and episode
            // (see `SessionKey`). The scanner reports the window title here, so
            // this is a title rather than a path in practice.
            let file_key = result
                .file_path
                .clone()
                .or_else(|| result.window_title.clone())
                .unwrap_or_default();
            session_key = Some(SessionKey {
                anime_id,
                episode,
                file_key,
            });

            // Remember a fresh (not-yet-known) file so it's known next time — but
            // only if `fp` is a real path. mpv/VLC provide only the window title,
            // which must never be stored as a file key (it would shadow the real
            // mapping and pin progress to the wrong entry).
            if !recognition.known_file && crate::engine::matcher::looks_like_path(fp) {
                let _ = state
                    .storage
                    .upsert_file_index(
                        fp,
                        Some(anime_id),
                        episode,
                        confidence as i32,
                        MappingSource::Automatic,
                        now,
                    )
                    .await;
            }

            state
                .events
                .publish(EngineEvent::AnimeIdentified(AnimeIdentified {
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
                        "auto-detect",
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

    Ok(session_key)
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
                .or_else(|| {
                    v.get("english_derived")
                        .and_then(|e| e.as_str())
                        .filter(|s| !s.is_empty())
                })
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

    fn key(anime_id: i64, episode: i32) -> SessionKey {
        SessionKey {
            anime_id,
            episode,
            file_key: format!("D:/Anime/a{anime_id}-e{episode}.mkv"),
        }
    }

    fn playing(anime_id: i64, episode: i32) -> Observation {
        Observation::Playing(key(anime_id, episode))
    }

    #[test]
    fn same_key_across_ticks_does_not_end_the_session() {
        let mut session = None;
        assert_eq!(advance_session(&mut session, playing(1, 5), 100, 2), None);
        assert_eq!(advance_session(&mut session, playing(1, 5), 102, 2), None);
        let active = session.expect("session stays open");
        assert_eq!(active.started_at, 100);
        assert_eq!(active.last_seen_at, 102);
    }

    #[test]
    fn key_change_ends_the_previous_session() {
        let mut session = None;
        advance_session(&mut session, playing(1, 5), 100, 2);
        advance_session(&mut session, playing(1, 5), 400, 2);
        let ended = advance_session(&mut session, playing(1, 6), 402, 2).expect("previous ended");
        assert_eq!(ended.anime_id, 1);
        assert_eq!(ended.episode, 5);
        assert_eq!(ended.watched_secs, 300);
        assert!(same_session(
            &session.expect("new session opened").key,
            &key(1, 6)
        ));
    }

    #[test]
    fn a_changed_file_key_alone_is_still_the_same_session() {
        // Player window titles churn mid-playback (elapsed time, percentage, a
        // paused marker). That must not restart the session, or every session
        // would last a single tick and never clear the minimum-watch gate.
        let titled = |title: &str| {
            Observation::Playing(SessionKey {
                anime_id: 1,
                episode: 5,
                file_key: title.to_string(),
            })
        };
        let mut session = None;
        advance_session(&mut session, titled("Show - 05 [00:01:00]"), 100, 2);
        assert_eq!(
            advance_session(&mut session, titled("Show - 05 [00:20:00]"), 1300, 2),
            None,
            "a title change with the same anime and episode is not a new session"
        );
        let active = session.expect("session stays open");
        assert_eq!(active.started_at, 100);
        assert_eq!(active.last_seen_at, 1300);
        assert_eq!(
            active.key.file_key, "Show - 05 [00:01:00]",
            "the first-seen key is kept as metadata"
        );
    }

    fn active(anime_id: i64, episode: i32) -> ActivePlayback {
        ActivePlayback {
            key: key(anime_id, episode),
            started_at: 0,
            last_seen_at: 0,
            missed_ticks: 0,
        }
    }

    fn ended(anime_id: i64, episode: i32) -> EndedPlayback {
        EndedPlayback {
            anime_id,
            episode,
            file_key: String::new(),
            watched_secs: 1800,
        }
    }

    #[test]
    fn a_playlist_advance_supersedes_the_session_it_ended() {
        assert!(superseded_by(&ended(1, 5), Some(&active(1, 6))));
        assert!(
            superseded_by(&ended(1, 5), Some(&active(1, 9))),
            "a jump further ahead in the same show is still an advance"
        );
    }

    #[test]
    fn playback_stopping_is_not_superseded() {
        assert!(!superseded_by(&ended(1, 5), None));
    }

    #[test]
    fn switching_to_another_anime_is_not_superseded() {
        assert!(!superseded_by(&ended(1, 5), Some(&active(2, 6))));
        assert!(!superseded_by(&ended(1, 5), Some(&active(2, 1))));
    }

    #[test]
    fn going_back_to_an_earlier_episode_is_not_superseded() {
        assert!(!superseded_by(&ended(1, 5), Some(&active(1, 4))));
        assert!(
            !superseded_by(&ended(1, 5), Some(&active(1, 5))),
            "the same episode again is a replay, not an advance"
        );
    }

    #[test]
    fn one_missed_tick_within_grace_keeps_the_session() {
        let mut session = None;
        advance_session(&mut session, playing(1, 5), 100, 2);
        assert_eq!(
            advance_session(&mut session, Observation::Unidentified, 102, 2),
            None
        );
        assert_eq!(
            advance_session(&mut session, Observation::Unidentified, 104, 2),
            None
        );
        assert!(session.is_some(), "two misses is still within a grace of 2");
    }

    #[test]
    fn misses_beyond_grace_end_the_session_excluding_grace_time() {
        let mut session = None;
        advance_session(&mut session, playing(1, 5), 100, 2);
        advance_session(&mut session, playing(1, 5), 400, 2);
        advance_session(&mut session, Observation::Unidentified, 402, 2);
        advance_session(&mut session, Observation::Unidentified, 404, 2);
        let ended = advance_session(&mut session, Observation::Unidentified, 406, 2)
            .expect("grace exhausted");
        assert_eq!(ended.episode, 5);
        assert_eq!(
            ended.watched_secs, 300,
            "grace ticks must not inflate the watched time"
        );
        assert!(session.is_none(), "session is cleared once it ends");
    }

    #[test]
    fn a_closed_player_ends_the_session_without_serving_out_the_grace() {
        // The whole point of the distinction: grace exists to ride out a player
        // that is running but momentarily unreadable. A player that has exited
        // will never become readable again, so making the user wait three ticks
        // for the Up Next prompt is pure latency.
        let mut session = None;
        advance_session(&mut session, playing(1, 5), 100, 2);
        advance_session(&mut session, playing(1, 5), 400, 2);
        let ended = advance_session(&mut session, Observation::NoPlayer, 402, 2)
            .expect("the player is gone, so the session ends on this very tick");
        assert_eq!(ended.episode, 5);
        assert_eq!(
            ended.watched_secs, 300,
            "watched time still stops at the last tick the episode was seen"
        );
        assert!(session.is_none(), "session is cleared once it ends");
    }

    #[test]
    fn a_closed_player_ends_a_session_already_inside_its_grace_window() {
        // A seek that hid the title, then the player closed. The pending misses
        // must not delay the end any further.
        let mut session = None;
        advance_session(&mut session, playing(1, 5), 100, 2);
        advance_session(&mut session, playing(1, 5), 400, 2);
        advance_session(&mut session, Observation::Unidentified, 402, 2);
        let ended = advance_session(&mut session, Observation::NoPlayer, 404, 2)
            .expect("ends as soon as the player is known to be gone");
        assert_eq!(ended.watched_secs, 300);
    }

    #[test]
    fn nothing_observed_without_a_session_is_a_no_op() {
        let mut session = None;
        assert_eq!(
            advance_session(&mut session, Observation::Unidentified, 100, 2),
            None
        );
        assert!(session.is_none());
        assert_eq!(
            advance_session(&mut session, Observation::NoPlayer, 100, 2),
            None
        );
        assert!(session.is_none());
    }

    #[test]
    fn zero_grace_ends_the_session_on_the_first_miss() {
        let mut session = None;
        advance_session(&mut session, playing(1, 5), 100, 0);
        advance_session(&mut session, playing(1, 5), 700, 0);
        let ended = advance_session(&mut session, Observation::Unidentified, 702, 0)
            .expect("ends immediately");
        assert_eq!(ended.watched_secs, 600);
    }

    #[test]
    fn min_watch_gate_compares_against_minutes() {
        assert!(!passes_min_watch(299, 5));
        assert!(passes_min_watch(300, 5));
        assert!(passes_min_watch(0, 0), "zero minutes always prompts");
    }
}
