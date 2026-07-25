import type { CollectionEntry } from './api';

export type CollectionFilter = 'all' | 'new' | 'complete' | 'incomplete';
export type CollectionSort = 'recent' | 'title' | 'progress';

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

export function sortCollection(entries: CollectionEntry[], sort: CollectionSort): CollectionEntry[] {
  const list = [...entries];
  switch (sort) {
    case 'title': list.sort((a, b) => a.title.localeCompare(b.title)); break;
    case 'progress': list.sort((a, b) => b.watched_episodes - a.watched_episodes); break;
    case 'recent':
    default: list.sort((a, b) => b.last_indexed_at - a.last_indexed_at); break;
  }
  return list;
}
