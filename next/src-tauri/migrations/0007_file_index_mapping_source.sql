-- Existing mappings predate provenance tracking. Treat them as legacy so they
-- require explicit repair confirmation and are never mistaken for new manual rows.
ALTER TABLE file_index
ADD COLUMN mapping_source TEXT NOT NULL DEFAULT 'legacy';
