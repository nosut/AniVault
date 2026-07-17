// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { mount, unmount, flushSync, tick } from 'svelte';
import { addSeasons } from './seasonUi';

const seasonEntry = {
  id: 1, title: 'Near Show', image_url: null, episodes: 12, status: 'NOT_YET_RELEASED',
  format: 'TV', average_score: null, popularity: 5000,
};
const futureEntry = {
  id: 2, title: 'Far Future Show', image_url: null, episodes: null, status: 'NOT_YET_RELEASED',
  format: 'TV', average_score: null, popularity: 4000, season: null, season_year: null, start_year: null,
};

vi.mock('./api', () => ({
  getSeasonAnime: vi.fn(async () => [seasonEntry]),
  getFutureAnime: vi.fn(async () => [futureEntry]),
  getLibraryIds: vi.fn(async () => []),
  updateListEntry: vi.fn(async () => {}),
  importAnilistAnime: vi.fn(async () => {}),
}));

import { getFutureAnime, getSeasonAnime } from './api';
import SeasonView from './SeasonView.svelte';

// The season 4 ahead of the real current one — the last normally browsable page.
function lastBrowsableSeason(): { season: string; year: number } {
  const now = new Date();
  const m = now.getMonth();
  const s = m < 3 ? 'WINTER' : m < 6 ? 'SPRING' : m < 9 ? 'SUMMER' : 'FALL';
  return addSeasons(s, now.getFullYear(), 4);
}

function headerText(): string {
  return document.querySelector('.season-nav h2')?.textContent ?? '';
}

async function settle() {
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
  flushSync();
}

describe('SeasonView future mode', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    document.body.innerHTML = '<div id="app"></div>';
    localStorage.clear();
    const last = lastBrowsableSeason();
    localStorage.setItem('anivault-season-state', JSON.stringify({ season: last.season, year: last.year, genre: '' }));
  });

  it('switches to Future Seasons after the last browsable season and back', async () => {
    const app = mount(SeasonView, { target: document.getElementById('app')! });
    await settle();

    // On the +4 season: normal browsing.
    expect(getSeasonAnime).toHaveBeenCalled();
    const next = document.querySelector<HTMLButtonElement>('button[aria-label="Next season"]')!;
    next.click();
    flushSync();
    await settle();

    // Now on the Future Seasons page.
    expect(headerText()).toBe('Future Seasons');
    expect(getFutureAnime).toHaveBeenCalled();
    expect(document.body.textContent).toContain('Far Future Show');
    expect(document.body.textContent).toContain('TBA');
    // No further paging forward.
    expect(document.querySelector('button[aria-label="Next season"]')).toBeNull();

    // Paging back returns to the +4 season.
    const prev = document.querySelector<HTMLButtonElement>('button[aria-label="Previous season"]')!;
    prev.click();
    flushSync();
    await settle();
    const last = lastBrowsableSeason();
    expect(headerText()).toContain(String(last.year));
    expect(document.body.textContent).toContain('Near Show');

    await unmount(app);
  });

  it('restores directly into future mode when the saved state is beyond the window', async () => {
    const beyond = addSeasons(lastBrowsableSeason().season, lastBrowsableSeason().year, 3);
    localStorage.setItem('anivault-season-state', JSON.stringify({ season: beyond.season, year: beyond.year, genre: '' }));

    const app = mount(SeasonView, { target: document.getElementById('app')! });
    await settle();

    expect(headerText()).toBe('Future Seasons');
    expect(getFutureAnime).toHaveBeenCalled();
    await unmount(app);
  });
});
