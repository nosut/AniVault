-- Links an AniList entry to the Sonarr series that covers it (many entries can
-- share one series, e.g. seasons), or marks it ignored for the coverage check.
CREATE TABLE IF NOT EXISTS sonarr_coverage_link (
    anime_id    INTEGER PRIMARY KEY REFERENCES anime(id) ON DELETE CASCADE,
    sonarr_id   INTEGER,
    ignored     BOOLEAN NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL
);

-- AniList media format (TV, MOVIE, OVA, ...). `type` is always ANIME.
ALTER TABLE anime ADD COLUMN format TEXT;
