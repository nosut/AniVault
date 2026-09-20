// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { flushSync, tick } from 'svelte';

// Fake only Date so the countdowns are deterministic; timers stay real.
vi.useFakeTimers({ toFake: ['Date'] });
vi.setSystemTime(new Date(2026, 8, 19, 12, 0, 0));
const NOW = Math.floor(Date.now() / 1000);
const H = 3600, D = 86400;

function entry(anime_id: number, title: string, airing_status: string | null) {
  return {
    anime_id, title, status: 'plan_to_watch', watched_episodes: 0, episode_count: 12,
    score: null, image_url: null, season: 'FALL', season_year: 2026, airing_status,
  };
}

function cal(anime_id: number, next_episode: number, airing_at: number) {
  return {
    anime_id, title: '', image_url: null, episode_count: null, progress: null,
    next_episode, airing_at, time_until_airing: airing_at - NOW, has_file: false, watched: false,
  };
}

vi.mock('./api', () => ({
  searchLibrary: vi.fn(async () => [
    entry(1, 'Premieres This Week', 'NOT_YET_RELEASED'),
    entry(2, 'Premieres Next Month', 'NOT_YET_RELEASED'),
    entry(3, 'Already Airing', 'RELEASING'),
    entry(4, 'Finished Long Ago', 'FINISHED'),
  ]),
  getCalendar: vi.fn(async () => [
    cal(1, 1, NOW + 3 * D + 4 * H),
    cal(2, 1, NOW + 30 * D),
    cal(3, 5, NOW + 5 * H),
  ]),
  getLibraryStats: vi.fn(async () => ({ total: 4, watching: 0, completed: 0, on_hold: 0, dropped: 0, plan_to_watch: 4 })),
  getEpisodeFilesBulk: vi.fn(async () => new Map()),
  getEpisodeFiles: vi.fn(async () => []),
  updateListEntry: vi.fn(async () => {}),
  deleteAnime: vi.fn(async () => {}),
  openEpisodeFile: vi.fn(async () => {}),
  openContainingFolder: vi.fn(async () => {}),
  scanLibraryFolders: vi.fn(async () => {}),
}));

import { createClassComponent } from 'svelte/legacy';
import LibraryView from './LibraryView.svelte';
import { searchLibrary } from './api';

async function settle() {
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
  flushSync();
}

function chipsByTitle(rowSelector: string, titleSelector: string): Record<string, string | null> {
  const out: Record<string, string | null> = {};
  for (const row of document.querySelectorAll(rowSelector)) {
    const title = row.querySelector(titleSelector)?.textContent?.trim() ?? '';
    out[title] = row.querySelector('.airing-chip')?.textContent?.trim() ?? null;
  }
  return out;
}

describe('LibraryView plan-to-watch airing marker', () => {
  let component: { $destroy: () => void } | undefined;

  beforeEach(() => {
    document.body.innerHTML = '<div id="app"></div>';
    localStorage.clear();
    localStorage.setItem('anivault-library-filter', 'plan_to_watch');
  });

  afterEach(() => {
    component?.$destroy();
    component = undefined;
  });

  it('chips premieres inside a week and already-airing shows in the table', async () => {
    component = createClassComponent({ component: LibraryView, target: document.getElementById('app')! });
    await settle();

    expect(chipsByTitle('tr.data-row', '.title-text')).toEqual({
      'Premieres This Week': 'Premieres in 3d 4h',
      'Premieres Next Month': null,
      'Already Airing': 'Airing · Ep 5 in 5h 0m',
      'Finished Long Ago': null,
    });
  });

  it('accents a chip under a day away', async () => {
    component = createClassComponent({ component: LibraryView, target: document.getElementById('app')! });
    await settle();

    const soon = [...document.querySelectorAll('.airing-chip.soon')].map((n) => n.textContent?.trim());
    expect(soon).toEqual(['Airing · Ep 5 in 5h 0m']);
  });

  it('shows the chip on grid poster cards too', async () => {
    localStorage.setItem('anivault-library-viewmode', 'grid');
    component = createClassComponent({ component: LibraryView, target: document.getElementById('app')! });
    await settle();

    const chips = [...document.querySelectorAll('.poster-info .airing-chip')].map((n) => n.textContent?.trim());
    expect(chips.sort()).toEqual(['Airing · Ep 5 in 5h 0m', 'Premieres in 3d 4h']);
  });
});

describe('LibraryView selection', () => {
  let component: { $destroy: () => void } | undefined;

  const fall = (id: number, title: string) => entry(id, title, 'FINISHED');
  const winter = (id: number, title: string) => ({ ...entry(id, title, 'FINISHED'), season: 'WINTER', season_year: 2027 });

  function mount() {
    component = createClassComponent({ component: LibraryView, target: document.getElementById('app')! });
  }

  const groupCheck = (label: string) =>
    document.querySelector<HTMLInputElement>(`input[aria-label="Select all of ${label}"]`)!;
  const rowCheck = (title: string) =>
    document.querySelector<HTMLInputElement>(`input[aria-label="Select ${title}"]`)!;
  const checkedTitles = () =>
    [...document.querySelectorAll<HTMLInputElement>('.data-row .col-check input, .poster-check input')]
      .filter((i) => i.checked)
      .map((i) => i.getAttribute('aria-label')!.replace('Select ', ''));
  const batchCount = () => document.querySelector('.batch-count')?.textContent?.trim() ?? null;

  async function click(el: HTMLElement) {
    el.click();
    await settle();
  }

  beforeEach(() => {
    document.body.innerHTML = '<div id="app"></div>';
    localStorage.clear();
    localStorage.setItem('anivault-library-filter', 'plan_to_watch');
    vi.mocked(searchLibrary).mockResolvedValue([
      fall(1, 'Fall One'), fall(2, 'Fall Two'), winter(3, 'Winter One'),
    ] as never);
  });

  afterEach(async () => {
    // The selection deliberately outlives the component, so empty it here or
    // it leaks into the next test.
    for (const box of document.querySelectorAll<HTMLInputElement>('.data-row .col-check input, .poster-check input')) {
      if (box.checked) await click(box);
    }
    component?.$destroy();
    component = undefined;
  });

  it('checks one season from its header without touching the others', async () => {
    mount();
    await settle();

    await click(groupCheck('Fall 2026'));

    expect(checkedTitles()).toEqual(['Fall One', 'Fall Two']);
    expect(batchCount()).toBe('2 selected');
    expect(groupCheck('Fall 2026').checked).toBe(true);
    expect(groupCheck('Winter 2027').checked).toBe(false);
  });

  it('shows a partly checked season as indeterminate, then fills and clears it', async () => {
    mount();
    await settle();

    await click(rowCheck('Fall Two'));
    expect(groupCheck('Fall 2026').indeterminate).toBe(true);
    expect(groupCheck('Fall 2026').checked).toBe(false);

    await click(groupCheck('Fall 2026'));
    expect(checkedTitles()).toEqual(['Fall One', 'Fall Two']);
    expect(groupCheck('Fall 2026').indeterminate).toBe(false);

    await click(groupCheck('Fall 2026'));
    expect(checkedTitles()).toEqual([]);
  });

  it('selects a collapsed season without expanding it', async () => {
    localStorage.setItem('anivault-library-season-collapsed', JSON.stringify({ fall2026: true }));
    mount();
    await settle();

    await click(groupCheck('Fall 2026'));

    expect(batchCount()).toBe('2 selected');
    expect(rowCheck('Fall One')).toBeNull();

    await click(groupCheck('Fall 2026'));
    expect(batchCount()).toBeNull();
  });

  it('offers the season checkbox on the poster grid too', async () => {
    localStorage.setItem('anivault-library-viewmode', 'grid');
    mount();
    await settle();

    await click(groupCheck('Winter 2027'));

    expect(checkedTitles()).toEqual(['Winter One']);
  });

  it('keeps checks across a detail round trip', async () => {
    mount();
    await settle();
    await click(groupCheck('Fall 2026'));

    component!.$destroy();
    mount();
    await settle();

    expect(checkedTitles()).toEqual(['Fall One', 'Fall Two']);
    expect(batchCount()).toBe('2 selected');
    expect(groupCheck('Fall 2026').checked).toBe(true);
  });

  it('drops checks for shows that left the list while the detail view was open', async () => {
    mount();
    await settle();
    await click(groupCheck('Fall 2026'));

    component!.$destroy();
    vi.mocked(searchLibrary).mockResolvedValue([fall(1, 'Fall One'), winter(3, 'Winter One')] as never);
    mount();
    await settle();

    expect(checkedTitles()).toEqual(['Fall One']);
    expect(batchCount()).toBe('1 selected');
  });
});
