pub type AnimeId = i64;
pub type EpisodeNumber = i32;
pub type ServiceId = String;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum WatchStatus {
    Watching,
    Completed,
    OnHold,
    Dropped,
    PlanToWatch,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ParseResult {
    pub title: String,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub confidence: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DetectionConfig {
    pub folders: Vec<String>,
    pub poll_interval_ms: u64,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            folders: vec!["D:\\Anime".to_string()],
            poll_interval_ms: 2_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TrackingStatus {
    pub is_running: bool,
    pub current_anime: Option<String>,
    pub current_anime_id: Option<i64>,
    pub current_episode: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OAuthStatus {
    pub authenticated: bool,
    pub username: Option<String>,
}
