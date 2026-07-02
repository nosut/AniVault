use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;
use std::time::SystemTime;

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

pub struct ListEntryFullRow {
    pub anime_id: i64,
    pub status: String,
    pub watched_episodes: i32,
    pub score: Option<i32>,
    pub notes: Option<String>,
    pub date_started: Option<String>,
    pub date_completed: Option<String>,
    pub local_updated: i64,
    pub remote_updated: Option<i64>,
}

pub struct SyncQueueRow {
    pub id: i64,
    pub anime_id: i64,
    pub service: String,
    pub operation: String,
    pub payload_json: String,
    pub created_at: i64,
    pub retry_count: i32,
    pub next_retry_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibraryRow {
    pub anime_id: i64,
    pub title: String,
    pub status: String,
    pub watched_episodes: i32,
    pub episode_count: Option<i32>,
    pub score: Option<i32>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibraryStats {
    pub total: i64,
    pub watching: i64,
    pub completed: i64,
    pub on_hold: i64,
    pub dropped: i64,
    pub plan_to_watch: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnimeDetailRow {
    pub anime_id: i64,
    pub titles_json: String,
    pub episode_count: Option<i32>,
    pub image_url: Option<String>,
    pub synopsis: Option<String>,
    pub anime_status: Option<String>,
    pub last_modified: i64,
    pub list_status: Option<String>,
    pub watched_episodes: Option<i32>,
    pub score: Option<i32>,
    pub notes: Option<String>,
    pub local_updated: Option<i64>,
    pub remote_updated: Option<i64>,
    pub tracker_id: Option<String>,
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

    // ── M3 AniList storage helpers ──────────────────────────────────────────

    pub async fn upsert_anime(
        &self,
        id: i64,
        titles_json: &str,
        episode_count: i32,
        image_url: Option<&str>,
        last_modified: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO anime (id, titles_json, episode_count, image_url, last_modified)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
               titles_json = excluded.titles_json,
               episode_count = excluded.episode_count,
               image_url = excluded.image_url,
               last_modified = excluded.last_modified",
        )
        .bind(id)
        .bind(titles_json)
        .bind(episode_count)
        .bind(image_url)
        .bind(last_modified)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn upsert_list_entry_full(
        &self,
        anime_id: i64,
        status: &str,
        watched_episodes: i32,
        score: Option<i32>,
        notes: &str,
        local_updated: i64,
        remote_updated: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO list_entry (anime_id, status, watched_episodes, score, notes, local_updated, remote_updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(anime_id) DO UPDATE SET
               status = excluded.status,
               watched_episodes = excluded.watched_episodes,
               score = excluded.score,
               notes = excluded.notes,
               local_updated = excluded.local_updated,
               remote_updated = excluded.remote_updated",
        )
        .bind(anime_id)
        .bind(status)
        .bind(watched_episodes)
        .bind(score)
        .bind(notes)
        .bind(local_updated)
        .bind(remote_updated)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_list_entry_full(
        &self,
        anime_id: i64,
    ) -> anyhow::Result<Option<ListEntryFullRow>> {
        let row = sqlx::query(
            "SELECT anime_id, status, watched_episodes, score, notes,
                    date_started, date_completed, local_updated, remote_updated
             FROM list_entry WHERE anime_id = ?1",
        )
        .bind(anime_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| ListEntryFullRow {
            anime_id: r.get("anime_id"),
            status: r.get("status"),
            watched_episodes: r.get("watched_episodes"),
            score: r.get("score"),
            notes: r.get("notes"),
            date_started: r.get("date_started"),
            date_completed: r.get("date_completed"),
            local_updated: r.get("local_updated"),
            remote_updated: r.get("remote_updated"),
        }))
    }

    pub async fn upsert_tracker_mapping(
        &self,
        anime_id: i64,
        service: &str,
        remote_id: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO tracker_mapping (anime_id, service, remote_id)
             VALUES (?1, ?2, ?3)",
        )
        .bind(anime_id)
        .bind(service)
        .bind(remote_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn fetch_pending_sync_rows(
        &self,
        service: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<SyncQueueRow>> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let rows = sqlx::query(
            "SELECT id, anime_id, service, operation, payload_json,
                    created_at, retry_count, next_retry_at
             FROM sync_queue
             WHERE service = ?1
               AND (next_retry_at IS NULL OR next_retry_at <= ?2)
             ORDER BY created_at ASC
             LIMIT ?3",
        )
        .bind(service)
        .bind(now)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| SyncQueueRow {
                id: r.get("id"),
                anime_id: r.get("anime_id"),
                service: r.get("service"),
                operation: r.get("operation"),
                payload_json: r.get("payload_json"),
                created_at: r.get("created_at"),
                retry_count: r.get("retry_count"),
                next_retry_at: r.get("next_retry_at"),
            })
            .collect())
    }

    pub async fn delete_sync_row(&self, id: i64) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM sync_queue WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_tracker_mappings(&self, service: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM tracker_mapping WHERE service = ?1")
            .bind(service)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_sync_retry(
        &self,
        id: i64,
        retry_count: i32,
        next_retry_at: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE sync_queue SET retry_count = ?1, next_retry_at = ?2 WHERE id = ?3",
        )
        .bind(retry_count)
        .bind(next_retry_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn sync_status_counts(
        &self,
        service: &str,
    ) -> anyhow::Result<(i64, i64, i64)> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sync_queue WHERE service = ?1 AND retry_count = 0",
        )
        .bind(service)
        .fetch_one(&self.pool)
        .await?;

        let failed: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sync_queue
             WHERE service = ?1
               AND retry_count > 0
               AND retry_count < 3
               AND (next_retry_at IS NULL OR next_retry_at <= ?2)",
        )
        .bind(service)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        let blocked: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sync_queue WHERE service = ?1 AND retry_count >= 3",
        )
        .bind(service)
        .fetch_one(&self.pool)
        .await?;

        Ok((pending, failed, blocked))
    }
    // ── Library browsing helpers ─────────────────────────────────────────

    pub async fn search_library(
        &self,
        query: &str,
        status_filter: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<LibraryRow>> {
        let pattern = format!("%{}%", query);

        let mut sql = String::from(
            "SELECT a.id as anime_id, json_extract(a.titles_json, '$.romaji') as title, \
             COALESCE(le.status, 'unlisted') as status, \
             COALESCE(le.watched_episodes, 0) as watched_episodes, \
             a.episode_count, le.score, a.image_url \
             FROM anime a \
             LEFT JOIN list_entry le ON a.id = le.anime_id \
             WHERE a.titles_json LIKE ?",
        );

        let use_filter = status_filter.is_some_and(|s| !s.is_empty());

        if use_filter {
            let sf = status_filter.unwrap();
            if sf == "unlisted" {
                sql.push_str(" AND le.anime_id IS NULL");
            } else {
                sql.push_str(" AND le.status = ?");
            }
        }

        sql.push_str(" ORDER BY a.id LIMIT ? OFFSET ?");

        let mut query_builder = sqlx::query(&sql).bind(&pattern);

        if use_filter {
            let sf = status_filter.unwrap();
            if sf != "unlisted" {
                query_builder = query_builder.bind(sf);
            }
        }

        query_builder = query_builder.bind(limit).bind(offset);

        let rows = query_builder.fetch_all(&self.pool).await?;

        Ok(rows
            .iter()
            .map(|row| LibraryRow {
                anime_id: row.get("anime_id"),
                title: row.get("title"),
                status: row.get("status"),
                watched_episodes: row.get("watched_episodes"),
                episode_count: row.get("episode_count"),
                score: row.get("score"),
                image_url: row.get("image_url"),
            })
            .collect())
    }

    pub async fn library_stats(&self) -> anyhow::Result<LibraryStats> {
        let row = sqlx::query(
            "SELECT \
             COUNT(*) as total, \
             COALESCE(SUM(CASE WHEN le.status = 'watching' THEN 1 ELSE 0 END), 0) as watching, \
             COALESCE(SUM(CASE WHEN le.status = 'completed' THEN 1 ELSE 0 END), 0) as completed, \
             COALESCE(SUM(CASE WHEN le.status = 'on_hold' THEN 1 ELSE 0 END), 0) as on_hold, \
             COALESCE(SUM(CASE WHEN le.status = 'dropped' THEN 1 ELSE 0 END), 0) as dropped, \
             COALESCE(SUM(CASE WHEN le.status = 'plan_to_watch' THEN 1 ELSE 0 END), 0) as plan_to_watch \
             FROM anime a \
             LEFT JOIN list_entry le ON a.id = le.anime_id",
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(LibraryStats {
            total: row.get("total"),
            watching: row.get("watching"),
            completed: row.get("completed"),
            on_hold: row.get("on_hold"),
            dropped: row.get("dropped"),
            plan_to_watch: row.get("plan_to_watch"),
        })
    }

    pub async fn anime_detail(&self, anime_id: i64) -> anyhow::Result<AnimeDetailRow> {
        let row = sqlx::query(
            "SELECT a.id as anime_id, a.titles_json, a.episode_count, a.image_url, \
             a.synopsis, a.status as anime_status, a.last_modified, \
             le.status as list_status, le.watched_episodes, le.score, le.notes, \
             le.local_updated, le.remote_updated, \
             tm.remote_id as tracker_id \
             FROM anime a \
             LEFT JOIN list_entry le ON a.id = le.anime_id \
             LEFT JOIN tracker_mapping tm ON a.id = tm.anime_id AND tm.service = 'anilist' \
             WHERE a.id = ?1",
        )
        .bind(anime_id)
        .fetch_optional(&self.pool)
        .await?;

        let row = row.ok_or_else(|| anyhow::anyhow!("Anime not found: {}", anime_id))?;

        Ok(AnimeDetailRow {
            anime_id: row.get("anime_id"),
            titles_json: row.get("titles_json"),
            episode_count: row.get("episode_count"),
            image_url: row.get("image_url"),
            synopsis: row.get("synopsis"),
            anime_status: row.get("anime_status"),
            last_modified: row.get("last_modified"),
            list_status: row.get("list_status"),
            watched_episodes: row.get("watched_episodes"),
            score: row.get("score"),
            notes: row.get("notes"),
            local_updated: row.get("local_updated"),
            remote_updated: row.get("remote_updated"),
            tracker_id: row.get("tracker_id"),
        })
    }

    pub async fn update_list_entry_partial(
        &self,
        anime_id: i64,
        status: Option<&str>,
        watched_episodes: Option<i32>,
        score: Option<i32>,
    ) -> anyhow::Result<()> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        sqlx::query(
            "INSERT INTO list_entry (anime_id, status, watched_episodes, score, local_updated) \
             VALUES (?1, COALESCE(?2, 'watching'), COALESCE(?3, 0), ?4, ?5) \
             ON CONFLICT(anime_id) DO UPDATE SET \
               status = COALESCE(?2, status), \
               watched_episodes = COALESCE(?3, watched_episodes), \
               score = COALESCE(?4, score), \
               local_updated = ?5",
        )
        .bind(anime_id)
        .bind(status)
        .bind(watched_episodes)
        .bind(score)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
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
