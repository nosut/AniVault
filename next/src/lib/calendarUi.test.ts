import { describe, expect, it } from 'vitest';
import { episodeMarker } from './calendarUi';

const NOW = 1_800_000_000;

describe('episodeMarker', () => {
  it('returns "have" when the episode file exists', () => {
    expect(episodeMarker({ has_file: true, airing_at: NOW - 3600 }, NOW)).toBe('have');
  });

  it('returns "have" even if airing time is in the future (early release)', () => {
    expect(episodeMarker({ has_file: true, airing_at: NOW + 3600 }, NOW)).toBe('have');
  });

  it('returns "missing" when the episode aired but there is no file', () => {
    expect(episodeMarker({ has_file: false, airing_at: NOW - 3600 }, NOW)).toBe('missing');
  });

  it('returns "missing" for an episode airing exactly now without a file', () => {
    expect(episodeMarker({ has_file: false, airing_at: NOW }, NOW)).toBe('missing');
  });

  it('returns "future" when the episode has not aired yet', () => {
    expect(episodeMarker({ has_file: false, airing_at: NOW + 3600 }, NOW)).toBe('future');
  });

  it('returns "future" when the airing time is unknown', () => {
    expect(episodeMarker({ has_file: false, airing_at: null }, NOW)).toBe('future');
  });
});
