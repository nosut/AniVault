// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { mount, unmount, flushSync, tick } from 'svelte';

const file = (episode: number) => ({
  file_path: `D:/Anime/Show/Show - ${String(episode).padStart(2, '0')}.mkv`,
  anime_id: 7, episode, confidence: 100, indexed_at: 0, ignored: false, mapping_source: 'automatic',
});
const detail = {
  anime_id: 7,
  titles_json: JSON.stringify({ english: 'Frieren' }),
  episode_count: 4, image_url: null, synopsis: null, anime_status: 'FINISHED', last_modified: 0,
  list_status: 'watching', watched_episodes: 2, score: null, notes: null, local_updated: null, remote_updated: null,
  tracker_id: null, recent_history: [{ id: 1, anime_id: 7, episode: 2, file_path: null, player: null, watched_at: 1 }],
};

vi.mock('./api', () => ({
  getTrackingStatus: vi.fn(async () => ({ active: true, watching: null })),
  fetchAnimeDetail: vi.fn(async () => detail),
  getEpisodeFiles: vi.fn(async () => [file(1), file(2), file(3)]),
  setTrackingEnabled: vi.fn(async () => {}),
  confirmIdentification: vi.fn(async () => {}),
  openEpisodeFile: vi.fn(async () => {}),
  openContainingFolder: vi.fn(async () => {}),
}));

import { getTrackingStatus, openEpisodeFile } from './api';
import { resetPlaybackForTests } from './playback';
import { getPref, resetPrefs, setPref } from './prefs';
import NowPlaying from './NowPlaying.svelte';

async function settle() {
  for (let i = 0; i < 4; i++) {
    await tick();
    await new Promise((r) => setTimeout(r, 0));
  }
  flushSync();
}

let app: ReturnType<typeof mount> | null = null;
function render(props: Record<string, unknown> = {}) {
  const target = document.createElement('div');
  document.body.appendChild(target);
  app = mount(NowPlaying, { target, props: { events: [], ...props } });
  return target;
}

beforeEach(() => {
  resetPlaybackForTests();
  resetPrefs();
  vi.mocked(getTrackingStatus).mockReset().mockResolvedValue({ active: true, watching: null });
  vi.mocked(openEpisodeFile).mockClear();
});
afterEach(() => {
  if (app) unmount(app);
  app = null;
  document.body.innerHTML = '';
  resetPlaybackForTests();
});

describe('NowPlaying card', () => {
  it('waits for playback when no show is known', async () => {
    const el = render();
    await settle();
    expect(el.textContent).toContain('Waiting for playback');
  });

  it('shows the last show with Next and Replay when idle', async () => {
    setPref('anivault-now-playing-last-show', '7');
    const el = render();
    await settle();
    expect(el.textContent).toContain('Frieren');
    expect(el.textContent).toContain('Last watched Ep 2');
    const next = [...el.querySelectorAll<HTMLButtonElement>('.np-ctl')].find((b) => b.textContent?.includes('Ep 3'));
    next!.click();
    await settle();
    expect(openEpisodeFile).toHaveBeenCalledWith('D:/Anime/Show/Show - 03.mkv');
  });

  it('shows the playing episode and its neighbours', async () => {
    vi.mocked(getTrackingStatus).mockResolvedValue({
      active: true,
      watching: { player_name: 'mpv', file_path: null, window_title: null, episode_guess: 2, anime_id: 7 },
    });
    const el = render();
    await settle();
    expect(el.textContent).toContain('Ep 2 · mpv');
    const labels = [...el.querySelectorAll('.np-ctl')].map((b) => b.textContent?.trim());
    expect(labels).toEqual(['◀ Ep 1', 'Ep 3 ▶', 'Replay']);
  });

  it('opens the Now Playing page', async () => {
    setPref('anivault-now-playing-last-show', '7');
    const onOpen = vi.fn();
    const target = document.createElement('div');
    document.body.appendChild(target);
    app = mount(NowPlaying, { target, props: { events: [] }, events: { open: onOpen } });
    await settle();
    target.querySelector<HTMLButtonElement>('.np-open')!.click();
    expect(onOpen).toHaveBeenCalled();
  });
});
