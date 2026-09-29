import { describe, expect, it } from 'vitest';
import { candidateAction, candidateLabel, missingRows } from './sonarrCoverageUi';
import type { CoverageRow, SonarrCandidate } from './api';

const row = (anime_id: number, title: string, state: CoverageRow['state']): CoverageRow =>
  ({ anime_id, title, image_url: null, list_status: 'watching', state } as CoverageRow);

const cand = (over: Partial<SonarrCandidate>): SonarrCandidate => ({
  tvdb_id: 1, title: 'Overgeared', year: 2025, season_count: 1, poster_url: null,
  overview: null, in_sonarr: false, sonarr_id: null, ...over,
});

describe('Sonarr coverage helpers', () => {
  it('keeps only missing rows, sorted by title ignoring case', () => {
    const rows = [
      row(1, 'zeta', 'missing'),
      { ...row(2, 'Covered', 'covered'), sonarr_id: 3, via: 'title' } as CoverageRow,
      row(3, 'Alpha', 'missing'),
      row(4, 'Ignored', 'ignored'),
    ];
    expect(missingRows(rows).map((r) => r.anime_id)).toEqual([3, 1]);
  });

  it('offers Link for a series Sonarr already has, Add otherwise', () => {
    expect(candidateAction(cand({ in_sonarr: true, sonarr_id: 9 }))).toBe('link');
    expect(candidateAction(cand({}))).toBe('add');
  });

  it('labels candidates with year, seasons and TVDB id', () => {
    expect(candidateLabel(cand({ season_count: 2 }))).toBe('Overgeared (2025) · 2 seasons · TVDB 1');
    expect(candidateLabel(cand({ year: null, season_count: 1 }))).toBe('Overgeared · 1 season · TVDB 1');
  });
});
