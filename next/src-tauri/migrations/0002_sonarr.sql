CREATE TABLE IF NOT EXISTS sonarr_series (
    sonarr_id            INTEGER PRIMARY KEY,
    title                TEXT NOT NULL,
    season_count         INTEGER NOT NULL DEFAULT 0,
    episode_count        INTEGER NOT NULL DEFAULT 0,
    episode_file_count   INTEGER NOT NULL DEFAULT 0,
    monitored            BOOLEAN NOT NULL DEFAULT 1,
    next_airing          INTEGER,
    path                 TEXT,
    poster_url           TEXT,
    overview             TEXT,
    network              TEXT,
    status               TEXT,
    added                INTEGER NOT NULL,
    last_synced          INTEGER NOT NULL
);

-- Replace the 0001 placeholder sonarr_mapping with the M6 schema
DROP TABLE IF EXISTS sonarr_mapping;
CREATE TABLE sonarr_mapping (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    sonarr_id            INTEGER NOT NULL UNIQUE REFERENCES sonarr_series(sonarr_id) ON DELETE CASCADE,
    anime_id             INTEGER REFERENCES anime(id) ON DELETE SET NULL,
    title_match          TEXT NOT NULL,
    confidence           INTEGER NOT NULL DEFAULT 0,
    mapped_at            INTEGER NOT NULL,
    user_confirmed       BOOLEAN NOT NULL DEFAULT 0
);
