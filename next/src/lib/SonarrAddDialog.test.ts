// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { flushSync, tick } from 'svelte';

vi.mock('./api', () => ({
  lookupSonarrCandidates: vi.fn(async () => [
    { tvdb_id: 100, title: 'Overgeared', year: 2025, season_count: 1, poster_url: null, overview: null, in_sonarr: false, sonarr_id: null },
    { tvdb_id: 200, title: 'Overlord', year: 2015, season_count: 4, poster_url: null, overview: null, in_sonarr: true, sonarr_id: 12 },
  ]),
  lookupSonarrByTvdb: vi.fn(async () => [
    { tvdb_id: 480598, title: 'Because I, the True Saint, was Banished', year: 2026, season_count: 1, poster_url: null, overview: null, in_sonarr: false, sonarr_id: null },
  ]),
  addToSonarr: vi.fn(async () => 55),
  linkSonarrCoverage: vi.fn(async () => {}),
}));

import { addToSonarr, linkSonarrCoverage, lookupSonarrByTvdb, lookupSonarrCandidates } from './api';
import { createClassComponent } from 'svelte/legacy';
import SonarrAddDialog from './SonarrAddDialog.svelte';

async function settle() {
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
  flushSync();
}

function buttons(): HTMLButtonElement[] {
  return [...document.querySelectorAll<HTMLButtonElement>('[data-testid="candidate-action"]')];
}

describe('SonarrAddDialog', () => {
  beforeEach(() => {
    document.body.innerHTML = '<div id="app"></div>';
    vi.clearAllMocks();
  });

  it('adds a new series and links an existing one', async () => {
    const done = vi.fn();
    const c = createClassComponent({
      component: SonarrAddDialog,
      target: document.getElementById('app')!,
      props: { animeId: 7, title: 'Overgeared' },
    });
    c.$on('done', done);
    await settle();

    expect(buttons().map((b) => b.textContent?.trim())).toEqual(['Add', 'Link']);
    expect(document.body.textContent).toContain('In Sonarr');

    buttons()[0]!.click();
    await settle();
    expect(addToSonarr).toHaveBeenCalledWith(7, 100);
    expect(done).toHaveBeenCalledTimes(1);

    buttons()[1]!.click();
    await settle();
    expect(linkSonarrCoverage).toHaveBeenCalledWith(7, 12);
    expect(done).toHaveBeenCalledTimes(2);
  });

  it('explains a failed add in plain words, with the raw error behind Show details', async () => {
    vi.mocked(addToSonarr).mockRejectedValueOnce('No AniList import list in Sonarr to copy settings from');
    createClassComponent({
      component: SonarrAddDialog,
      target: document.getElementById('app')!,
      props: { animeId: 7, title: 'Overgeared' },
    });
    await settle();
    buttons()[0]!.click();
    await settle();
    expect(document.body.textContent).toContain(`"Overgeared" couldn't be added to Sonarr. This needs an AniList import list in Sonarr`);
    expect(document.body.textContent).not.toContain('copy settings from');

    [...document.querySelectorAll('button')].find((b) => b.textContent?.trim() === 'Show details')!.click();
    await settle();
    expect(document.body.textContent).toContain('No AniList import list in Sonarr to copy settings from');
  });

  it('retries the same choice from the error', async () => {
    vi.mocked(addToSonarr).mockRejectedValueOnce('Sonarr add series HTTP 503: Service Unavailable');
    createClassComponent({
      component: SonarrAddDialog,
      target: document.getElementById('app')!,
      props: { animeId: 7, title: 'Overgeared' },
    });
    await settle();
    buttons()[0]!.click();
    await settle();
    [...document.querySelectorAll('button')].find((b) => b.textContent?.trim() === 'Try again')!.click();
    await settle();
    expect(addToSonarr).toHaveBeenLastCalledWith(7, 100);
    expect(addToSonarr).toHaveBeenCalledTimes(2);
  });

  it('re-searches for the new show when animeId changes, so Add never uses the old show results', async () => {
    vi.mocked(lookupSonarrCandidates)
      .mockResolvedValueOnce([
        { tvdb_id: 100, title: 'Show A', year: 2025, season_count: 1, poster_url: null, overview: null, in_sonarr: false, sonarr_id: null },
      ])
      .mockResolvedValueOnce([
        { tvdb_id: 300, title: 'Show B', year: 2024, season_count: 1, poster_url: null, overview: null, in_sonarr: false, sonarr_id: null },
      ]);
    const c = createClassComponent({
      component: SonarrAddDialog,
      target: document.getElementById('app')!,
      props: { animeId: 7, title: 'Show A' },
    });
    await settle();
    c.$set({ animeId: 8, title: 'Show B' });
    await settle();
    expect(lookupSonarrCandidates).toHaveBeenLastCalledWith(8);
    expect(document.body.textContent).not.toContain('Show A (2025)');
    buttons()[0]!.click();
    await settle();
    expect(addToSonarr).toHaveBeenCalledWith(8, 300);
  });

  it('finds a show the name search missed from a pasted TVDB link, and adds it', async () => {
    vi.mocked(lookupSonarrCandidates).mockResolvedValueOnce([]);
    createClassComponent({
      component: SonarrAddDialog,
      target: document.getElementById('app')!,
      props: { animeId: 7, title: 'True Saint' },
    });
    await settle();
    expect(document.body.textContent).toContain('Sonarr found no match by name.');

    const input = document.querySelector<HTMLInputElement>('input[aria-label="TVDB ID or link"]')!;
    input.value = ' https://thetvdb.com/series/shinnoseijo-anime ';
    input.dispatchEvent(new Event('input'));
    await settle();
    document.querySelector('form')!.dispatchEvent(new Event('submit', { cancelable: true }));
    await settle();

    expect(lookupSonarrByTvdb).toHaveBeenCalledWith('https://thetvdb.com/series/shinnoseijo-anime');
    expect(input.value).toBe('');
    buttons()[0]!.click();
    await settle();
    expect(addToSonarr).toHaveBeenCalledWith(7, 480598);
  });

  it('puts the pasted series first without listing it twice', async () => {
    vi.mocked(lookupSonarrByTvdb).mockResolvedValueOnce([
      { tvdb_id: 200, title: 'Overlord', year: 2015, season_count: 4, poster_url: null, overview: null, in_sonarr: true, sonarr_id: 12 },
    ]);
    createClassComponent({
      component: SonarrAddDialog,
      target: document.getElementById('app')!,
      props: { animeId: 7, title: 'Overgeared' },
    });
    await settle();
    const input = document.querySelector<HTMLInputElement>('input[aria-label="TVDB ID or link"]')!;
    input.value = '200';
    input.dispatchEvent(new Event('input'));
    await settle();
    document.querySelector('form')!.dispatchEvent(new Event('submit', { cancelable: true }));
    await settle();
    expect(buttons().map((b) => b.textContent?.trim())).toEqual(['Link', 'Add']);
  });

  it('explains input that is not a TVDB ID or link', async () => {
    vi.mocked(lookupSonarrByTvdb).mockRejectedValueOnce(`"Frieren" isn't a TVDB ID or thetvdb.com link`);
    createClassComponent({
      component: SonarrAddDialog,
      target: document.getElementById('app')!,
      props: { animeId: 7, title: 'Overgeared' },
    });
    await settle();
    const input = document.querySelector<HTMLInputElement>('input[aria-label="TVDB ID or link"]')!;
    input.value = 'Frieren';
    input.dispatchEvent(new Event('input'));
    await settle();
    document.querySelector('form')!.dispatchEvent(new Event('submit', { cancelable: true }));
    await settle();
    expect(document.body.textContent).toContain(`AniVault couldn't find "Frieren" in Sonarr. Check the TVDB ID or link.`);
  });
});
