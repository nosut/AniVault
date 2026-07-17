import { describe, expect, it } from 'vitest';
import { addSeasons, futureLabel, seasonOffset } from './seasonUi';

describe('seasonOffset', () => {
  it('is 0 for the same season', () => {
    expect(seasonOffset('SUMMER', 2026, 'SUMMER', 2026)).toBe(0);
  });

  it('counts forward within a year', () => {
    expect(seasonOffset('FALL', 2026, 'SUMMER', 2026)).toBe(1);
  });

  it('counts across year boundaries', () => {
    expect(seasonOffset('WINTER', 2027, 'SUMMER', 2026)).toBe(2);
    expect(seasonOffset('SUMMER', 2027, 'SUMMER', 2026)).toBe(4);
    expect(seasonOffset('FALL', 2027, 'SUMMER', 2026)).toBe(5);
  });

  it('is negative for past seasons', () => {
    expect(seasonOffset('SPRING', 2026, 'SUMMER', 2026)).toBe(-1);
  });
});

describe('addSeasons', () => {
  it('advances within a year', () => {
    expect(addSeasons('WINTER', 2026, 2)).toEqual({ season: 'SUMMER', year: 2026 });
  });

  it('wraps into later years', () => {
    expect(addSeasons('SUMMER', 2026, 4)).toEqual({ season: 'SUMMER', year: 2027 });
    expect(addSeasons('FALL', 2026, 2)).toEqual({ season: 'SPRING', year: 2027 });
  });
});

describe('futureLabel', () => {
  it('uses season and year when assigned', () => {
    expect(futureLabel({ season: 'WINTER', season_year: 2028, start_year: 2028 })).toBe('Winter 2028');
  });

  it('falls back to the start year when no season is assigned', () => {
    expect(futureLabel({ season: null, season_year: null, start_year: 2028 })).toBe('2028');
  });

  it('is TBA when nothing is known', () => {
    expect(futureLabel({ season: null, season_year: null, start_year: null })).toBe('TBA');
  });

  it('is TBA when season exists without a year', () => {
    expect(futureLabel({ season: 'WINTER', season_year: null, start_year: null })).toBe('TBA');
  });
});
