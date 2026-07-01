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
