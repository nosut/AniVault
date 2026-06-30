CREATE TABLE IF NOT EXISTS anime (
  id INTEGER PRIMARY KEY,
  titles_json TEXT NOT NULL,
  type TEXT,
  status TEXT,
  episode_count INTEGER,
  image_url TEXT,
  synopsis TEXT,
  last_modified INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS list_entry (
  anime_id INTEGER PRIMARY KEY REFERENCES anime(id) ON DELETE CASCADE,
  status TEXT NOT NULL,
  watched_episodes INTEGER NOT NULL DEFAULT 0,
  score INTEGER,
  notes TEXT,
  date_started TEXT,
  date_completed TEXT,
  local_updated INTEGER NOT NULL,
  remote_updated INTEGER
);

CREATE TABLE IF NOT EXISTS watch_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  anime_id INTEGER NOT NULL REFERENCES anime(id) ON DELETE CASCADE,
  episode INTEGER NOT NULL,
  file_path TEXT,
  player TEXT,
  watched_at INTEGER NOT NULL,
  source TEXT NOT NULL DEFAULT 'taiga_next'
);

CREATE TABLE IF NOT EXISTS tracker_mapping (
  anime_id INTEGER NOT NULL REFERENCES anime(id) ON DELETE CASCADE,
  service TEXT NOT NULL,
  remote_id TEXT NOT NULL,
  PRIMARY KEY (anime_id, service)
);

CREATE TABLE IF NOT EXISTS file_index (
  file_path TEXT PRIMARY KEY,
  anime_id INTEGER REFERENCES anime(id) ON DELETE SET NULL,
  episode INTEGER,
  confidence INTEGER NOT NULL DEFAULT 0,
  indexed_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_queue (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  anime_id INTEGER NOT NULL REFERENCES anime(id) ON DELETE CASCADE,
  service TEXT NOT NULL,
  operation TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  retry_count INTEGER NOT NULL DEFAULT 0,
  next_retry_at INTEGER
);

CREATE TABLE IF NOT EXISTS settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS migration_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  source TEXT NOT NULL,
  source_id TEXT,
  status TEXT NOT NULL,
  message TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sonarr_mapping (
  anime_id INTEGER PRIMARY KEY REFERENCES anime(id) ON DELETE CASCADE,
  sonarr_series_id INTEGER NOT NULL,
  sonarr_title TEXT NOT NULL,
  monitored INTEGER NOT NULL DEFAULT 0,
  updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS integration_queue (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  integration TEXT NOT NULL,
  operation TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  retry_count INTEGER NOT NULL DEFAULT 0,
  next_retry_at INTEGER
);
