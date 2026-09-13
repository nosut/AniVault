/** The tag labels the Sonarr import uses when `sonarr.wanted_tags` is unset (mirrors `WANTED_TAGS` in `sonarr/import.rs`). */
export const DEFAULT_SONARR_WANTED_TAGS = ['1 - nosut', 'mine'];

/**
 * Parse the comma-separated wanted-tags input. Returns `null` for an empty
 * input, meaning "no preference": the setting is removed and the default applies.
 * Repeats are dropped case-insensitively, keeping the first spelling.
 */
export function parseWantedTags(text: string): string[] | null {
  const seen = new Set<string>();
  const tags: string[] = [];
  for (const raw of text.split(',')) {
    const tag = raw.trim();
    if (!tag || seen.has(tag.toLowerCase())) continue;
    seen.add(tag.toLowerCase());
    tags.push(tag);
  }
  return tags.length > 0 ? tags : null;
}

export function formatWantedTags(tags: string[] | null): string {
  return (tags ?? []).join(', ');
}
