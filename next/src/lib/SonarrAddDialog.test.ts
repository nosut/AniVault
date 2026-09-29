// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { flushSync, tick } from 'svelte';

vi.mock('./api', () => ({
  lookupSonarrCandidates: vi.fn(async () => [
    { tvdb_id: 100, title: 'Overgeared', year: 2025, season_count: 1, poster_url: null, overview: null, in_sonarr: false, sonarr_id: null },
    { tvdb_id: 200, title: 'Overlord', year: 2015, season_count: 4, poster_url: null, overview: null, in_sonarr: true, sonarr_id: 12 },
  ]),
  addToSonarr: vi.fn(async () => 55),
  linkSonarrCoverage: vi.fn(async () => {}),
}));

import { addToSonarr, linkSonarrCoverage } from './api';
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

    buttons()[0].click();
    await settle();
    expect(addToSonarr).toHaveBeenCalledWith(7, 100);
    expect(done).toHaveBeenCalledTimes(1);

    buttons()[1].click();
    await settle();
    expect(linkSonarrCoverage).toHaveBeenCalledWith(7, 12);
    expect(done).toHaveBeenCalledTimes(2);
  });

  it('shows the error text when adding fails', async () => {
    vi.mocked(addToSonarr).mockRejectedValueOnce('No AniList import list in Sonarr to copy settings from');
    createClassComponent({
      component: SonarrAddDialog,
      target: document.getElementById('app')!,
      props: { animeId: 7, title: 'Overgeared' },
    });
    await settle();
    buttons()[0].click();
    await settle();
    expect(document.body.textContent).toContain('No AniList import list in Sonarr to copy settings from');
  });
});
