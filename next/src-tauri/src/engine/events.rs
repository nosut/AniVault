use crate::engine::models::{AnimeId, EpisodeNumber, ServiceId};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MatchCandidate {
    pub anime_id: AnimeId,
    pub title: String,
    pub confidence: u8,
    pub match_source: String,
}

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
        candidates: Vec<MatchCandidate>,
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
    /// An automatic scan (watcher or timer) changed the file index — the UI
    /// should refresh library/file views. Not emitted for manual scans, which
    /// already return their report to the caller.
    LibraryUpdated {
        indexed: i64,
        removed: i64,
    },
}
