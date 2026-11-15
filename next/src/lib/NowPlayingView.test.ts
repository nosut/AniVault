// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { mount, unmount, flushSync, tick } from 'svelte';
import { get } from 'svelte/store';

const file = (episode: number) => ({
  file_path: `D:/Anime/Show/Show - ${String(episode).padStart(2, '0')}.mkv`,
  anime_id: 7, episode, confidence: 100, indexed_at: 0, ignored: false, mapping_source: 'automatic',
});
const detail = {
  anime_id: 7,
  titles_json: JSON.stringify({ romaji: 'Sousou no Frieren', english: 'Frieren' }),
  episode_count: 4, image_url: null, synopsis: 'An elf mage.', anime_status: 'FINISHED', last_modified: 0,
  list_status: 'watching', watched_episodes: 2, score: 9, notes: null, local_updated: null, remote_updated: null,
  tracker_id: null, recent_history: [],
};

vi.mock('./api', () => ({
  getTrackingStatus: vi.fn(async () => ({ active: true, watching: null })),
  fetchAnimeDetail: vi.fn(async () => detail),
  getEpisodeFiles: vi.fn(async () => [file(1), file(2), file(3)]),
  getSeriesDiskSize: vi.fn(async () => 0),
  openEpisodeFile: vi.fn(async () => {}),
  openContainingFolder: vi.fn(async () => {}),
}));

import { getTrackingStatus, openEpisodeFile, fetchAnimeDetail } from './api';
import { currentShow, playingEpisode, resetPlaybackForTests, startPlaybackPolling, noteEngineEvents } from './playback';
import NowPlayingView from './NowPlayingView.svelte';

async function settle() {
  for (let i = 0; i < 4; i++) {
    await tick();
    await new Promise((r) => setTimeout(r, 0));
  }
  flushSync();
}

let app: ReturnType<typeof mount> | null = null;
function render() {
  const target = document.createElement('div');
  document.body.appendChild(target);
  app = mount(NowPlayingView, { target });
  return target;
}

beforeEach(() => {
  resetPlaybackForTests();
  localStorage.clear();
  vi.mocked(getTrackingStatus).mockReset().mockResolvedValue({ active: true, watching: null });
  vi.mocked(openEpisodeFile).mockClear();
  vi.mocked(fetchAnimeDetail).mockClear();
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  resetPlaybackForTests();
});

describe('playback store', () => {
  it('loads the identified show and reports the playing episode', async () => {
    vi.mocked(getTrackingStatus).mockResolvedValue({
      active: true,
      watching: { player_name: 'mpv', file_path: null, window_title: null, episode_guess: 3, anime_id: 7 },
    });
    const stop = startPlaybackPolling();
    await settle();
    expect(get(currentShow)?.animeId).toBe(7);
    expect(get(playingEpisode)).toBe(3);
    expect(localStorage.getItem('anivault.nowPlaying.lastAnimeId')).toBe('7');
    stop();
  });

  it('restores the last show when nothing is playing', async () => {
    localStorage.setItem('anivault.nowPlaying.lastAnimeId', '7');
    const stop = startPlaybackPolling();
    await settle();
    expect(get(currentShow)?.animeId).toBe(7);
    expect(get(playingEpisode)).toBeNull();
    stop();
  });

  it('reloads the show when its progress advances', async () => {
    localStorage.setItem('anivault.nowPlaying.lastAnimeId', '7');
    const stop = startPlaybackPolling();
    await settle();
    vi.mocked(fetchAnimeDetail).mockClear();
    noteEngineEvents([{ ProgressAdvanced: { anime_id: 7, new_episode: 3 } } as never]);
    await settle();
    expect(fetchAnimeDetail).toHaveBeenCalledWith(7);
    stop();
  });
});

describe('NowPlayingView', () => {
  it('shows an empty state before anything has played', async () => {
    const el = render();
    await settle();
    expect(el.textContent).toContain('Nothing playing yet');
  });

  it('shows the playing show and plays the next episode in one click', async () => {
    vi.mocked(getTrackingStatus).mockResolvedValue({
      active: true,
      watching: { player_name: 'mpv', file_path: null, window_title: null, episode_guess: 2, anime_id: 7 },
    });
    const el = render();
    await settle();
    expect(el.querySelector('h2')?.textContent).toBe('Frieren');
    expect(el.textContent).toContain('Playing in mpv');
    expect(el.querySelector('[aria-current="true"]')?.textContent?.trim()).toContain('2');
    // Episode 4 has no file on disk.
    expect(el.querySelector<HTMLButtonElement>('button[aria-label^="Play episode 4"]')?.disabled).toBe(true);

    const next = [...el.querySelectorAll<HTMLButtonElement>('button')].find((b) => b.textContent?.includes('Play Ep 3'));
    next!.click();
    await settle();
    expect(openEpisodeFile).toHaveBeenCalledWith('D:/Anime/Show/Show - 03.mkv');
  });

  it('plays any episode from its tile', async () => {
    localStorage.setItem('anivault.nowPlaying.lastAnimeId', '7');
    const el = render();
    await settle();
    el.querySelector<HTMLButtonElement>('button[aria-label^="Play episode 1"]')!.click();
    await settle();
    expect(openEpisodeFile).toHaveBeenCalledWith('D:/Anime/Show/Show - 01.mkv');
  });
});
