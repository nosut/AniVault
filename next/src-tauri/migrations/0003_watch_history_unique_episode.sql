CREATE UNIQUE INDEX IF NOT EXISTS idx_watch_history_anime_episode
ON watch_history(anime_id, episode);
