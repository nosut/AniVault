// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { mount, unmount, flushSync, tick } from 'svelte';

// Fake only Date so the "today" grouping is deterministic; timers stay real.
vi.useFakeTimers({ toFake: ['Date'] });
vi.setSystemTime(new Date(2026, 6, 17, 12, 0, 0));
const NOW = Math.floor(Date.now() / 1000);
const H = 3600, D = 86400;

function cal(animeId: number, title: string, ep: number, airingAt: number, hasFile: boolean) {
  return {
    anime_id: animeId, title, image_url: null, episode_count: null, progress: null,
    next_episode: ep, airing_at: airingAt, time_until_airing: Math.max(0, airingAt - NOW), has_file: hasFile,
    watched: false,
  };
}

vi.mock('./api', () => ({
  getLibraryStats: vi.fn(async () => ({ total: 327, watching: 12, completed: 300, on_hold: 5, dropped: 4, plan_to_watch: 6 })),
  getContinueWatching: vi.fn(async () => [
    { anime_id: 1, anime_title: 'Frieren S2', image_url: null, watched_episodes: 2, episode_count: 12, last_watched_at: 100 },
    { anime_id: 3, anime_title: 'Sakamoto Days', image_url: null, watched_episodes: 28, episode_count: 33, last_watched_at: 90 },
  ]),
  getReadyToWatch: vi.fn(async () => [
    { anime_id: 1, title: 'Frieren S2', image_url: null, next_episode: 3, ready_count: 2, watched_episodes: 2, episode_count: 12 },
  ]),
  getCalendar: vi.fn(async () => [
    cal(1, 'Frieren S2', 3, NOW - 9 * H, true),        // aired today, downloaded
    cal(2, 'One Piece', 1142, NOW - 11 * H, false),    // aired today, missing
    cal(4, 'Kaiju No. 8', 4, NOW + 2 * H, false),      // airs later today
    cal(3, 'Sakamoto Days', 29, NOW - 4 * D, false),   // aired days ago, missing
    cal(5, 'Other Show', 7, NOW + 3 * D, false),       // future, different day
  ]),
  getAniListConnectionStatus: vi.fn(async () => true),
  getSyncStatus: vi.fn(async () => ({ pending: 0, failed: 0, blocked: 0, last_sync_at: 1000 })),
  getSonarrStatus: vi.fn(async () => ({ connected: true, series_count: 2, mapped_count: 2, last_sync_at: null })),
  searchSonarrEpisode: vi.fn(async () => 'Search started for episode 29'),
  confirmIdentification: vi.fn(async () => {}),
}));

import { searchSonarrEpisode } from './api';
import DashboardView from './DashboardView.svelte';

async function settle() {
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
  flushSync();
}

function sectionText(testId: string): string {
  return document.querySelector(`[data-testid="${testId}"]`)?.textContent ?? '';
}

describe('DashboardView home layout', () => {
  beforeEach(() => {
    document.body.innerHTML = '<div id="app"></div>';
  });
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('shows airing today, ready to watch, and missing downloads', async () => {
    const app = mount(DashboardView, { target: document.getElementById('app')!, props: { events: [] } });
    await settle();

    // Header pills.
    expect(document.body.textContent).toContain('synced');
    expect(document.body.textContent).toContain('327 in library');

    // Airing today: the three entries airing this local day, not the +3d one.
    const today = sectionText('airing-today');
    expect(today).toContain('Frieren S2');
    expect(today).toContain('Aired · 11h ago');
    expect(today).toContain('in 2h 0m');
    expect(today).not.toContain('Other Show');

    // Ready to watch.
    const ready = sectionText('ready-to-watch');
    expect(ready).toContain('Frieren S2');
    expect(ready).toContain('Ep 3');
    expect(ready).toContain('2 episodes ready');

    // Missing downloads, oldest first.
    const missing = sectionText('missing-downloads');
    expect(missing.indexOf('Sakamoto Days')).toBeLessThan(missing.indexOf('One Piece'));
    expect(missing).toContain('Ep 29');
    expect(missing).toContain('4d ago');
    expect(missing).toContain('today');

    // Jump back in: ready label from the ready list, missing label from the calendar.
    const jump = sectionText('jump-back-in');
    expect(jump).toContain('Ep 3 ready');
    expect(jump).toContain('Ep 29 not downloaded');

    // The old stat tiles are gone.
    expect(document.body.textContent).not.toContain('Plan to Watch');

    await unmount(app);
  });

  it('sends a Sonarr search from a missing-download row', async () => {
    const app = mount(DashboardView, { target: document.getElementById('app')!, props: { events: [] } });
    await settle();

    // Oldest missing entry first: Sakamoto Days Ep 29.
    const get = document.querySelector<HTMLButtonElement>('[data-testid="missing-downloads"] .get-btn')!;
    expect(get).not.toBeNull();
    get.click();
    flushSync();
    await settle();

    expect(searchSonarrEpisode).toHaveBeenCalledWith(3, 29);
    expect(sectionText('missing-downloads')).toContain('Sent');

    await unmount(app);
  });
});
