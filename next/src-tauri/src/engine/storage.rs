use serde::Deserialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

pub struct AnimeRow {
    pub id: i64,
    pub titles_json: String,
    pub episode_count: Option<i32>,
}

pub struct ListEntryRow {
    pub anime_id: i64,
    pub status: String,
    pub watched_episodes: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WatchHistoryRow {
    pub id: i64,
    pub anime_id: i64,
    pub episode: i32,
    pub file_path: Option<String>,
    pub player: Option<String>,
    pub watched_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, Deserialize)]
pub struct FileIndexRow {
    pub file_path: String,
    pub anime_id: Option<i64>,
    pub episode: Option<i32>,
    pub confidence: i32,
    pub indexed_at: i64,
}

#[derive(Clone)]
pub struct Storage {
    pool: SqlitePool,
}

impl Storage {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
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

    pub async fn close(self) {
        self.pool.close().await;
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

    pub async fn queue_sync(
        &self,
        anime_id: i64,
        service: &str,
        operation: &str,
        payload_json: &str,
        created_at: i64,
    ) -> anyhow::Result<i64> {
        let result = sqlx::query(
            "INSERT INTO sync_queue (anime_id, service, operation, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
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

    pub async fn migration_count(&self) -> anyhow::Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>(0))
    }

    pub async fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>> {
        let row = sqlx::query("SELECT value_json FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|row| row.get::<String, _>(0)))
    }

    pub async fn set_setting(
        &self,
        key: &str,
        value_json: &str,
        updated_at: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value_json)
        .bind(updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_setting(&self, key: &str) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM settings WHERE key = ?1")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn fetch_anime(&self, id: i64) -> anyhow::Result<Option<AnimeRow>> {
        let row = sqlx::query("SELECT id, titles_json, episode_count FROM anime WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|row| AnimeRow {
            id: row.get("id"),
            titles_json: row.get("titles_json"),
            episode_count: row.get("episode_count"),
        }))
    }

    pub async fn upsert_list_entry_progress(
        &self,
        anime_id: i64,
        status: &str,
        watched_episodes: i32,
        updated: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO list_entry (anime_id, status, watched_episodes, local_updated)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(anime_id) DO UPDATE SET
               status = excluded.status,
               watched_episodes = MAX(excluded.watched_episodes, list_entry.watched_episodes),
               local_updated = excluded.local_updated",
        )
        .bind(anime_id)
        .bind(status)
        .bind(watched_episodes)
        .bind(updated)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_list_entry(&self, anime_id: i64) -> anyhow::Result<Option<ListEntryRow>> {
        let row = sqlx::query(
            "SELECT anime_id, status, watched_episodes FROM list_entry WHERE anime_id = ?1",
        )
        .bind(anime_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| ListEntryRow {
            anime_id: row.get("anime_id"),
            status: row.get("status"),
            watched_episodes: row.get("watched_episodes"),
        }))
    }

    pub async fn list_recent_watch_history(&self, limit: i64) -> anyhow::Result<Vec<WatchHistoryRow>> {
        let rows = sqlx::query(
            "SELECT id, anime_id, episode, file_path, player, watched_at
             FROM watch_history ORDER BY watched_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|row| WatchHistoryRow {
                id: row.get("id"),
                anime_id: row.get("anime_id"),
                episode: row.get("episode"),
                file_path: row.get("file_path"),
                player: row.get("player"),
                watched_at: row.get("watched_at"),
            })
            .collect())
    }

    pub async fn search_anime_by_title(&self, query: &str, limit: i64) -> anyhow::Result<Vec<AnimeRow>> {
        let pattern = format!("%{}%", query);
        let rows = sqlx::query(
            "SELECT id, titles_json, episode_count FROM anime
             WHERE titles_json LIKE ?1
             ORDER BY id LIMIT ?2",
        )
        .bind(&pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|row| AnimeRow {
                id: row.get("id"),
                titles_json: row.get("titles_json"),
                episode_count: row.get("episode_count"),
            })
            .collect())
    }

    pub async fn upsert_file_index(
        &self,
        file_path: &str,
        anime_id: i64,
        episode: i32,
        confidence: i32,
        indexed_at: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO file_index (file_path, anime_id, episode, confidence, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(file_path) DO UPDATE SET
               anime_id = excluded.anime_id,
               episode = excluded.episode,
               confidence = excluded.confidence,
               indexed_at = excluded.indexed_at",
        )
        .bind(file_path)
        .bind(anime_id)
        .bind(episode)
        .bind(confidence)
        .bind(indexed_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_file_index(&self, file_path: &str) -> anyhow::Result<Option<FileIndexRow>> {
        let row = sqlx::query(
            "SELECT file_path, anime_id, episode, confidence, indexed_at
             FROM file_index WHERE file_path = ?1",
        )
        .bind(file_path)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| FileIndexRow {
            file_path: row.get("file_path"),
            anime_id: row.get("anime_id"),
            episode: row.get("episode"),
            confidence: row.get("confidence"),
            indexed_at: row.get("indexed_at"),
        }))
    }

    pub async fn list_file_index(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<FileIndexRow>> {
        let rows = sqlx::query(
            "SELECT file_path, anime_id, episode, confidence, indexed_at
             FROM file_index ORDER BY indexed_at DESC LIMIT ?1 OFFSET ?2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|row| FileIndexRow {
                file_path: row.get("file_path"),
                anime_id: row.get("anime_id"),
                episode: row.get("episode"),
                confidence: row.get("confidence"),
                indexed_at: row.get("indexed_at"),
            })
            .collect())
    }
}

pub struct Tests;

impl Tests {
    pub async fn new_in_memory() -> Storage {
        let storage = Storage::connect("sqlite::memory:").await.unwrap();
        storage.migrate().await.unwrap();
        storage
    }
}
