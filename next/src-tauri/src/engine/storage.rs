use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

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
}
