import { describe, it, expect } from 'vitest';
import { latestProgressAdvance, samePrompt } from './upNext';
import type { EngineEvent } from './api';

const pa = (anime_id: number, new_episode: number): EngineEvent =>
  ({ ProgressAdvanced: { anime_id, old_episode: new_episode - 1, new_episode, source: 'auto' } } as EngineEvent);

describe('latestProgressAdvance', () => {
  it('returns null when there is no ProgressAdvanced event', () => {
    expect(latestProgressAdvance([])).toBeNull();
    expect(latestProgressAdvance([{ LibraryUpdated: { indexed: 1, removed: 0 } } as EngineEvent])).toBeNull();
  });
  it('returns the last ProgressAdvanced in the batch', () => {
    expect(latestProgressAdvance([pa(1, 3), pa(2, 5)])).toEqual({ anime_id: 2, new_episode: 5 });
  });
  it('returns the last ProgressAdvanced even when a non-ProgressAdvanced event trails it', () => {
    expect(
      latestProgressAdvance([pa(1, 3), { LibraryUpdated: { indexed: 1, removed: 0 } } as EngineEvent]),
    ).toEqual({ anime_id: 1, new_episode: 3 });
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
