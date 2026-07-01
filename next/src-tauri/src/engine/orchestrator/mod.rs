use std::sync::Arc;
use std::time::{Duration, Instant};

const CURRENT_ANIME_TIMEOUT: Duration = Duration::from_secs(5);

use crate::engine::anilist::search_anilist_fallback;
use crate::engine::event_bus::EventBus;
use crate::engine::events::{AnimeIdentified, EngineEvent, MediaDetected};
use crate::engine::models::ParseResult;
use crate::engine::recognition::matcher::{search_local, MatchResult};
use crate::engine::recognition::parser::parse_filename;
use crate::engine::storage::Storage;

pub async fn handle_media_detected(
    storage: &Storage,
    detected: MediaDetected,
) -> anyhow::Result<Option<MatchResult>> {
    Ok(handle_media_detected_event(storage, detected)
        .await?
        .map(|outcome| outcome.matched))
}

pub fn start_tracking_loop(bus: EventBus, storage: Storage) {
    start_tracking_loop_with_status(bus, storage, Arc::new(|_| {}));
}

pub fn start_tracking_loop_with_status(
    bus: EventBus,
    storage: Storage,
    on_current_anime: Arc<dyn Fn(Option<String>) + Send + Sync>,
) {
    tokio::spawn(async move {
        let mut current_anime_timeout = CurrentAnimeTimeout::new(CURRENT_ANIME_TIMEOUT);
        loop {
            for event in bus.drain() {
                if let EngineEvent::MediaDetected(detected) = event {
                    if let Ok(Some(outcome)) = handle_media_detected_event(&storage, detected).await {
                        on_current_anime(Some(outcome.matched.title.clone()));
                        current_anime_timeout.record_match(Instant::now());
                        bus.publish(EngineEvent::AnimeIdentified(AnimeIdentified {
                            anime_id: outcome.matched.anime_id,
                            episode: outcome.episode,
                            confidence: outcome.matched.confidence,
                            evidence: outcome.evidence,
                        }));
                        bus.publish(EngineEvent::ProgressAdvanced {
                            anime_id: outcome.matched.anime_id,
                            old_episode: outcome.old_episode,
                            new_episode: outcome.episode,
                            source: "local_detection".to_string(),
                        });
                    }
                }
            }
            if current_anime_timeout.should_clear(Instant::now()) {
                on_current_anime(None);
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
}

#[derive(Debug, Clone)]
struct CurrentAnimeTimeout {
    timeout: Duration,
    last_match_at: Option<Instant>,
    is_current_set: bool,
}

impl CurrentAnimeTimeout {
    fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            last_match_at: None,
            is_current_set: false,
        }
    }

    fn record_match(&mut self, now: Instant) {
        self.last_match_at = Some(now);
        self.is_current_set = true;
    }

    fn should_clear(&mut self, now: Instant) -> bool {
        if !self.is_current_set {
            return false;
        }

        let Some(last_match_at) = self.last_match_at else {
            return false;
        };

        if now.duration_since(last_match_at) >= self.timeout {
            self.is_current_set = false;
            self.last_match_at = None;
            return true;
        }

        false
    }
}

#[derive(Debug, Clone)]
struct TrackingOutcome {
    matched: MatchResult,
    episode: i32,
    old_episode: i32,
    evidence: String,
}

async fn handle_media_detected_event(
    storage: &Storage,
    detected: MediaDetected,
) -> anyhow::Result<Option<TrackingOutcome>> {
    let Some((evidence, parsed, episode)) = parse_detected_media(&detected) else {
        return Ok(None);
    };

    let matched = match search_local(storage, &parsed).await? {
        Some(m) => m,
        None => {
            // Fallback to AniList search with auto-add
            match search_anilist_fallback(storage, &parsed.title).await? {
                Some(m) => m,
                None => return Ok(None),
            }
        }
    };

    let old_episode = storage
        .anime_by_id(matched.anime_id)
        .await?
        .map(|(_, _, episode)| episode)
        .unwrap_or_default();

    storage
        .append_watch_history_once(
            matched.anime_id,
            episode,
            detected.file_path.as_deref(),
            Some(&detected.player_name),
            detected.detected_at_unix,
        )
        .await?;
    storage.update_watched_episodes(matched.anime_id, episode).await?;

    Ok(Some(TrackingOutcome {
        matched,
        episode,
        old_episode,
        evidence,
    }))
}

fn parse_detected_media(detected: &MediaDetected) -> Option<(String, ParseResult, i32)> {
    let evidence = detection_evidence(detected)?;
    let parsed = parse_filename(&evidence);
    let episode = parsed.episode?;
    Some((evidence, parsed, episode))
}

fn detection_evidence(detected: &MediaDetected) -> Option<String> {
    detected
        .file_path
        .clone()
        .or_else(|| detected.window_title.clone())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_anime_timeout_clears_once_after_inactivity() {
        let mut timeout = CurrentAnimeTimeout::new(Duration::from_secs(5));
        let now = std::time::Instant::now();

        timeout.record_match(now);

        assert!(!timeout.should_clear(now + Duration::from_secs(4)));
        assert!(timeout.should_clear(now + Duration::from_secs(6)));
        assert!(!timeout.should_clear(now + Duration::from_secs(7)));
    }
}
