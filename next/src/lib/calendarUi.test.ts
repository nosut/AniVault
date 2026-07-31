import { describe, expect, it } from 'vitest';
import { entryLabel, episodeMarker, markerLabels } from './calendarUi';

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

describe('entryLabel', () => {
  it('names the download state for an unwatched entry', () => {
    expect(
      entryLabel({ title: 'Frieren', next_episode: 7, watched: false }, 'have'),
    ).toBe('Frieren Ep 7 (Downloaded)');
  });

  it('appends "watched" after the download state', () => {
    expect(
      entryLabel({ title: 'Frieren', next_episode: 6, watched: true }, 'have'),
    ).toBe('Frieren Ep 6 (Downloaded, watched)');
  });

  it('keeps watched independent of the download state', () => {
    expect(
      entryLabel({ title: 'Dandadan', next_episode: 9, watched: true }, 'missing'),
    ).toBe('Dandadan Ep 9 (Not downloaded, watched)');
  });

  it('falls back to "?" when the episode number is unknown', () => {
    expect(
      entryLabel({ title: 'Unknown Show', next_episode: null, watched: false }, 'future'),
    ).toBe('Unknown Show Ep ? (Upcoming)');
  });

  it('exposes the marker labels used in the dot tooltip', () => {
    expect(markerLabels.have).toBe('Downloaded');
    expect(markerLabels.missing).toBe('Not downloaded');
    expect(markerLabels.future).toBe('Upcoming');
  });
});
