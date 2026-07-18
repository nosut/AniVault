-- Windows filesystems are case-insensitive, but file_index keys files by the
-- exact path string. A case-only rename on disk therefore accumulated a second
-- row for the same physical file (the old spelling still "exists" to the
-- prune check, so it was never removed). Collapse such case-variant rows,
-- keeping the best one per case-folded path: a mapped row over an unmapped
-- one, then the most recently indexed.
DELETE FROM file_index WHERE rowid IN (
  SELECT rowid FROM (
    SELECT rowid, ROW_NUMBER() OVER (
      PARTITION BY LOWER(file_path)
      ORDER BY (anime_id IS NOT NULL) DESC, indexed_at DESC, rowid DESC
    ) AS rn FROM file_index
  ) WHERE rn > 1
);

-- Case-insensitive lookups (get_file_index, upsert re-pathing) stay indexed.
CREATE INDEX IF NOT EXISTS idx_file_index_path_nocase
  ON file_index (file_path COLLATE NOCASE);
