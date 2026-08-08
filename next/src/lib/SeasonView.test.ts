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
  diffSeason: vi.fn(async () => ({ first_visit: true, new_ids: [] })),
  FUTURE_SEASON_KEY: '__FUTURE__',
}));

import { getFutureAnime, getSeasonAnime, diffSeason, importAnilistAnime } from './api';
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

  it('jumps back to the current season from the future page and hides the button there', async () => {
    const app = mount(SeasonView, { target: document.getElementById('app')! });
    await settle();

    // Saved state is the +4 season, so the Current button is offered.
    const current = document.querySelector<HTMLButtonElement>('button[aria-label="Go to current season"]')!;
    expect(current).not.toBeNull();
    current.click();
    flushSync();
    await settle();

    const now = new Date();
    const m = now.getMonth();
    const label = m < 3 ? 'Winter' : m < 6 ? 'Spring' : m < 9 ? 'Summer' : 'Fall';
    expect(headerText()).toBe(`${label} ${now.getFullYear()}`);
    // Already current: nothing to jump to.
    expect(document.querySelector('button[aria-label="Go to current season"]')).toBeNull();

    await unmount(app);
  });
});

describe('SeasonView new-releases band', () => {
  const entries = [
    { ...seasonEntry, id: 1, title: 'Established Show' },
    { ...seasonEntry, id: 7, title: 'Brand New Show' },
  ];

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(getSeasonAnime).mockResolvedValue(entries as never);
    vi.mocked(diffSeason).mockResolvedValue({ first_visit: false, new_ids: [7] });
    localStorage.clear();
    document.body.innerHTML = '';
  });

  it('groups new shows in a band and keeps them out of the main grid', async () => {
    const target = document.createElement('div');
    document.body.appendChild(target);
    const component = mount(SeasonView, { target });
    await settle();

    const band = document.querySelector('.new-band');
    expect(band, 'the band renders when something is new').toBeTruthy();
    expect(band?.querySelector('.group-count')?.textContent).toBe('1');

    const bandTitles = [...(band?.querySelectorAll('.poster-title') ?? [])].map((n) => n.textContent);
    expect(bandTitles).toEqual(['Brand New Show']);

    // The flagged show must appear exactly once on the page.
    const allTitles = [...document.querySelectorAll('.poster-title')].map((n) => n.textContent);
    expect(allTitles.filter((t) => t === 'Brand New Show')).toHaveLength(1);
    expect(allTitles).toContain('Established Show');

    unmount(component);
  });

  it('renders no band on a first visit', async () => {
    vi.mocked(diffSeason).mockResolvedValue({ first_visit: true, new_ids: [] });
    const target = document.createElement('div');
    document.body.appendChild(target);
    const component = mount(SeasonView, { target });
    await settle();

    expect(document.querySelector('.new-band')).toBeNull();
    expect(document.querySelector('.rest-head')).toBeNull();
    expect([...document.querySelectorAll('.poster-title')]).toHaveLength(2);

    unmount(component);
  });

  it('still renders the season when the diff call fails', async () => {
    // Newness is a convenience over a live API call and must never be able to
    // block the grid.
    vi.mocked(diffSeason).mockRejectedValue(new Error('db locked'));
    const target = document.createElement('div');
    document.body.appendChild(target);
    const component = mount(SeasonView, { target });
    await settle();

    expect(document.querySelector('.new-band')).toBeNull();
    expect([...document.querySelectorAll('.poster-title')]).toHaveLength(2);
    expect(document.querySelector('.message.error')).toBeNull();

    unmount(component);
  });

  it('does not record a baseline while a genre filter is active', async () => {
    const target = document.createElement('div');
    document.body.appendChild(target);
    const component = mount(SeasonView, { target });
    await settle();
    expect(vi.mocked(diffSeason).mock.calls[0]?.[3]).toBe(true);

    vi.mocked(diffSeason).mockClear();
    const select = document.querySelector('.genre-select') as HTMLSelectElement;
    select.value = 'Mecha';
    select.dispatchEvent(new Event('change'));
    await settle();

    expect(vi.mocked(diffSeason).mock.calls[0]?.[3]).toBe(false);

    unmount(component);
  });

  it('dispatches select when a card in the band is clicked', async () => {
    const onSelect = vi.fn();
    const target = document.createElement('div');
    document.body.appendChild(target);
    const component = mount(SeasonView, { target, events: { select: onSelect } });
    await settle();

    const bandCard = document.querySelector('.new-band .poster-card') as HTMLElement;
    bandCard.click();
    await settle();

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect.mock.calls[0]?.[0]?.detail).toEqual({ anime_id: 7 });

    unmount(component);
  });

  it('dispatches select when a card in the lower grid is clicked', async () => {
    const onSelect = vi.fn();
    const target = document.createElement('div');
    document.body.appendChild(target);
    const component = mount(SeasonView, { target, events: { select: onSelect } });
    await settle();

    // The "rest" grid is the .poster-grid that sits directly under .season-view;
    // the band's own .poster-grid is nested inside .new-band.
    const restCard = document.querySelector('.season-view > .poster-grid .poster-card') as HTMLElement;
    restCard.click();
    await settle();

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect.mock.calls[0]?.[0]?.detail).toEqual({ anime_id: 1 });

    unmount(component);
  });

  it('adds from the add button without also dispatching select', async () => {
    const onSelect = vi.fn();
    const target = document.createElement('div');
    document.body.appendChild(target);
    const component = mount(SeasonView, { target, events: { select: onSelect } });
    await settle();

    const addBtn = document.querySelector('.new-band .add-btn') as HTMLElement;
    addBtn.click();
    await settle();

    expect(vi.mocked(importAnilistAnime)).toHaveBeenCalledWith(7);
    expect(onSelect).not.toHaveBeenCalled();

    unmount(component);
  });

  it('diffs against the future sentinel key when mounted in future mode', async () => {
    localStorage.setItem('anivault-season-state', JSON.stringify({ future: true, season: 'FALL', year: 2026, genre: '' }));
    const target = document.createElement('div');
    document.body.appendChild(target);
    const component = mount(SeasonView, { target });
    await settle();

    expect(vi.mocked(diffSeason)).toHaveBeenCalledWith('__FUTURE__', 0, [futureEntry.id], true);

    unmount(component);
  });
});
