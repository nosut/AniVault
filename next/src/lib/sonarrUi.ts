import type { SonarrTagOption } from './api';

/** The tag labels the Sonarr import uses when `sonarr.wanted_tags` is unset (mirrors `WANTED_TAGS` in `sonarr/import.rs`). */
export const DEFAULT_SONARR_WANTED_TAGS = ['1 - nosut', 'mine'];

export interface WantedTagRow {
  label: string;
  /** Series carrying the tag; `null` for a saved tag Sonarr no longer has. */
  seriesCount: number | null;
  checked: boolean;
  /** Saved, but not defined in Sonarr any more: it filters nothing. */
  missing: boolean;
}

const norm = (label: string) => label.trim().toLowerCase();

/**
 * One row per Sonarr tag, checked when it is in the saved `sonarr.wanted_tags`
 * (matched like the import does: trimmed, case-insensitive). `saved === null`
 * means the setting is unset, so the default tags are checked where they exist.
 * Saved tags Sonarr no longer has are appended, checked and flagged `missing`.
 */
export function wantedTagRows(tags: SonarrTagOption[], saved: string[] | null): WantedTagRow[] {
  const wanted = new Set((saved ?? DEFAULT_SONARR_WANTED_TAGS).map(norm));
  const rows: WantedTagRow[] = tags.map((t) => ({
    label: t.label,
    seriesCount: t.series_count,
    checked: wanted.has(norm(t.label)),
    missing: false,
  }));
  if (saved) {
    const known = new Set(tags.map((t) => norm(t.label)));
    for (const label of saved) {
      if (!known.has(norm(label)) && label.trim()) {
        rows.push({ label: label.trim(), seriesCount: null, checked: true, missing: true });
      }
    }
  }
  return rows;
}

/** Flip one row. Unchecking a missing tag removes it, since it can't be re-checked. */
export function toggleWantedTag(rows: WantedTagRow[], label: string): WantedTagRow[] {
  return rows
    .map((r) => (r.label === label ? { ...r, checked: !r.checked } : r))
    .filter((r) => !(r.missing && !r.checked));
}

/** The labels to store in `sonarr.wanted_tags`. An empty list imports every series. */
export function selectedWantedTags(rows: WantedTagRow[]): string[] {
  return rows.filter((r) => r.checked).map((r) => r.label);
}
