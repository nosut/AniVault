use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

#[derive(Clone)]
pub struct Storage {
    pool: SqlitePool,
}

impl Storage {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::query("PRAGMA journal_mode = WAL").execute(&self.pool).await?;
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    pub async fn journal_mode(&self) -> anyhow::Result<String> {
        let row = sqlx::query("PRAGMA journal_mode").fetch_one(&self.pool).await?;
        Ok(row.get::<String, _>(0))
    }

    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn insert_minimal_anime(&self, id: i64, title: &str) -> anyhow::Result<()> {
        let titles_json = serde_json::json!({ "romaji": title, "english": null, "japanese": null, "synonyms": [] }).to_string();
        sqlx::query(
            "INSERT OR REPLACE INTO anime (id, titles_json, last_modified) VALUES (?1, ?2, 0)",
        )
        .bind(id)
        .bind(titles_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn ensure_fts_index(&self) -> anyhow::Result<()> {
        sqlx::query(
            "CREATE VIRTUAL TABLE IF NOT EXISTS anime_fts USING fts5(anime_id UNINDEXED, title, synonyms)",
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn insert_minimal_anime_with_synonyms(
        &self,
        id: i64,
        title: &str,
        synonyms: &[&str],
    ) -> anyhow::Result<()> {
        let titles_json = serde_json::json!({
            "romaji": title,
            "english": null,
            "japanese": null,
            "synonyms": synonyms,
        })
        .to_string();
        sqlx::query(
            "INSERT OR REPLACE INTO anime (id, titles_json, last_modified) VALUES (?1, ?2, 0)",
        )
        .bind(id)
        .bind(titles_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn anime_by_id(&self, id: i64) -> anyhow::Result<Option<(i64, String, i32)>> {
        let row = sqlx::query(
            "SELECT anime.id, json_extract(anime.titles_json, '$.romaji'), COALESCE(list_entry.watched_episodes, 0)
             FROM anime
             LEFT JOIN list_entry ON list_entry.anime_id = anime.id
             WHERE anime.id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| (r.get(0), r.get(1), r.get(2))))
    }

    pub async fn update_watched_episodes(&self, anime_id: i64, episode: i32) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO list_entry (anime_id, status, watched_episodes, local_updated)
             VALUES (?1, 'watching', ?2, unixepoch())
             ON CONFLICT(anime_id) DO UPDATE SET watched_episodes = MAX(watched_episodes, ?2), local_updated = unixepoch()",
        )
        .bind(anime_id)
        .bind(episode)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_watched_episodes(&self, anime_id: i64, episode: i32) -> anyhow::Result<()> {
        let now = unixepoch_inner();
        sqlx::query(
            "INSERT INTO list_entry (anime_id, status, watched_episodes, local_updated)
             VALUES (?1, 'watching', ?2, ?3)
             ON CONFLICT(anime_id) DO UPDATE SET watched_episodes = ?2, local_updated = ?3",
        )
        .bind(anime_id)
        .bind(episode)
        .bind(now)
        .execute(&self.pool)
        .await?;

        // Log manual edit in watch_history
        sqlx::query(
            "INSERT OR IGNORE INTO watch_history (anime_id, episode, source, watched_at)
             VALUES (?1, ?2, 'manual', ?3)",
        )
        .bind(anime_id)
        .bind(episode)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn insert_or_ignore_anime_local(
        &self,
        id: i64,
        title_romaji: &str,
        title_english: Option<&str>,
        episode_count: Option<i32>,
    ) -> anyhow::Result<()> {
        let titles_json = serde_json::json!({
            "romaji": title_romaji,
            "english": title_english,
            "japanese": null,
            "synonyms": [],
        })
        .to_string();
        sqlx::query(
            "INSERT OR IGNORE INTO anime (id, titles_json, episode_count, last_modified) VALUES (?1, ?2, ?3, unixepoch())",
        )
        .bind(id)
        .bind(titles_json)
        .bind(episode_count)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn append_watch_history(
        &self,
        anime_id: i64,
        episode: i32,
        file_path: Option<&str>,
        player: Option<&str>,
        watched_at: i64,
    ) -> anyhow::Result<i64> {
        let result = sqlx::query(
            "INSERT INTO watch_history (anime_id, episode, file_path, player, watched_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(anime_id)
        .bind(episode)
        .bind(file_path)
        .bind(player)
        .bind(watched_at)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn append_watch_history_once(
        &self,
        anime_id: i64,
        episode: i32,
        file_path: Option<&str>,
        player: Option<&str>,
        watched_at: i64,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "INSERT OR IGNORE INTO watch_history (anime_id, episode, file_path, player, watched_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(anime_id)
        .bind(episode)
        .bind(file_path)
        .bind(player)
        .bind(watched_at)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn queue_sync(
        &self,
        anime_id: i64,
        service: &str,
        operation: &str,
        payload_json: &str,
        created_at: i64,
    ) -> anyhow::Result<i64> {
        let result = sqlx::query(
            "INSERT OR IGNORE INTO sync_queue (anime_id, service, operation, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(anime_id)
        .bind(service)
        .bind(operation)
        .bind(payload_json)
        .bind(created_at)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn pending_sync_count(&self, service: &str) -> anyhow::Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) FROM sync_queue WHERE service = ?1")
            .bind(service)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>(0))
    }

    pub async fn watch_history_count(&self, anime_id: i64, episode: i32) -> anyhow::Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) FROM watch_history WHERE anime_id = ?1 AND episode = ?2")
            .bind(anime_id)
            .bind(episode)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>(0))
    }

    pub async fn get_library_anime(&self) -> anyhow::Result<Vec<LibraryEntry>> {
        let rows = sqlx::query(
            "SELECT anime.id, json_extract(anime.titles_json, '$.romaji'),
                    COALESCE(list_entry.status, 'plan_to_watch'),
                    COALESCE(list_entry.watched_episodes, 0),
                    anime.episode_count
             FROM anime
             LEFT JOIN list_entry ON list_entry.anime_id = anime.id
             ORDER BY lower(json_extract(anime.titles_json, '$.romaji'))",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| LibraryEntry {
                id: r.get(0),
                title: r.get(1),
                status: r.get(2),
                watched_episodes: r.get(3),
                episode_count: r.get(4),
            })
            .collect())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct LibraryEntry {
    pub id: i64,
    pub title: String,
    pub status: String,
    pub watched_episodes: i32,
    pub episode_count: Option<i32>,
}

fn unixepoch_inner() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
