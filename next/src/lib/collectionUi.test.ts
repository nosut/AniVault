import { describe, it, expect } from 'vitest';
import { isComplete, filterCollection, sortCollection } from './collectionUi';
import type { CollectionEntry } from './api';

function e(p: Partial<CollectionEntry>): CollectionEntry {
  return {
    anime_id: 1, title: 'A', image_url: null, status: 'watching', watched_episodes: 0,
    episode_count: null, downloaded_count: 1, max_downloaded_episode: 1,
    next_unwatched_episode: 1, next_episode_path: null, new_count: 1, last_indexed_at: 0, ...p,
  };
}

describe('isComplete', () => {
  it('is true only when every episode is on disk', () => {
    expect(isComplete(e({ episode_count: 12, max_downloaded_episode: 12 }))).toBe(true);
    expect(isComplete(e({ episode_count: 12, max_downloaded_episode: 8 }))).toBe(false);
    expect(isComplete(e({ episode_count: null, max_downloaded_episode: 8 }))).toBe(false);
  });
});

describe('filterCollection', () => {
  const list = [
    e({ anime_id: 1, title: 'Frieren', episode_count: 4, max_downloaded_episode: 4, new_count: 0 }),
    e({ anime_id: 2, title: 'Spy Family', episode_count: 12, max_downloaded_episode: 6, new_count: 3 }),
  ];
  it('filters by new', () => {
    expect(filterCollection(list, 'new', '').map((x) => x.anime_id)).toEqual([2]);
  });
  it('filters by complete / incomplete', () => {
    expect(filterCollection(list, 'complete', '').map((x) => x.anime_id)).toEqual([1]);
    expect(filterCollection(list, 'incomplete', '').map((x) => x.anime_id)).toEqual([2]);
  });
  it('applies a case-insensitive title query on top of the filter', () => {
    expect(filterCollection(list, 'all', 'spy').map((x) => x.anime_id)).toEqual([2]);
  });
});

describe('sortCollection', () => {
  const list = [
    e({ anime_id: 1, title: 'Bravo', last_indexed_at: 100, watched_episodes: 5 }),
    e({ anime_id: 2, title: 'Alpha', last_indexed_at: 300, watched_episodes: 1 }),
  ];
  it('recent sorts by last_indexed_at desc', () => {
    expect(sortCollection(list, 'recent').map((x) => x.anime_id)).toEqual([2, 1]);
  });
  it('title sorts alphabetically', () => {
    expect(sortCollection(list, 'title').map((x) => x.anime_id)).toEqual([2, 1]);
  });
  it('progress sorts by watched desc', () => {
    expect(sortCollection(list, 'progress').map((x) => x.anime_id)).toEqual([1, 2]);
  });
  it('does not mutate its input array', () => {
    const before = [...list];
    sortCollection(list, 'title');
    expect(list).toEqual(before);
  });
});
