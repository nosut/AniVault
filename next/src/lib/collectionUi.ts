import type { CollectionEntry } from './api';

export type CollectionFilter = 'all' | 'new' | 'complete' | 'incomplete';

/** Every episode of a known-length series is on disk. */
export function isComplete(e: CollectionEntry): boolean {
  return e.episode_count != null && e.episode_count > 0 && e.max_downloaded_episode >= e.episode_count;
}

export function filterCollection(
  entries: CollectionEntry[],
  filter: CollectionFilter,
  query: string,
): CollectionEntry[] {
  const q = query.trim().toLowerCase();
  return entries.filter((e) => {
    if (q && !e.title.toLowerCase().includes(q)) return false;
    switch (filter) {
      case 'new': return e.new_count > 0;
      case 'complete': return isComplete(e);
      case 'incomplete': return !isComplete(e);
      default: return true;
    }
  });
}


