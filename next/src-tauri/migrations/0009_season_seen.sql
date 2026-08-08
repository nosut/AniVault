-- Which shows a season was already known to contain the last time it was viewed.
-- The Seasons page fetches from AniList live and keeps no other memory of a
-- season, so diffing a fresh listing against these rows is the only way to know
-- what was added since the last visit.
--
-- Deliberately no foreign key to `anime`: season listings are AniList ids the
-- user has not imported, so an FK would reject exactly the rows this table
-- exists to hold.
--
-- The Future Seasons page has no season of its own and is stored under the
-- sentinel key ('__FUTURE__', 0).
CREATE TABLE IF NOT EXISTS season_seen (
  season        TEXT    NOT NULL,
  year          INTEGER NOT NULL,
  anime_id      INTEGER NOT NULL,
  first_seen_at INTEGER NOT NULL,
  PRIMARY KEY (season, year, anime_id)
);
