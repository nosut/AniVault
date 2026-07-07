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

#[derive(Debug, Clone, serde::Serialize)]
pub struct WatchHistoryFullRow {
    pub id: i64,
    pub anime_id: i64,
    pub anime_title: String,
    pub episode: i32,
    pub file_path: Option<String>,
    pub player: Option<String>,
    pub watched_at: i64,
    pub source: String,
}

#[derive(Debug, Clone, serde::Serialize, Deserialize)]
pub struct FileIndexRow {
    pub file_path: String,
    pub anime_id: Option<i64>,
    pub episode: Option<i32>,
    pub confidence: i32,
    pub indexed_at: i64,
    #[serde(default)]
    pub ignored: bool,
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
    pub season: Option<String>,
    pub season_year: Option<i32>,
    /// AniList media airing status (e.g. RELEASING, FINISHED) — used to guess a
    /// download-bar length when the episode count is still unknown.
    pub airing_status: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AnimeStats {
    pub score_distribution: Vec<ScoreBucket>,
    pub total_anime: i64,
    pub total_episodes_watched: i64,
    pub total_rewatches: i64,
    pub avg_score: f64,
    pub episodes_today: i64,
    pub episodes_this_week: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ScoreBucket {
    pub range: String,
    pub count: i64,
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SonarrSeriesDb {
    pub sonarr_id: i64,
    pub title: String,
    pub season_count: i32,
    pub episode_count: i32,
    pub episode_file_count: i32,
    pub monitored: bool,
    pub next_airing: Option<i64>,
    pub path: Option<String>,
    pub poster_url: Option<String>,
    pub overview: Option<String>,
    pub network: Option<String>,
    pub status: Option<String>,
    pub added: i64,
    pub last_synced: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SonarrMappingDb {
    pub id: Option<i64>,
    pub sonarr_id: i64,
    pub anime_id: Option<i64>,
    pub title_match: String,
    pub confidence: i32,
    pub mapped_at: i64,
    pub user_confirmed: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SonarrSeriesListRow {
    pub sonarr_id: i64,
    pub title: String,
    pub poster_url: Option<String>,
    pub episode_count: i32,
    pub anime_id: Option<i64>,
    pub confidence: Option<i32>,
    pub anime_title: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SonarrAvailabilityDb {
    pub sonarr_id: i64,
    pub sonarr_title: String,
    pub monitored: bool,
    pub episode_count: i32,
    pub episode_file_count: i32,
    pub next_airing: Option<i64>,
    pub path: Option<String>,
    pub season_count: i32,
    pub sonarr_status: Option<String>,
}

#[derive(Clone)]
pub struct Storage {
    pool: SqlitePool,
    database_path: String,
}

impl Storage {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let opts = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true);
        // Each `sqlite::memory:` connection is a separate empty database, so an
        // in-memory pool must stay at 1 connection or queries can land on a
        // connection that never ran the migrations.
        let max_connections = if database_url.contains(":memory:") { 1 } else { 5 };
        let pool = SqlitePoolOptions::new()
            .max_connections(max_connections)
            .connect_with(opts)
            .await?;
        let db_path = database_url.strip_prefix("sqlite:").unwrap_or(database_url).to_string();
        Ok(Self { pool, database_path: db_path })
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::query("PRAGMA journal_mode = WAL").execute(&self.pool).await?;
        sqlx::query("PRAGMA wal_autocheckpoint = 1000").execute(&self.pool).await?;
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    pub async fn journal_mode(&self) -> anyhow::Result<String> {
        let row = sqlx::query("PRAGMA journal_mode").fetch_one(&self.pool).await?;
        Ok(row.get::<String, _>(0))
    }

    pub async fn close(&self) {
        self.pool.close().await;
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

    pub async fn list_all_watch_history(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<WatchHistoryFullRow>> {
        let rows = sqlx::query(
            "SELECT wh.id, wh.anime_id, \
             COALESCE(NULLIF(json_extract(a.titles_json, '$.english'), ''), json_extract(a.titles_json, '$.romaji'), 'Unknown') as anime_title, \
             wh.episode, wh.file_path, wh.player, wh.watched_at, wh.source \
             FROM watch_history wh \
             LEFT JOIN anime a ON wh.anime_id = a.id \
             ORDER BY wh.watched_at DESC \
             LIMIT ?1 OFFSET ?2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| WatchHistoryFullRow {
                id: r.get("id"),
                anime_id: r.get("anime_id"),
                anime_title: r.get("anime_title"),
                episode: r.get("episode"),
                file_path: r.get("file_path"),
                player: r.get("player"),
                watched_at: r.get("watched_at"),
                source: r.get("source"),
            })
            .collect())
    }

    pub async fn watch_history_total_count(&self) -> anyhow::Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) FROM watch_history")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>(0))
    }

    pub async fn search_watch_history(
        &self,
        query: &str,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<WatchHistoryFullRow>> {
        let pattern = format!("%{}%", query);
        let rows = sqlx::query(
            "SELECT wh.id, wh.anime_id, \
             COALESCE(NULLIF(json_extract(a.titles_json, '$.english'), ''), json_extract(a.titles_json, '$.romaji'), 'Unknown') as anime_title, \
             wh.episode, wh.file_path, wh.player, wh.watched_at, wh.source \
             FROM watch_history wh \
             LEFT JOIN anime a ON wh.anime_id = a.id \
             WHERE (json_extract(a.titles_json, '$.romaji') LIKE ?1 \
                OR json_extract(a.titles_json, '$.english') LIKE ?1 \
                OR json_extract(a.titles_json, '$.japanese') LIKE ?1 \
                OR EXISTS (SELECT 1 FROM json_each(a.titles_json, '$.synonyms') syn WHERE syn.value LIKE ?1)) \
             ORDER BY wh.watched_at DESC \
             LIMIT ?2 OFFSET ?3",
        )
        .bind(&pattern)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| WatchHistoryFullRow {
                id: r.get("id"),
                anime_id: r.get("anime_id"),
                anime_title: r.get("anime_title"),
                episode: r.get("episode"),
                file_path: r.get("file_path"),
                player: r.get("player"),
                watched_at: r.get("watched_at"),
                source: r.get("source"),
            })
            .collect())
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

    /// Recent watch history for a single anime (newest first) — powers the
    /// detail page's per-show history list.
    pub async fn list_recent_watch_history_for_anime(
        &self,
        anime_id: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<WatchHistoryRow>> {
        let rows = sqlx::query(
            "SELECT id, anime_id, episode, file_path, player, watched_at
             FROM watch_history WHERE anime_id = ?1 ORDER BY watched_at DESC LIMIT ?2",
        )
        .bind(anime_id)
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
        // Tokenize the query into significant words and match anime whose titles_json
        // contains ANY of them. A single full-title LIKE is too brittle: punctuation
        // ("Online!" vs stored "online?"), stylized characters ("«Fruitmaster»"), and
        // romaji-vs-English differences all cause misses. Broaden the candidate set here
        // and let the caller (matcher::score_title_match) rank precisely.
        let words: Vec<String> = query
            .split_whitespace()
            .map(|w| {
                w.chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
                    .to_lowercase()
            })
            // Drop very short / low-signal tokens ("a", "of", "to", "the", numbers)
            .filter(|w| w.len() >= 3 && w.parse::<i64>().is_err())
            .collect();

        // Build a WHERE clause with one LIKE per token, ORed together. Fetch a wider
        // pool than `limit` so the ranking step downstream has room to work.
        let (where_clause, patterns): (String, Vec<String>) = if words.is_empty() {
            // Fall back to the whole (trimmed) query if nothing survived tokenization.
            ("titles_json LIKE ?1".to_string(), vec![format!("%{}%", query.trim())])
        } else {
            let clauses: Vec<String> = (0..words.len())
                .map(|i| format!("titles_json LIKE ?{}", i + 1))
                .collect();
            let patterns = words.iter().map(|w| format!("%{}%", w)).collect();
            (clauses.join(" OR "), patterns)
        };

        let pool_limit = (limit * 5).max(25);
        let sql = format!(
            "SELECT id, titles_json, episode_count FROM anime WHERE {} ORDER BY id LIMIT {}",
            where_clause, pool_limit
        );
        let mut q = sqlx::query(&sql);
        for p in &patterns {
            q = q.bind(p);
        }
        let rows = q.fetch_all(&self.pool).await?;
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
        anime_id: Option<i64>,
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
            "SELECT file_path, anime_id, episode, confidence, indexed_at, ignored
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
            ignored: row.get::<i64, _>("ignored") != 0,
        }))
    }

    /// Look up a mapped file by its filename (basename) rather than full path.
    /// Players like mpv only surface the filename in their window title, so the
    /// absolute-path index lookup misses; this matches on the trailing filename.
    pub async fn get_file_index_by_filename(
        &self,
        filename: &str,
    ) -> anyhow::Result<Option<FileIndexRow>> {
        // Escape LIKE metacharacters so titles with % or _ match literally.
        let escaped = filename
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped}");
        let row = sqlx::query(
            "SELECT file_path, anime_id, episode, confidence, indexed_at, ignored \
             FROM file_index \
             WHERE file_path LIKE ?1 ESCAPE '\\' AND anime_id IS NOT NULL AND ignored = 0 \
             ORDER BY confidence DESC, indexed_at DESC LIMIT 1",
        )
        .bind(pattern)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| FileIndexRow {
            file_path: row.get("file_path"),
            anime_id: row.get("anime_id"),
            episode: row.get("episode"),
            confidence: row.get("confidence"),
            indexed_at: row.get("indexed_at"),
            ignored: row.get::<i64, _>("ignored") != 0,
        }))
    }

    pub async fn file_index_by_anime(&self, anime_id: i64) -> anyhow::Result<Vec<FileIndexRow>> {
        let rows = sqlx::query(
            "SELECT file_path, anime_id, episode, confidence, indexed_at, ignored FROM file_index WHERE anime_id = ?1 ORDER BY episode",
        )
        .bind(anime_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| FileIndexRow {
                file_path: r.get("file_path"),
                anime_id: r.get("anime_id"),
                episode: r.get("episode"),
                confidence: r.get("confidence"),
                indexed_at: r.get("indexed_at"),
                ignored: r.get::<i64, _>("ignored") != 0,
            })
            .collect())
    }

    pub async fn list_file_index(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<FileIndexRow>> {
        let rows = sqlx::query(
            "SELECT file_path, anime_id, episode, confidence, indexed_at, ignored
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
                ignored: row.get::<i64, _>("ignored") != 0,
            })
            .collect())
    }

    pub async fn delete_file_index(&self, file_path: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM file_index WHERE file_path = ?1")
            .bind(file_path)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Toggle the persistent "ignore" tombstone. When ignoring, the existing
    /// match is cleared so the file drops out of auto-match results, but the row
    /// is kept so the scanner keeps skipping it on future rescans.
    pub async fn set_file_index_ignored(
        &self,
        file_path: &str,
        ignored: bool,
    ) -> anyhow::Result<()> {
        if ignored {
            sqlx::query(
                "UPDATE file_index SET ignored = 1, anime_id = NULL, confidence = 0 WHERE file_path = ?1",
            )
            .bind(file_path)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query("UPDATE file_index SET ignored = 0 WHERE file_path = ?1")
                .bind(file_path)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Batch unmap: clear the anime link (and stale confidence) so files return
    /// to the "Unmapped" pool, keeping the rows so they can be re-mapped.
    pub async fn unmap_file_indexes(&self, file_paths: &[String]) -> anyhow::Result<()> {
        if file_paths.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for path in file_paths {
            sqlx::query("UPDATE file_index SET anime_id = NULL, confidence = 0 WHERE file_path = ?1")
                .bind(path)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Remove bogus file-index rows whose "path" is actually a player window
    /// title (no directory separator). These were mistakenly stored from mpv/VLC
    /// and shadow the real mappings. Returns the number removed.
    pub async fn delete_pathless_file_index(&self) -> anyhow::Result<u64> {
        let res = sqlx::query(
            "DELETE FROM file_index WHERE instr(file_path, '\\') = 0 AND instr(file_path, '/') = 0",
        )
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected())
    }

    /// All indexed file paths whose location is under `dir` (prefix match on the
    /// directory path). Used by the scanner to find rows whose file has been
    /// deleted from disk so they can be pruned. `dir` should already include a
    /// trailing path separator so it can't match a sibling like `Anime2\` when
    /// the folder is `Anime\`.
    pub async fn file_paths_under(&self, dir: &str) -> anyhow::Result<Vec<String>> {
        // Escape LIKE metacharacters so paths with % or _ match literally.
        let escaped = dir
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("{escaped}%");
        let rows = sqlx::query(
            "SELECT file_path FROM file_index WHERE file_path LIKE ?1 ESCAPE '\\'",
        )
        .bind(pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| r.get::<String, _>("file_path")).collect())
    }

    /// Batch delete of file-index rows in a single transaction.
    pub async fn delete_file_indexes(&self, file_paths: &[String]) -> anyhow::Result<()> {
        if file_paths.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for path in file_paths {
            sqlx::query("DELETE FROM file_index WHERE file_path = ?1")
                .bind(path)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Batch ignore / un-ignore in a single transaction (see `set_file_index_ignored`).
    pub async fn set_file_indexes_ignored(
        &self,
        file_paths: &[String],
        ignored: bool,
    ) -> anyhow::Result<()> {
        if file_paths.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for path in file_paths {
            if ignored {
                sqlx::query(
                    "UPDATE file_index SET ignored = 1, anime_id = NULL, confidence = 0 WHERE file_path = ?1",
                )
                .bind(path)
                .execute(&mut *tx)
                .await?;
            } else {
                sqlx::query("UPDATE file_index SET ignored = 0 WHERE file_path = ?1")
                    .bind(path)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    /// Batch manual mapping: each tuple is (file_path, anime_id, episode). All are
    /// written at full confidence (100) in a single transaction.
    pub async fn upsert_file_mappings(
        &self,
        mappings: &[(String, i64, i32)],
        indexed_at: i64,
    ) -> anyhow::Result<()> {
        if mappings.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for (file_path, anime_id, episode) in mappings {
            sqlx::query(
                "INSERT INTO file_index (file_path, anime_id, episode, confidence, indexed_at, ignored)
                 VALUES (?1, ?2, ?3, 100, ?4, 0)
                 ON CONFLICT(file_path) DO UPDATE SET
                   anime_id = excluded.anime_id,
                   episode = excluded.episode,
                   confidence = 100,
                   indexed_at = excluded.indexed_at,
                   ignored = 0",
            )
            .bind(file_path)
            .bind(anime_id)
            .bind(episode)
            .bind(indexed_at)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
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

    /// Set an anime's release season (COALESCE — only overwrites when a value is
    /// provided, so it won't clear an existing season).
    pub async fn set_anime_season(
        &self,
        id: i64,
        season: Option<&str>,
        season_year: Option<i32>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE anime SET season = COALESCE(?2, season), season_year = COALESCE(?3, season_year) WHERE id = ?1",
        )
        .bind(id)
        .bind(season)
        .bind(season_year)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn upsert_anime_full(
        &self,
        id: i64,
        titles_json: &str,
        episode_count: i32,
        image_url: Option<&str>,
        synopsis: Option<&str>,
        anime_type: Option<&str>,
        anime_status: Option<&str>,
        last_modified: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO anime (id, titles_json, type, status, episode_count, image_url, synopsis, last_modified)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
               titles_json = excluded.titles_json,
               type = COALESCE(excluded.type, anime.type),
               status = COALESCE(excluded.status, anime.status),
               episode_count = excluded.episode_count,
               image_url = COALESCE(excluded.image_url, anime.image_url),
               synopsis = COALESCE(excluded.synopsis, anime.synopsis),
               last_modified = excluded.last_modified",
        )
        .bind(id)
        .bind(titles_json)
        .bind(anime_type)
        .bind(anime_status)
        .bind(episode_count)
        .bind(image_url)
        .bind(synopsis)
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

        // retry_count >= 3 is "blocked" (see sync_status_counts / drain_queue):
        // those rows are excluded here so a permanently failing push isn't
        // retried on every worker pass forever.
        let rows = sqlx::query(
            "SELECT id, anime_id, service, operation, payload_json,
                    created_at, retry_count, next_retry_at
             FROM sync_queue
             WHERE service = ?1
               AND retry_count < 3
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
            "SELECT a.id as anime_id, COALESCE(NULLIF(json_extract(a.titles_json, '$.english'), ''), json_extract(a.titles_json, '$.romaji')) as title, \
             COALESCE(le.status, 'unlisted') as status, \
             COALESCE(le.watched_episodes, 0) as watched_episodes, \
             a.episode_count, le.score, a.image_url, a.season, a.season_year, \
             a.status as airing_status \
             FROM anime a \
             LEFT JOIN list_entry le ON a.id = le.anime_id \
             WHERE (json_extract(a.titles_json, '$.romaji') LIKE ?1 \
                OR json_extract(a.titles_json, '$.english') LIKE ?1 \
                OR json_extract(a.titles_json, '$.japanese') LIKE ?1 \
                OR EXISTS (SELECT 1 FROM json_each(a.titles_json, '$.synonyms') syn WHERE syn.value LIKE ?1))",
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
                season: row.get("season"),
                season_year: row.get("season_year"),
                airing_status: row.get("airing_status"),
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

    /// Flip a list entry to "completed" once its watched-episode count reaches the
    /// anime's episode count (e.g. 12/12). No-op if the show has no known episode
    /// count, isn't started, or is already completed.
    pub async fn auto_complete_if_capped(&self, anime_id: i64) -> anyhow::Result<bool> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let res = sqlx::query(
            "UPDATE list_entry SET status = 'completed', local_updated = ?2 \
             WHERE anime_id = ?1 AND status != 'completed' AND watched_episodes > 0 \
               AND watched_episodes >= \
                   (SELECT episode_count FROM anime \
                    WHERE id = ?1 AND episode_count IS NOT NULL AND episode_count > 0)",
        )
        .bind(anime_id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /// Delete an anime and everything tied to it. `list_entry`, `watch_history`,
    /// `tracker_mapping` and `sync_queue` cascade on delete; `sonarr_mapping`
    /// nulls out. Any indexed files are unmapped (and their stale confidence
    /// cleared) so they resurface as Unmapped rather than pointing at a ghost id.
    pub async fn delete_anime(&self, anime_id: i64) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE file_index SET anime_id = NULL, confidence = 0 WHERE anime_id = ?1")
            .bind(anime_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM anime WHERE id = ?1")
            .bind(anime_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    // ── Sonarr series ───────────────────────────────────────────────────────────

    pub async fn sonarr_series_upsert(&self, series: &SonarrSeriesDb) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO sonarr_series (sonarr_id, title, season_count, episode_count, episode_file_count, monitored, next_airing, path, poster_url, overview, network, status, added, last_synced)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(sonarr_id) DO UPDATE SET
               title = excluded.title,
               season_count = excluded.season_count,
               episode_count = excluded.episode_count,
               episode_file_count = excluded.episode_file_count,
               monitored = excluded.monitored,
               next_airing = excluded.next_airing,
               path = excluded.path,
               poster_url = excluded.poster_url,
               overview = excluded.overview,
               network = excluded.network,
               status = excluded.status,
               last_synced = excluded.last_synced",
        )
        .bind(series.sonarr_id)
        .bind(&series.title)
        .bind(series.season_count)
        .bind(series.episode_count)
        .bind(series.episode_file_count)
        .bind(series.monitored)
        .bind(series.next_airing)
        .bind(&series.path)
        .bind(&series.poster_url)
        .bind(&series.overview)
        .bind(&series.network)
        .bind(&series.status)
        .bind(series.added)
        .bind(series.last_synced)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn sonarr_series_list(&self) -> anyhow::Result<Vec<SonarrSeriesDb>> {
        let rows = sqlx::query(
            "SELECT sonarr_id, title, season_count, episode_count, episode_file_count, monitored, next_airing, path, poster_url, overview, network, status, added, last_synced
             FROM sonarr_series ORDER BY title",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|row| SonarrSeriesDb {
            sonarr_id: row.get("sonarr_id"),
            title: row.get("title"),
            season_count: row.get("season_count"),
            episode_count: row.get("episode_count"),
            episode_file_count: row.get("episode_file_count"),
            monitored: row.get("monitored"),
            next_airing: row.get("next_airing"),
            path: row.get("path"),
            poster_url: row.get("poster_url"),
            overview: row.get("overview"),
            network: row.get("network"),
            status: row.get("status"),
            added: row.get("added"),
            last_synced: row.get("last_synced"),
        }).collect())
    }

    pub async fn sonarr_series_count(&self) -> anyhow::Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) FROM sonarr_series")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>(0))
    }

    pub async fn sonarr_series_delete_all(&self) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM sonarr_series").execute(&self.pool).await?;
        Ok(())
    }

    // ── Sonarr mapping ──────────────────────────────────────────────────────────

    pub async fn sonarr_mapping_upsert(&self, mapping: &SonarrMappingDb) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO sonarr_mapping (sonarr_id, anime_id, title_match, confidence, mapped_at, user_confirmed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(sonarr_id) DO UPDATE SET
               anime_id = excluded.anime_id,
               title_match = excluded.title_match,
               confidence = excluded.confidence,
               mapped_at = excluded.mapped_at,
               user_confirmed = excluded.user_confirmed",
        )
        .bind(mapping.sonarr_id)
        .bind(mapping.anime_id)
        .bind(&mapping.title_match)
        .bind(mapping.confidence)
        .bind(mapping.mapped_at)
        .bind(mapping.user_confirmed)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn sonarr_mapping_by_anime(&self, anime_id: i64) -> anyhow::Result<Option<SonarrMappingDb>> {
        let row = sqlx::query(
            "SELECT id, sonarr_id, anime_id, title_match, confidence, mapped_at, user_confirmed
             FROM sonarr_mapping WHERE anime_id = ?1",
        )
        .bind(anime_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| SonarrMappingDb {
            id: Some(r.get("id")),
            sonarr_id: r.get("sonarr_id"),
            anime_id: r.get("anime_id"),
            title_match: r.get("title_match"),
            confidence: r.get("confidence"),
            mapped_at: r.get("mapped_at"),
            user_confirmed: r.get("user_confirmed"),
        }))
    }

    pub async fn sonarr_mapping_by_sonarr_id(&self, sonarr_id: i64) -> anyhow::Result<Option<SonarrMappingDb>> {
        let row = sqlx::query(
            "SELECT id, sonarr_id, anime_id, title_match, confidence, mapped_at, user_confirmed
             FROM sonarr_mapping WHERE sonarr_id = ?1",
        )
        .bind(sonarr_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| SonarrMappingDb {
            id: Some(r.get("id")),
            sonarr_id: r.get("sonarr_id"),
            anime_id: r.get("anime_id"),
            title_match: r.get("title_match"),
            confidence: r.get("confidence"),
            mapped_at: r.get("mapped_at"),
            user_confirmed: r.get("user_confirmed"),
        }))
    }

    pub async fn sonarr_mapping_unmapped(&self) -> anyhow::Result<Vec<SonarrMappingDb>> {
        let rows = sqlx::query(
            "SELECT id, sonarr_id, anime_id, title_match, confidence, mapped_at, user_confirmed
             FROM sonarr_mapping WHERE anime_id IS NULL ORDER BY title_match",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| SonarrMappingDb {
            id: Some(r.get("id")),
            sonarr_id: r.get("sonarr_id"),
            anime_id: r.get("anime_id"),
            title_match: r.get("title_match"),
            confidence: r.get("confidence"),
            mapped_at: r.get("mapped_at"),
            user_confirmed: r.get("user_confirmed"),
        }).collect())
    }

    pub async fn sonarr_mapping_count(&self) -> anyhow::Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) FROM sonarr_mapping")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>(0))
    }

    /// Remove Sonarr series that are no longer wanted (e.g. lost their tag).
    /// Passing an empty slice clears them all. Mappings cascade on delete.
    pub async fn prune_sonarr_series_except(&self, keep_ids: &[i64]) -> anyhow::Result<()> {
        if keep_ids.is_empty() {
            sqlx::query("DELETE FROM sonarr_series").execute(&self.pool).await?;
            return Ok(());
        }
        let placeholders = (1..=keep_ids.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!("DELETE FROM sonarr_series WHERE sonarr_id NOT IN ({placeholders})");
        let mut q = sqlx::query(&sql);
        for id in keep_ids {
            q = q.bind(id);
        }
        q.execute(&self.pool).await?;
        Ok(())
    }

    /// List every imported Sonarr series with its current anime mapping (if any),
    /// for the manual-mapping UI.
    pub async fn list_sonarr_series(&self) -> anyhow::Result<Vec<SonarrSeriesListRow>> {
        let rows = sqlx::query(
            "SELECT s.sonarr_id, s.title, s.poster_url, s.episode_count, \
                    m.anime_id, m.confidence, \
                    COALESCE(NULLIF(json_extract(a.titles_json, '$.english'), ''), json_extract(a.titles_json, '$.romaji')) as anime_title \
             FROM sonarr_series s \
             LEFT JOIN sonarr_mapping m ON m.sonarr_id = s.sonarr_id \
             LEFT JOIN anime a ON a.id = m.anime_id \
             ORDER BY s.title COLLATE NOCASE",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| SonarrSeriesListRow {
                sonarr_id: r.get("sonarr_id"),
                title: r.get("title"),
                poster_url: r.get("poster_url"),
                episode_count: r.get("episode_count"),
                anime_id: r.get("anime_id"),
                confidence: r.get("confidence"),
                anime_title: r.get("anime_title"),
            })
            .collect())
    }

    pub async fn sonarr_mapping_delete_all(&self) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM sonarr_mapping").execute(&self.pool).await?;
        Ok(())
    }

    // ── Sonarr availability (join) ──────────────────────────────────────────────

    pub async fn sonarr_availability(&self, anime_id: i64) -> anyhow::Result<Option<SonarrAvailabilityDb>> {
        let row = sqlx::query(
            "SELECT s.sonarr_id, s.title, s.monitored, s.episode_count, s.episode_file_count,
                    s.next_airing, s.path, s.season_count, s.status as sonarr_status
             FROM sonarr_series s
             JOIN sonarr_mapping m ON s.sonarr_id = m.sonarr_id
             WHERE m.anime_id = ?1",
        )
        .bind(anime_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| SonarrAvailabilityDb {
            sonarr_id: r.get("sonarr_id"),
            sonarr_title: r.get("title"),
            monitored: r.get("monitored"),
            episode_count: r.get("episode_count"),
            episode_file_count: r.get("episode_file_count"),
            next_airing: r.get("next_airing"),
            path: r.get("path"),
            season_count: r.get("season_count"),
            sonarr_status: r.get("sonarr_status"),
        }))
    }

    pub fn database_path(&self) -> String {
        self.database_path.clone()
    }

    pub async fn wal_checkpoint(&self) -> anyhow::Result<()> {
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn log_migration(&self, source: &str, source_id: &str, status: &str, message: &str) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        sqlx::query(
            "INSERT INTO migration_log (source, source_id, status, message, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(source)
        .bind(source_id)
        .bind(status)
        .bind(message)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn export_all_anime(&self) -> anyhow::Result<Vec<crate::engine::migration::backup::AnimeExport>> {
        use crate::engine::migration::backup::AnimeExport;
        let rows = sqlx::query(
            "SELECT id, titles_json, type, status, episode_count, image_url, synopsis, last_modified FROM anime ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| AnimeExport {
            id: r.get("id"),
            titles_json: r.get("titles_json"),
            anime_type: r.get("type"),
            status: r.get("status"),
            episode_count: r.get("episode_count"),
            image_url: r.get("image_url"),
            synopsis: r.get("synopsis"),
            last_modified: r.get("last_modified"),
        }).collect())
    }

    pub async fn export_all_list_entries(&self) -> anyhow::Result<Vec<crate::engine::migration::backup::ListEntryExport>> {
        use crate::engine::migration::backup::ListEntryExport;
        let rows = sqlx::query(
            "SELECT anime_id, status, watched_episodes, score, notes, date_started, date_completed, local_updated, remote_updated FROM list_entry ORDER BY anime_id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| ListEntryExport {
            anime_id: r.get("anime_id"),
            status: r.get("status"),
            watched_episodes: r.get("watched_episodes"),
            score: r.get("score"),
            notes: r.get("notes"),
            date_started: r.get("date_started"),
            date_completed: r.get("date_completed"),
            local_updated: r.get("local_updated"),
            remote_updated: r.get("remote_updated"),
        }).collect())
    }

    pub async fn compute_stats(&self) -> anyhow::Result<AnimeStats> {
        let score_rows = sqlx::query(
            "SELECT \
               CASE WHEN score IS NULL THEN -1 \
                    WHEN score < 10 THEN 0 \
                    WHEN score < 30 THEN 1 \
                    WHEN score < 50 THEN 2 \
                    WHEN score < 70 THEN 3 \
                    WHEN score < 90 THEN 4 \
                    ELSE 5 END as bucket, \
               COUNT(*) as cnt \
             FROM list_entry \
             WHERE score IS NOT NULL AND score > 0 \
             GROUP BY bucket \
             ORDER BY bucket"
        ).fetch_all(&self.pool).await?;

        let bucket_labels = ["0-9", "10-29", "30-49", "50-69", "70-89", "90-100"];
        let mut score_distribution: Vec<ScoreBucket> = bucket_labels.iter().map(|r| ScoreBucket { range: r.to_string(), count: 0 }).collect();
        for row in &score_rows {
            let bucket: i32 = row.get("bucket");
            let cnt: i64 = row.get("cnt");
            if bucket >= 0 && (bucket as usize) < score_distribution.len() {
                score_distribution[bucket as usize].count = cnt;
            }
        }

        let total_anime: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM list_entry").fetch_one(&self.pool).await?;
        let total_eps: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(watched_episodes), 0) FROM list_entry").fetch_one(&self.pool).await?;
        let total_rewatches: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM watch_history WHERE source = 'manual' OR source = 'auto-detect'").fetch_one(&self.pool).await?;
        let avg_score: f64 = sqlx::query_scalar("SELECT COALESCE(AVG(CAST(score AS REAL)), 0) FROM list_entry WHERE score IS NOT NULL AND score > 0").fetch_one(&self.pool).await?;

        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
        let day_ago = now - 86400;
        let week_ago = now - 604800;

        let episodes_today: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM watch_history WHERE watched_at > ?1").bind(day_ago).fetch_one(&self.pool).await?;
        let episodes_this_week: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM watch_history WHERE watched_at > ?1").bind(week_ago).fetch_one(&self.pool).await?;

        Ok(AnimeStats { score_distribution, total_anime, total_episodes_watched: total_eps, total_rewatches, avg_score, episodes_today, episodes_this_week })
    }

    pub async fn all_library_anime_ids(&self) -> anyhow::Result<Vec<i64>> {
        let rows = sqlx::query("SELECT anime_id FROM list_entry").fetch_all(&self.pool).await?;
        Ok(rows.iter().map(|r| r.get::<i64, _>("anime_id")).collect())
    }

    /// Anime ids eligible for the airing calendar: currently watching or planned.
    pub async fn calendar_anime_ids(&self) -> anyhow::Result<Vec<i64>> {
        let rows = sqlx::query(
            "SELECT anime_id FROM list_entry WHERE status IN ('watching', 'plan_to_watch') ORDER BY anime_id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| r.get::<i64, _>("anime_id")).collect())
    }

    /// Library anime (in the user's list) whose episode count or airing status is
    /// still unknown — candidates for a metadata backfill from AniList.
    pub async fn library_anime_missing_meta(&self, limit: i64) -> anyhow::Result<Vec<i64>> {
        let rows = sqlx::query(
            "SELECT a.id FROM anime a \
             JOIN list_entry le ON a.id = le.anime_id \
             WHERE a.episode_count IS NULL OR a.episode_count = 0 OR a.status IS NULL \
             ORDER BY a.id LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| r.get::<i64, _>("id")).collect())
    }

    /// Fill in an anime's episode count / airing status without clobbering values
    /// that are already set (COALESCE keeps existing when the fetched value is None).
    pub async fn update_anime_episode_meta(
        &self,
        id: i64,
        episode_count: Option<i32>,
        status: Option<&str>,
        now: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE anime SET \
               episode_count = COALESCE(?2, episode_count), \
               status = COALESCE(?3, status), \
               last_modified = ?4 \
             WHERE id = ?1",
        )
        .bind(id)
        .bind(episode_count)
        .bind(status)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Word-wildcard search: builds %word1%word2%word3% pattern for LIKE.
    /// Useful when punctuation differences prevent exact substring matching.
    pub async fn search_anime_by_words(&self, words: &[&str], limit: i64) -> anyhow::Result<Vec<AnimeRow>> {
        if words.is_empty() { return Ok(vec![]); }
        let pattern = format!("%{}%", words.join("%"));
        let rows = sqlx::query("SELECT id, titles_json, episode_count FROM anime WHERE titles_json LIKE ?1 ORDER BY id LIMIT ?2")
            .bind(&pattern).bind(limit).fetch_all(&self.pool).await?;
        Ok(rows.iter().map(|row| AnimeRow {
            id: row.get("id"),
            titles_json: row.get("titles_json"),
            episode_count: row.get("episode_count"),
        }).collect())
    }

    pub async fn watching_anime_ids(&self) -> anyhow::Result<Vec<i64>> {
        let rows = sqlx::query(
            "SELECT anime_id FROM list_entry WHERE status = 'watching' ORDER BY anime_id"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| r.get::<i64, _>("anime_id")).collect())
    }

    pub async fn continue_watching(&self, limit: i64) -> anyhow::Result<Vec<ContinueWatchingRow>> {
        let rows = sqlx::query(
            "SELECT a.id as anime_id, \
             COALESCE(NULLIF(json_extract(a.titles_json, '$.english'), ''), json_extract(a.titles_json, '$.romaji'), 'Unknown') as anime_title, \
             a.image_url, \
             COALESCE(le.watched_episodes, 0) as watched_episodes, \
             a.episode_count, \
             MAX(wh.watched_at) as last_watched_at \
             FROM watch_history wh \
             JOIN anime a ON wh.anime_id = a.id \
             LEFT JOIN list_entry le ON a.id = le.anime_id \
             WHERE le.status = 'watching' \
             GROUP BY a.id \
             ORDER BY last_watched_at DESC \
             LIMIT ?1"
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| ContinueWatchingRow {
            anime_id: r.get("anime_id"),
            anime_title: r.get("anime_title"),
            image_url: r.get("image_url"),
            watched_episodes: r.get("watched_episodes"),
            episode_count: r.get("episode_count"),
            last_watched_at: r.get("last_watched_at"),
        }).collect())
    }

    pub async fn export_all_watch_history(&self) -> anyhow::Result<Vec<crate::engine::migration::backup::WatchHistoryExport>> {
        use crate::engine::migration::backup::WatchHistoryExport;
        let rows = sqlx::query(
            "SELECT anime_id, episode, file_path, player, watched_at, source FROM watch_history ORDER BY watched_at",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| WatchHistoryExport {
            anime_id: r.get("anime_id"),
            episode: r.get("episode"),
            file_path: r.get("file_path"),
            player: r.get("player"),
            watched_at: r.get("watched_at"),
            source: r.get::<String, _>("source"),
        }).collect())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContinueWatchingRow {
    pub anime_id: i64,
    pub anime_title: String,
    pub image_url: Option<String>,
    pub watched_episodes: i32,
    pub episode_count: Option<i32>,
    pub last_watched_at: i64,
}

pub struct Tests;

impl Tests {
    pub async fn new_in_memory() -> Storage {
        let storage = Storage::connect("sqlite::memory:").await.unwrap();
        storage.migrate().await.unwrap();
        storage
    }
}
