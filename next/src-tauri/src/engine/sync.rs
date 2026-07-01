/// Sync queue operations: queue, drain, complete, reschedule with backoff.
/// SyncManager: Tokio worker loop that pushes progress to AniList.

use sqlx::Row;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::engine::storage::Storage;

// ── Queue operations ──

#[derive(Debug, Clone)]
pub struct SyncItem {
    pub queue_id: i64,
    pub anime_id: i64,
    pub operation: String,
    pub payload_json: String,
    pub retry_count: i32,
}

pub async fn queue_sync_results(
    storage: &Storage,
    anime_id: i64,
    episode: i32,
    watched_at_unix: i64,
    now: i64,
) -> anyhow::Result<i64> {
    let payload = serde_json::json!({ "episode": episode, "watched_at": watched_at_unix }).to_string();
    let id = storage
        .queue_sync(anime_id, "anilist", "update_progress", &payload, now)
        .await?;
    Ok(id)
}

pub async fn pending_sync_batch(
    storage: &Storage,
    service: &str,
    limit: usize,
    now: i64,
) -> anyhow::Result<Vec<SyncItem>> {
    let rows = sqlx::query(
        "SELECT id, anime_id, operation, payload_json, retry_count
         FROM sync_queue
         WHERE service = ?1 AND (next_retry_at IS NULL OR next_retry_at <= ?2)
         ORDER BY created_at ASC
         LIMIT ?3",
    )
    .bind(service)
    .bind(now)
    .bind(limit as i64)
    .fetch_all(storage.pool())
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| SyncItem {
            queue_id: r.get(0),
            anime_id: r.get(1),
            operation: r.get(2),
            payload_json: r.get(3),
            retry_count: r.get(4),
        })
        .collect())
}

pub async fn complete_sync_item(storage: &Storage, queue_id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM sync_queue WHERE id = ?1")
        .bind(queue_id)
        .execute(storage.pool())
        .await?;
    Ok(())
}

pub async fn reschedule_sync_item(storage: &Storage, queue_id: i64, now: i64) -> anyhow::Result<()> {
    let current_retry: i32 = sqlx::query("SELECT retry_count FROM sync_queue WHERE id = ?1")
        .bind(queue_id)
        .fetch_one(storage.pool())
        .await?
        .get(0);

    let next_retry_count = current_retry + 1;
    let delay = backoff_delay(next_retry_count);
    let next_at = now + delay;

    sqlx::query(
        "UPDATE sync_queue SET retry_count = ?1, next_retry_at = ?2 WHERE id = ?3",
    )
    .bind(next_retry_count)
    .bind(next_at)
    .bind(queue_id)
    .execute(storage.pool())
    .await?;

    Ok(())
}

pub fn backoff_delay(retry_count: i32) -> i64 {
    let seconds = (1u64 << retry_count.min(10) as u64) * 30;
    seconds.min(21_600) as i64 // max 6 hours
}

// ── SyncManager ──

pub struct SyncManager {
    running: Arc<AtomicBool>,
}

impl SyncManager {
    pub fn start(storage: Storage) -> Self {
        let running = Arc::new(AtomicBool::new(true));

        let r = running.clone();
        tokio::spawn(async move {
            let mut idle_count: u32 = 0;
            while r.load(Ordering::Relaxed) {
                let items = pending_sync_batch(&storage, "anilist", 5, now_unix())
                    .await
                    .unwrap_or_default();

                if items.is_empty() {
                    idle_count += 1;
                    let sleep = if idle_count > 10 {
                        Duration::from_secs(30)
                    } else {
                        Duration::from_secs(2)
                    };
                    tokio::time::sleep(sleep).await;
                    continue;
                }

                idle_count = 0;

                for item in &items {
                    let Ok(token) =
                        crate::engine::oauth::load_oauth_token(&storage).await
                    else {
                        // No token yet, skip
                        continue;
                    };

                    let Some(token) = token else {
                        continue;
                    };

                    match push_progress_to_anilist(&token.access_token, item).await {
                        Ok(()) => {
                            let _ = complete_sync_item(&storage, item.queue_id).await;
                        }
                        Err(e) => {
                            eprintln!("sync failed for anime {}: {e}", item.anime_id);
                            let _ =
                                reschedule_sync_item(&storage, item.queue_id, now_unix()).await;
                        }
                    }
                }
            }
        });

        Self { running }
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

async fn push_progress_to_anilist(access_token: &str, item: &SyncItem) -> anyhow::Result<()> {
    use crate::engine::rate_limit::anilist_limiter;
    anilist_limiter().acquire().await;

    let payload: serde_json::Value = serde_json::from_str(&item.payload_json)?;
    let episode = payload["episode"].as_i64().unwrap_or(0) as i32;

    let client = reqwest::Client::new();
    let resp = client
        .post("https://graphql.anilist.co")
        .header("Authorization", format!("Bearer {}", access_token))
        .json(&serde_json::json!({
            "query": "mutation ($mediaId: Int, $progress: Int) { SaveMediaListEntry (mediaId: $mediaId, progress: $progress) { id progress } }",
            "variables": {
                "mediaId": item.anime_id,
                "progress": episode
            }
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("AniList HTTP {}", resp.status());
    }

    let body: serde_json::Value = resp.json().await?;
    if body.get("errors").is_some() {
        anyhow::bail!("AniList GraphQL error: {body}");
    }

    Ok(())
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
