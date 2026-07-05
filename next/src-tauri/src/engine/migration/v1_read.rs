//! Taiga v1 data reader.
//!
//! Reads v1 data from two possible sources:
//! 1. v1 SQLite DB (`media.sqlite`) — tables `anime`, `anime_list`
//! 2. v1 XML files — legacy format for history, anime DB, list entries

use serde::{Deserialize, Serialize};

// ── Data types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V1Anime {
    pub id: i64,
    pub title: String,
    pub english: String,
    pub japanese: String,
    pub synonyms: Vec<String>,
    pub anime_type: i32,
    pub status: i32,
    pub episode_count: i32,
    pub image_url: String,
    pub synopsis: String,
    pub score: f32,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub last_modified: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V1ListEntry {
    pub anime_id: i64,
    pub watched_episodes: i32,
    pub score: i32,
    pub status: i32,
    pub date_started: String,
    pub date_completed: String,
    pub notes: String,
    pub last_updated: i64,
    pub rewatched_times: i32,
    pub rewatching: bool,
    pub rewatching_ep: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V1HistoryItem {
    pub anime_id: i64,
    pub episode: i32,
    pub timestamp: String,
}

// ── v1 status enums → v2 string mapping ──────────────────────────────────────

pub fn v1_list_status_to_v2(status: i32) -> &'static str {
    match status {
        1 => "watching",
        2 => "completed",
        3 => "on_hold",
        4 => "dropped",
        5 => "plan_to_watch",
        _ => "watching",
    }
}

pub fn v1_anime_type_to_v2(t: i32) -> &'static str {
    match t {
        1 => "tv",
        2 => "ova",
        3 => "movie",
        4 => "special",
        5 => "ona",
        6 => "music",
        _ => "tv",
    }
}

pub fn v1_anime_status_to_v2(s: i32) -> &'static str {
    match s {
        1 => "finished_airing",
        2 => "airing",
        3 => "not_yet_aired",
        _ => "finished_airing",
    }
}

// ── SQLite reader ────────────────────────────────────────────────────────────

fn split_comma(s: &str) -> Vec<String> {
    s.split(", ")
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect()
}

pub async fn read_v1_sqlite(
    path: &str,
) -> Result<(Vec<V1Anime>, Vec<V1ListEntry>), anyhow::Error> {
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::Row;
    use std::str::FromStr;

    let db_url = format!("sqlite:{}", path);
    let opts = SqliteConnectOptions::from_str(&db_url)?
        .read_only(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?;

    // Read anime table (check if table exists first)
    let table_check: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='anime'",
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    let anime: Vec<V1Anime> = if table_check > 0 {
        let rows = sqlx::query("SELECT * FROM anime")
            .fetch_all(&pool)
            .await?;
        rows.iter()
            .map(|row| V1Anime {
                id: row.get("id"),
                title: row.get::<String, _>("title"),
                english: row.try_get::<String, _>("english").unwrap_or_default(),
                japanese: row.try_get::<String, _>("japanese").unwrap_or_default(),
                synonyms: split_comma(
                    &row.try_get::<String, _>("synonym").unwrap_or_default(),
                ),
                anime_type: row.try_get("type").unwrap_or(0),
                status: row.try_get("status").unwrap_or(0),
                episode_count: row.try_get("episode_count").unwrap_or(-1),
                image_url: row.try_get::<String, _>("image").unwrap_or_default(),
                synopsis: row.try_get::<String, _>("synopsis").unwrap_or_default(),
                score: row.try_get::<f64, _>("score").unwrap_or(0.0) as f32,
                genres: split_comma(
                    &row.try_get::<String, _>("genres").unwrap_or_default(),
                ),
                tags: split_comma(
                    &row.try_get::<String, _>("tags").unwrap_or_default(),
                ),
                last_modified: row.try_get("modified").unwrap_or(0),
            })
            .collect()
    } else {
        Vec::new()
    };

    // Read anime_list table
    let list_check: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='anime_list'",
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    let entries: Vec<V1ListEntry> = if list_check > 0 {
        let rows = sqlx::query("SELECT * FROM anime_list")
            .fetch_all(&pool)
            .await?;
        rows.iter()
            .map(|row| V1ListEntry {
                anime_id: row.try_get("media_id").unwrap_or(0),
                watched_episodes: row.try_get("progress").unwrap_or(0),
                score: row.try_get("score").unwrap_or(0),
                status: row.try_get("status").unwrap_or(0),
                date_started: row
                    .try_get::<String, _>("date_start")
                    .unwrap_or_default(),
                date_completed: row
                    .try_get::<String, _>("date_end")
                    .unwrap_or_default(),
                notes: row.try_get::<String, _>("notes").unwrap_or_default(),
                last_updated: row.try_get("last_updated").unwrap_or(0),
                rewatched_times: row.try_get("rewatched_times").unwrap_or(0),
                rewatching: row.try_get("rewatching").unwrap_or(false),
                rewatching_ep: row.try_get("rewatching_ep").unwrap_or(0),
            })
            .collect()
    } else {
        Vec::new()
    };

    pool.close().await;
    Ok((anime, entries))
}

// ── XML readers (legacy v1 format) ───────────────────────────────────────────

/// Parse v1 history XML.
///
/// Expected format:
/// ```xml
/// <history>
///   <items>
///     <item>
///       <anime_id>123</anime_id>
///       <episode>5</episode>
///       <time>2024-01-15T20:30:00</time>
///     </item>
///   </items>
/// </history>
/// ```
pub fn read_v1_history_xml(path: &str) -> Result<Vec<V1HistoryItem>, anyhow::Error> {
    let xml = std::fs::read_to_string(path)?;
    let mut reader = quick_xml::Reader::from_str(&xml);
    reader.config_mut().trim_text(true);

    let mut items = Vec::new();
    let mut current: Option<V1HistoryItem> = None;
    let mut buf = Vec::new();
    let mut in_item = false;
    let mut current_field = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "item" => {
                        in_item = true;
                        current = Some(V1HistoryItem {
                            anime_id: 0,
                            episode: 0,
                            timestamp: String::new(),
                        });
                    }
                    "anime_id" | "episode" | "time" if in_item => {
                        current_field = name;
                    }
                    _ => {}
                }
            }
            Ok(quick_xml::events::Event::Text(ref e)) => {
                if in_item {
                    if let Some(ref mut item) = current {
                        let text = e.unescape()?.to_string();
                        match current_field.as_str() {
                            "anime_id" => item.anime_id = text.parse().unwrap_or(0),
                            "episode" => item.episode = text.parse().unwrap_or(0),
                            "time" => item.timestamp = text,
                            _ => {}
                        }
                    }
                }
            }
            Ok(quick_xml::events::Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "item" {
                    if let Some(item) = current.take() {
                        if item.anime_id > 0 {
                            items.push(item);
                        }
                    }
                    in_item = false;
                }
                current_field.clear();
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => {
                // Skip malformed XML nodes; continue parsing
                if let quick_xml::Error::IllFormed(_) = e {
                    buf.clear();
                    continue;
                }
                return Err(e.into());
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(items)
}

/// Parse v1 anime XML database.
///
/// Expected format:
/// ```xml
/// <database>
///   <anime>
///     <id>...</id>
///     <title>...</title>
///     ...same fields as SQLite anime table...
///   </anime>
/// </database>
/// ```
pub fn read_v1_anime_xml(path: &str) -> Result<Vec<V1Anime>, anyhow::Error> {
    let xml = std::fs::read_to_string(path)?;
    let mut reader = quick_xml::Reader::from_str(&xml);
    reader.config_mut().trim_text(true);

    let mut items = Vec::new();
    let mut current: Option<V1Anime> = None;
    let mut buf = Vec::new();
    let mut in_anime = false;
    let mut current_field = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "anime" => {
                        in_anime = true;
                        current = Some(V1Anime {
                            id: 0,
                            title: String::new(),
                            english: String::new(),
                            japanese: String::new(),
                            synonyms: Vec::new(),
                            anime_type: 0,
                            status: 0,
                            episode_count: -1,
                            image_url: String::new(),
                            synopsis: String::new(),
                            score: 0.0,
                            genres: Vec::new(),
                            tags: Vec::new(),
                            last_modified: 0,
                        });
                    }
                    _ if in_anime => {
                        current_field = name;
                    }
                    _ => {}
                }
            }
            Ok(quick_xml::events::Event::Text(ref e)) => {
                if in_anime {
                    if let Some(ref mut item) = current {
                        let text = e.unescape()?.to_string();
                        match current_field.as_str() {
                            "id" => item.id = text.parse().unwrap_or(0),
                            "title" => item.title = text,
                            "english" => item.english = text,
                            "japanese" => item.japanese = text,
                            "synonym" => item.synonyms = split_comma(&text),
                            "type" => item.anime_type = text.parse().unwrap_or(0),
                            "status" => item.status = text.parse().unwrap_or(0),
                            "episode_count" => {
                                item.episode_count = text.parse().unwrap_or(-1)
                            }
                            "image" => item.image_url = text,
                            "synopsis" => item.synopsis = text,
                            "score" => item.score = text.parse().unwrap_or(0.0),
                            "genres" => item.genres = split_comma(&text),
                            "tags" => item.tags = split_comma(&text),
                            "modified" => {
                                item.last_modified = text.parse().unwrap_or(0)
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(quick_xml::events::Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "anime" {
                    if let Some(item) = current.take() {
                        if item.id > 0 {
                            items.push(item);
                        }
                    }
                    in_anime = false;
                }
                current_field.clear();
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => {
                if let quick_xml::Error::IllFormed(_) = e {
                    buf.clear();
                    continue;
                }
                return Err(e.into());
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(items)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_history_xml() {
        let xml = r#"<history><items></items></history>"#;
        let tmp = std::env::temp_dir().join("test_empty_history.xml");
        std::fs::write(&tmp, xml).unwrap();
        let items = read_v1_history_xml(&tmp.to_string_lossy()).unwrap();
        assert!(items.is_empty());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn parse_history_xml_with_items() {
        let xml = r#"<history><items>
            <item><anime_id>42</anime_id><episode>3</episode><time>2024-01-15T20:30:00</time></item>
            <item><anime_id>99</anime_id><episode>7</episode><time>2024-02-01T12:00:00</time></item>
        </items></history>"#;
        let tmp = std::env::temp_dir().join("test_history.xml");
        std::fs::write(&tmp, xml).unwrap();
        let items = read_v1_history_xml(&tmp.to_string_lossy()).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].anime_id, 42);
        assert_eq!(items[0].episode, 3);
        assert_eq!(items[1].anime_id, 99);
        assert_eq!(items[1].episode, 7);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn v1_status_mapping() {
        assert_eq!(v1_list_status_to_v2(1), "watching");
        assert_eq!(v1_list_status_to_v2(2), "completed");
        assert_eq!(v1_list_status_to_v2(3), "on_hold");
        assert_eq!(v1_list_status_to_v2(4), "dropped");
        assert_eq!(v1_list_status_to_v2(5), "plan_to_watch");
        assert_eq!(v1_list_status_to_v2(0), "watching"); // default
    }

    #[test]
    fn v1_type_mapping() {
        assert_eq!(v1_anime_type_to_v2(1), "tv");
        assert_eq!(v1_anime_type_to_v2(2), "ova");
        assert_eq!(v1_anime_type_to_v2(3), "movie");
    }

    #[test]
    fn split_comma_handles_empty() {
        assert!(split_comma("").is_empty());
    }

    #[test]
    fn split_comma_splits_correctly() {
        let result = split_comma("Action, Adventure, Sci-Fi");
        assert_eq!(result, vec!["Action", "Adventure", "Sci-Fi"]);
    }

    #[tokio::test]
    async fn read_v1_sqlite_returns_data() {
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;

        // Create temp file v1 database using sqlx
        let tmp = std::env::temp_dir().join("test_v1_media.sqlite");
        let _ = std::fs::remove_file(&tmp); // clean up previous runs
        let db_url = format!("sqlite:{}", tmp.to_string_lossy());
        let opts = SqliteConnectOptions::from_str(&db_url)
            .unwrap()
            .create_if_missing(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE anime (
                id INTEGER PRIMARY KEY,
                title TEXT, english TEXT, japanese TEXT, synonym TEXT,
                type INTEGER, status INTEGER, episode_count INTEGER,
                image TEXT, synopsis TEXT, score REAL,
                genres TEXT, tags TEXT, modified INTEGER
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "CREATE TABLE anime_list (
                media_id INTEGER PRIMARY KEY,
                progress INTEGER, score INTEGER, status INTEGER,
                date_start TEXT, date_end TEXT, notes TEXT,
                last_updated INTEGER, rewatched_times INTEGER,
                rewatching INTEGER, rewatching_ep INTEGER
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO anime VALUES (1, 'Test Title', 'English', '日本語', 'Syn1, Syn2', 1, 1, 12, '', '', 8.5, 'Action', 'tag1', 1000)",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO anime_list VALUES (1, 5, 80, 1, '2024-01-01', '', 'notes here', 2000, 0, 0, 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool.close().await;

        let (anime, entries) = read_v1_sqlite(&tmp.to_string_lossy()).await.unwrap();
        assert_eq!(anime.len(), 1);
        assert_eq!(entries.len(), 1);

        assert_eq!(anime[0].id, 1);
        assert_eq!(anime[0].title, "Test Title");
        assert_eq!(anime[0].english, "English");
        assert_eq!(anime[0].japanese, "日本語");
        assert_eq!(anime[0].synonyms.len(), 2);
        assert_eq!(anime[0].episode_count, 12);
        assert_eq!(anime[0].genres, vec!["Action"]);
        assert_eq!(anime[0].score, 8.5);

        assert_eq!(entries[0].anime_id, 1);
        assert_eq!(entries[0].watched_episodes, 5);
        assert_eq!(entries[0].score, 80);
        assert_eq!(entries[0].notes, "notes here");

        let _ = std::fs::remove_file(&tmp);
    }
}
