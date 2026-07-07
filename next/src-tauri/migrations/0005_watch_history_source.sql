-- Rebrand the legacy default watch-history source. Rows recorded before the
-- app set an explicit source fell back to the table default 'taiga_next',
-- which then surfaced on the History page. Code now always passes an explicit
-- source ('manual' / 'auto-detect' / 'import'); relabel the old rows so no
-- watch record shows the old project name.
UPDATE watch_history SET source = 'anivault' WHERE source = 'taiga_next';
