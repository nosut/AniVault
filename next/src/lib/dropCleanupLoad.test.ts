import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';

vi.mock('./api', () => ({
  getSetting: vi.fn(async () => null),
  openEpisodeFile: vi.fn(async () => {}),
  openContainingFolder: vi.fn(async () => {}),
}));

import { getSetting } from './api';
import { cleanupQueue, loadCleanupSettings, noteStatusChanges, offerRemovalCleanup } from './dropCleanup';
import { appProblem } from './notify';

describe('loading clean-up settings', () => {
  beforeEach(() => {
    vi.useRealTimers();
    appProblem.set(null);
    cleanupQueue.set([]);
    vi.mocked(getSetting).mockReset().mockResolvedValue(null);
  });

  it('fills unset values with the defaults', async () => {
    expect(await loadCleanupSettings()).toEqual({ offerSonarr: true, offerFiles: true, statuses: ['dropped'], offerResume: true });
  });

  it('throws on a failed read instead of passing the defaults off as saved choices', async () => {
    vi.mocked(getSetting).mockRejectedValue('database is locked');
    await expect(loadCleanupSettings()).rejects.toBe('database is locked');
  });

  it('skips the prompt and says why when the settings cannot be read', async () => {
    vi.useFakeTimers();
    vi.mocked(getSetting).mockRejectedValue('database is locked');
    noteStatusChanges([{ animeId: 1, title: 'Show', from: 'watching', to: 'dropped' }]);
    await vi.advanceTimersByTimeAsync(1000);

    expect(get(cleanupQueue)).toEqual([]);
    expect(get(appProblem)?.message).toMatch(/^Your clean-up settings couldn't be read, so no clean-up prompt was shown/);
  });
});

describe('offering clean-up before a removal', () => {
  beforeEach(() => cleanupQueue.set([]));

  it('queues a removal prompt and resolves with the choice it settles on', async () => {
    const answer = offerRemovalCleanup([{ animeId: 7, title: 'Show' }]);
    const [req] = get(cleanupQueue);
    expect(req).toMatchObject({ kind: 'remove', shows: [{ animeId: 7, title: 'Show' }] });
    req!.settle!(false);
    // A later settle (the App's close handler) cannot change the answer.
    req!.settle!(true);
    expect(await answer).toBe(false);
  });

  it('goes ahead without a prompt when there is nothing to remove', async () => {
    expect(await offerRemovalCleanup([])).toBe(true);
    expect(get(cleanupQueue)).toEqual([]);
  });
});
