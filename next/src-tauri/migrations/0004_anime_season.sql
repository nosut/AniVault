-- Release season for anime (from AniList `season` / `seasonYear`), used to show
-- what season a Plan to Watch title belongs to.
ALTER TABLE anime ADD COLUMN season TEXT;
ALTER TABLE anime ADD COLUMN season_year INTEGER;
