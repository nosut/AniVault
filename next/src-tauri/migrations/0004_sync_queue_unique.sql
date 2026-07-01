CREATE UNIQUE INDEX IF NOT EXISTS idx_sync_queue_anime_ep_operation
ON sync_queue(anime_id, operation, payload_json);
