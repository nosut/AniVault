import { describe, it, expect } from 'vitest';
import { latestPlaybackEnded, samePrompt } from './upNext';
import type { EngineEvent } from './api';

const pe = (anime_id: number, episode: number): EngineEvent =>
  ({ PlaybackEnded: { anime_id, episode, file_key: `D:/a${anime_id}-e${episode}.mkv`, watched_secs: 1500 } } as EngineEvent);

describe('latestPlaybackEnded', () => {
  it('returns null when there is no PlaybackEnded event', () => {
    expect(latestPlaybackEnded([])).toBeNull();
    expect(latestPlaybackEnded([{ LibraryUpdated: { indexed: 1, removed: 0 } } as EngineEvent])).toBeNull();
  });
  it('ignores ProgressAdvanced, which no longer drives the prompt', () => {
    const pa = { ProgressAdvanced: { anime_id: 1, old_episode: 2, new_episode: 3, source: 'manual' } } as EngineEvent;
    expect(latestPlaybackEnded([pa])).toBeNull();
  });
  it('returns the last PlaybackEnded in the batch', () => {
    expect(latestPlaybackEnded([pe(1, 3), pe(2, 5)])).toEqual({ anime_id: 2, episode: 5 });
  });
  it('returns the last PlaybackEnded even when another event trails it', () => {
    expect(
      latestPlaybackEnded([pe(1, 3), { LibraryUpdated: { indexed: 1, removed: 0 } } as EngineEvent]),
    ).toEqual({ anime_id: 1, episode: 3 });
  });
});

describe('samePrompt', () => {
  it('treats identical anime+episode as the same prompt', () => {
    expect(samePrompt({ anime_id: 1, episode: 13 }, { anime_id: 1, episode: 13 })).toBe(true);
    expect(samePrompt({ anime_id: 1, episode: 13 }, { anime_id: 1, episode: 14 })).toBe(false);
    expect(samePrompt(null, { anime_id: 1, episode: 13 })).toBe(false);
    expect(samePrompt(null, null)).toBe(false);
  });
});
