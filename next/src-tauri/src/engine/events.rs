use crate::engine::models::{AnimeId, EpisodeNumber, ServiceId};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MediaDetected {
    pub player_name: String,
    pub file_path: Option<String>,
    pub window_title: Option<String>,
    pub detected_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AnimeIdentified {
    pub anime_id: AnimeId,
    pub episode: EpisodeNumber,
    pub confidence: u8,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EngineEvent {
    MediaDetected(MediaDetected),
    AnimeIdentified(AnimeIdentified),
    PlaybackDetected {
        player_name: String,
        file_path: Option<String>,
        window_title: Option<String>,
        episode_guess: Option<EpisodeNumber>,
        detected_at_unix: i64,
    },
    ProgressAdvanced {
        anime_id: AnimeId,
        old_episode: EpisodeNumber,
        new_episode: EpisodeNumber,
        source: String,
    },
    SyncQueued {
        service: ServiceId,
        anime_id: AnimeId,
    },
    SyncFailed {
        service: ServiceId,
        anime_id: AnimeId,
        message: String,
    },
}
