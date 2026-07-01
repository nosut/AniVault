use std::time::Duration;

use crate::engine::event_bus::EventBus;
use crate::engine::events::{AnimeIdentified, EngineEvent, MediaDetected};
use crate::engine::recognition::matcher::{search_local, MatchResult};
use crate::engine::recognition::parser::parse_filename;
use crate::engine::storage::Storage;

pub async fn handle_media_detected(
    storage: &Storage,
    detected: MediaDetected,
) -> anyhow::Result<Option<MatchResult>> {
    let Some((parsed, episode)) = parse_detected_media(&detected) else {
        return Ok(None);
    };

    let Some(matched) = search_local(storage, &parsed).await? else {
        return Ok(None);
    };

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

    Ok(Some(matched))
}

pub fn start_tracking_loop(bus: EventBus, storage: Storage) {
    tokio::spawn(async move {
        loop {
            for event in bus.drain() {
                if let EngineEvent::MediaDetected(detected) = event {
                    let old_episode = current_episode(&storage, &detected).await.unwrap_or_default();
                    if let Ok(Some(matched)) = handle_media_detected(&storage, detected.clone()).await {
                        let episode = parse_detected_media(&detected)
                            .map(|(_, episode)| episode)
                            .unwrap_or_default();
                        bus.publish(EngineEvent::AnimeIdentified(AnimeIdentified {
                            anime_id: matched.anime_id,
                            episode,
                            confidence: matched.confidence,
                            evidence: detection_evidence(&detected).unwrap_or_default(),
                        }));
                        bus.publish(EngineEvent::ProgressAdvanced {
                            anime_id: matched.anime_id,
                            old_episode,
                            new_episode: episode,
                            source: "local_detection".to_string(),
                        });
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
}

fn parse_detected_media(detected: &MediaDetected) -> Option<(crate::engine::models::ParseResult, i32)> {
    let parsed = parse_filename(&detection_evidence(detected)?);
    let episode = parsed.episode?;
    Some((parsed, episode))
}

async fn current_episode(storage: &Storage, detected: &MediaDetected) -> anyhow::Result<i32> {
    let Some((parsed, _)) = parse_detected_media(detected) else {
        return Ok(0);
    };
    let Some(matched) = search_local(storage, &parsed).await? else {
        return Ok(0);
    };
    Ok(storage
        .anime_by_id(matched.anime_id)
        .await?
        .map(|(_, _, episode)| episode)
        .unwrap_or_default())
}

fn detection_evidence(detected: &MediaDetected) -> Option<String> {
    detected
        .file_path
        .clone()
        .or_else(|| detected.window_title.clone())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
