/// Rate limiter for AniList API: ensures minimum interval between calls.
/// AniList allows ~85 requests per 70 seconds (≈823ms per request).

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct RateLimiter {
    last_call: Arc<Mutex<Option<tokio::time::Instant>>>,
    min_interval: Duration,
}

impl RateLimiter {
    pub fn new(min_interval_ms: u64) -> Self {
        Self {
            last_call: Arc::new(Mutex::new(None)),
            min_interval: Duration::from_millis(min_interval_ms),
        }
    }

    pub async fn acquire(&self) {
        let mut last = self.last_call.lock().await;
        let now = tokio::time::Instant::now();

        if let Some(last_time) = *last {
            let elapsed = now.duration_since(last_time);
            if elapsed < self.min_interval {
                drop(last);
                tokio::time::sleep(self.min_interval - elapsed).await;
                let mut last = self.last_call.lock().await;
                *last = Some(tokio::time::Instant::now());
                return;
            }
        }

        *last = Some(now);
    }
}

/// Global AniList limiter: 825ms between calls (≈85 req / 70 sec).
pub fn anilist_limiter() -> RateLimiter {
    RateLimiter::new(825)
}
