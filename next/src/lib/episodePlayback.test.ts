import { describe, it, expect } from 'vitest';
import type { FileIndexEntry, RecentHistoryEntry } from './api';
import { buildEpisodeTiles, continueTarget, fileName, neighbours, playbackControls, seasonOf } from './episodePlayback';

const file = (episode: number | null, path = `D:/Anime/Show/Show - ${String(episode).padStart(2, '0')}.mkv`): FileIndexEntry => ({
  file_path: path,
  anime_id: 1,
  episode,
  confidence: 100,
  indexed_at: 0,
  ignored: false,
  mapping_source: 'automatic',
});
const hist = (episode: number, id = episode): RecentHistoryEntry => ({
  id, anime_id: 1, episode, file_path: null, player: null, watched_at: id,
});

describe('seasonOf and fileName', () => {
  it('reads SxxEyy and Season folders, defaulting to 1', () => {
    expect(seasonOf('D:/A/Show S02E03.mkv')).toBe(2);
    expect(seasonOf('D:/A/Season 3/Show - 01.mkv')).toBe(3);
    expect(seasonOf('D:/A/Show - 01.mkv')).toBe(1);
  });
  it('strips folders from both slash styles', () => {
    expect(fileName('D:\\Anime\\Show\\Ep 01.mkv')).toBe('Ep 01.mkv');
    expect(fileName('D:/Anime/Show/Ep 01.mkv')).toBe('Ep 01.mkv');
  });
});

describe('continueTarget', () => {
  const files = [file(1), file(2), file(3), file(4)];

  it('continues after recorded progress', () => {
    expect(continueTarget(files, 2)).toMatchObject({ episode: 3, kind: 'continue' });
  });

  it('starts again from the first file once the show is finished', () => {
    expect(continueTarget(files, 4, [], 4)).toMatchObject({ episode: 1, kind: 'restart' });
  });

  it('waits for the next episode of an airing show instead of restarting', () => {
    // Caught up on episode 4 of 12; episode 5 has not been released yet.
    expect(continueTarget(files, 4, [hist(4)], 12)).toBeNull();
    // Episode count unknown (still airing): never assume it is over.
    expect(continueTarget(files, 4, [hist(4)], null)).toBeNull();
    expect(continueTarget(files, 4, [hist(4)])).toBeNull();
  });

  it('carries on a rewatch from the latest watch below progress', () => {
    expect(continueTarget(files, 4, [hist(2, 9), hist(4, 1)])).toMatchObject({ episode: 3, kind: 'rewatch' });
  });

  it('ignores a latest watch at progress (a normal watch)', () => {
    expect(continueTarget(files, 2, [hist(2)])).toMatchObject({ episode: 3, kind: 'continue' });
  });

  it('falls through when the rewatch episode has no file', () => {
    expect(continueTarget([file(1), file(2), file(4)], 4, [hist(2)], 4)).toMatchObject({ episode: 1, kind: 'restart' });
  });

  it('returns null with no files and skips files with no episode number', () => {
    expect(continueTarget([], 0)).toBeNull();
    expect(continueTarget([file(null)], 0)).toBeNull();
  });
});

describe('buildEpisodeTiles', () => {
  it('fills missing episodes up to the episode count for one season', () => {
    const [s] = buildEpisodeTiles([file(1), file(3)], { watched: 1, episodeCount: 4, nextEpisode: 3 });
    expect(s!.tiles.map((t) => [t.episode, t.state])).toEqual([
      [1, 'watched'], [2, 'missing'], [3, 'next'], [4, 'missing'],
    ]);
  });

  it('marks the playing episode even over watched', () => {
    const [s] = buildEpisodeTiles([file(1), file(2)], { watched: 2, episodeCount: 2, playingEpisode: 1, nextEpisode: 2 });
    expect(s!.tiles.map((t) => t.state)).toEqual(['playing', 'next']);
  });

  it('extends past the episode count when files go further', () => {
    const [s] = buildEpisodeTiles([file(1), file(3)], { watched: 0, episodeCount: null });
    expect(s!.tiles.map((t) => t.state)).toEqual(['unwatched', 'missing', 'unwatched']);
  });

  it('groups multi-season files without filling gaps', () => {
    const seasons = buildEpisodeTiles(
      [file(1, 'D:/A/Show S01E01.mkv'), file(1, 'D:/A/Show S02E01.mkv'), file(3, 'D:/A/Show S02E03.mkv')],
      { watched: 0, episodeCount: 24 },
    );
    expect(seasons.map((s) => [s.season, s.tiles.map((t) => t.episode)])).toEqual([[1, [1]], [2, [1, 3]]]);
  });

  it('returns nothing for a show with no files and no count', () => {
    expect(buildEpisodeTiles([], { watched: 0, episodeCount: null })).toEqual([]);
  });
});

describe('neighbours', () => {
  it('skips gaps in both directions', () => {
    const n = neighbours([file(1), file(3), file(6)], 4);
    expect(n.prev?.episode).toBe(3);
    expect(n.next?.episode).toBe(6);
  });
  it('has no neighbour past either end', () => {
    const n = neighbours([file(1), file(2)], 1);
    expect(n.prev).toBeNull();
    expect(n.next?.episode).toBe(2);
  });
});

describe('playbackControls', () => {
  const files = [file(1), file(2), file(3), file(4)];

  it('uses the playing episode neighbours', () => {
    const c = playbackControls(files, 4, [], 2);
    expect([c.current, c.currentFile?.episode, c.prev?.episode, c.next?.episode]).toEqual([2, 2, 1, 3]);
  });

  it('offers the last watched as replay and the continue target as next when idle', () => {
    const c = playbackControls(files, 4, [hist(2)], null);
    expect([c.current, c.prev?.episode, c.next?.episode]).toEqual([2, 1, 3]);
  });

  it('offers no next when caught up on an airing show, keeping the replay', () => {
    const c = playbackControls(files, 4, [hist(4)], null, 12);
    expect([c.current, c.currentFile?.episode, c.prev?.episode, c.next]).toEqual([4, 4, 3, null]);
  });

  it('has nothing to replay with no history', () => {
    const c = playbackControls(files, 0, [], null);
    expect([c.current, c.currentFile, c.next?.episode]).toEqual([null, null, 1]);
  });
});
