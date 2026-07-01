CREATE VIRTUAL TABLE IF NOT EXISTS anime_fts USING fts5(
  anime_id UNINDEXED,
  title,
  synonyms
);

CREATE TRIGGER IF NOT EXISTS anime_ai AFTER INSERT ON anime BEGIN
  INSERT INTO anime_fts(anime_id, title, synonyms)
  VALUES (
    new.id,
    COALESCE(json_extract(new.titles_json, '$.romaji'), ''),
    COALESCE(json_extract(new.titles_json, '$.english'), '') || ' ' ||
    COALESCE((SELECT group_concat(value, ' ') FROM json_each(json_extract(new.titles_json, '$.synonyms'))), '')
  );
END;

CREATE TRIGGER IF NOT EXISTS anime_ad AFTER DELETE ON anime BEGIN
  DELETE FROM anime_fts WHERE anime_id = old.id;
END;

CREATE TRIGGER IF NOT EXISTS anime_au AFTER UPDATE ON anime BEGIN
  DELETE FROM anime_fts WHERE anime_id = old.id;
  INSERT INTO anime_fts(anime_id, title, synonyms)
  VALUES (
    new.id,
    COALESCE(json_extract(new.titles_json, '$.romaji'), ''),
    COALESCE(json_extract(new.titles_json, '$.english'), '') || ' ' ||
    COALESCE((SELECT group_concat(value, ' ') FROM json_each(json_extract(new.titles_json, '$.synonyms'))), '')
  );
END;
