// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { flushSync, tick } from 'svelte';

function show(anime_id: number, title: string) {
  return {
    anime_id, title, image_url: null, status: 'completed', watched_episodes: 12, episode_count: 12,
    downloaded_count: 12, max_downloaded_episode: 12, next_unwatched_episode: null,
    next_episode_path: null, new_count: 0, last_indexed_at: anime_id,
  };
}

vi.mock('./api', () => ({
  getCollection: vi.fn(),
  getEpisodeFiles: vi.fn(async () => []),
  deleteAnime: vi.fn(async () => {}),
  updateListEntry: vi.fn(async () => {}),
  getSetting: vi.fn(async () => null),
  openEpisodeFile: vi.fn(async () => {}),
  openContainingFolder: vi.fn(async () => {}),
}));

import { createClassComponent } from 'svelte/legacy';
import CollectionView from './CollectionView.svelte';
import { getCollection } from './api';
import { cleanupApplied } from './dropCleanup';
import { resetPrefs } from './prefs';

async function settle() {
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
  flushSync();
}

describe('CollectionView after a clean-up', () => {
  let component: { $destroy: () => void } | undefined;

  beforeEach(() => {
    document.body.innerHTML = '<div id="app"></div>';
    resetPrefs();
  });

  afterEach(() => {
    component?.$destroy();
    component = undefined;
  });

  it('keeps the list on screen while it reloads, and drops the cleaned show', async () => {
    vi.mocked(getCollection).mockResolvedValueOnce([show(1, 'Kept'), show(2, 'Cleaned'), show(3, 'Also Kept')]);
    component = createClassComponent({ component: CollectionView, target: document.getElementById('app')! });
    await settle();
    const cards = () => [...document.querySelectorAll('.poster-card:not(.skeleton-card)')];
    expect(cards()).toHaveLength(3);

    // Hold the reload open to see what's on screen meanwhile.
    let finish!: (v: ReturnType<typeof show>[]) => void;
    vi.mocked(getCollection).mockReturnValueOnce(new Promise((r) => { finish = r; }));
    cleanupApplied.set({ animeIds: [2] });
    cleanupApplied.set(null);
    await settle();
    // Placeholders would collapse the page and reset the scroll position.
    expect(document.querySelector('.skeleton-card')).toBeNull();
    expect(cards()).toHaveLength(3);

    finish([show(1, 'Kept'), show(3, 'Also Kept')]);
    await settle();
    expect(cards().map((c) => c.textContent)).not.toContain(expect.stringContaining('Cleaned'));
    expect(cards()).toHaveLength(2);
  });
});
