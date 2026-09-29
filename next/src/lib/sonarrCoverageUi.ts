import type { CoverageRow, SonarrCandidate } from './api';

/** Shows no Sonarr series covers, sorted by title (case-insensitive). */
export function missingRows(rows: CoverageRow[]): CoverageRow[] {
  return rows
    .filter((r) => r.state === 'missing')
    .sort((a, b) => a.title.toLowerCase().localeCompare(b.title.toLowerCase()));
}

/** A series Sonarr already has is linked, never re-added (Sonarr would refuse). */
export function candidateAction(c: SonarrCandidate): 'link' | 'add' {
  return c.in_sonarr && c.sonarr_id != null ? 'link' : 'add';
}

export function candidateLabel(c: SonarrCandidate): string {
  const year = c.year ? ` (${c.year})` : '';
  const seasons = `${c.season_count} season${c.season_count === 1 ? '' : 's'}`;
  return `${c.title}${year} · ${seasons} · TVDB ${c.tvdb_id}`;
}

/** Shows the user chose to leave out of the check, sorted by title. */
export function ignoredRows(rows: CoverageRow[]): CoverageRow[] {
  return rows
    .filter((r) => r.state === 'ignored')
    .sort((a, b) => a.title.toLowerCase().localeCompare(b.title.toLowerCase()));
}
