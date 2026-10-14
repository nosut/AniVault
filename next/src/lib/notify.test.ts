import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';

vi.mock('./api', () => ({
  openEpisodeFile: vi.fn(async () => {}),
  openContainingFolder: vi.fn(async () => {}),
}));

import { openEpisodeFile, openContainingFolder } from './api';
import { appProblem, playFile, showInFolder, openLink } from './notify';

const settle = () => new Promise((r) => setTimeout(r, 0));

describe('notify', () => {
  beforeEach(() => {
    appProblem.set(null);
    vi.mocked(openEpisodeFile).mockReset().mockResolvedValue(undefined);
    vi.mocked(openContainingFolder).mockReset().mockResolvedValue(undefined);
  });

  it('stays quiet when the episode opens', async () => {
    playFile('C:/a.mkv');
    await settle();
    expect(get(appProblem)).toBeNull();
  });

  it('reports a failed play in plain words, with a retry that tries the same file', async () => {
    vi.mocked(openEpisodeFile).mockRejectedValueOnce('The system cannot find the file specified. (os error 2)');
    playFile('C:/a.mkv');
    await settle();

    const problem = get(appProblem)!;
    expect(problem.message).toMatch(/^The episode couldn't be opened\. The file or folder couldn.t be found/);
    expect(problem.details).toContain('os error 2');

    problem.retry!();
    await settle();
    expect(openEpisodeFile).toHaveBeenLastCalledWith('C:/a.mkv');
    expect(get(appProblem)).toBeNull();
  });

  it('reports a failed folder or link open', async () => {
    vi.mocked(openContainingFolder).mockRejectedValueOnce('Access is denied. (os error 5)');
    showInFolder('C:/a.mkv');
    await settle();
    expect(get(appProblem)?.message).toMatch(/^The folder couldn't be opened\./);

    vi.mocked(openEpisodeFile).mockRejectedValueOnce('boom');
    openLink('https://anilist.co/anime/1');
    await settle();
    expect(get(appProblem)?.message).toMatch(/^The page couldn't be opened in your browser\./);
  });
});
