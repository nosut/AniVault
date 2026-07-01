use std::time::Duration;

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
    tokio::spawn(async move {
        loop {
            for event in bus.drain() {
                if let EngineEvent::MediaDetected(detected) = event {
                    if let Ok(Some(outcome)) = handle_media_detected_event(&storage, detected).await {
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
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
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

    let Some(matched) = search_local(storage, &parsed).await? else {
        return Ok(None);
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
