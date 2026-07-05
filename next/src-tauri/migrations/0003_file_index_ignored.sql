-- Persistent "ignore" tombstone for known files. When set, a file is never
-- re-indexed or auto-matched by the library scanner / rematch, but its row is
-- kept so the scanner's "already indexed" check keeps skipping it.
ALTER TABLE file_index ADD COLUMN ignored INTEGER NOT NULL DEFAULT 0;
