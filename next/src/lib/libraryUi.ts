// Pure helpers for LibraryView, kept out of the component for testability.

import { getCurrentSeason } from './seasonUi';

export { getCurrentSeason };

/// A persisted status filter is only trusted if it still corresponds to a tab.
/// Anything else -- an empty string, or a category that has since been removed
/// -- falls back to All, so a stale localStorage value cannot leave the view
/// with no tab selected and an empty-looking list.
export function normalizeStatusFilter(
  value: string | null,
  known: (string | null)[],
): string | null {
  if (!value) return null;
  return known.includes(value) ? value : null;
}

/// The season fields every grouped row carries. Structural, so both
/// LibraryEntry and test fixtures satisfy it.
export interface SeasonGrouped {
  season: string | null;
  season_year: number | null;
}

export const SEASON_ORDER: Record<string, number> = {
  WINTER: 0, SPRING: 1, SUMMER: 2, FALL: 3,
};

const SEASON_LABELS: Record<string, string> = {
  WINTER: 'Winter', SPRING: 'Spring', SUMMER: 'Summer', FALL: 'Fall',
};

/// Sortable position of a season. Undated shows sort last.
export function seasonSortVal(e: SeasonGrouped): number {
  if (!e.season_year) return Number.POSITIVE_INFINITY;
  return e.season_year * 10 + (SEASON_ORDER[e.season ?? ''] ?? 0);
}

/// Stable identity for a season group, used as the localStorage collapse key.
export function seasonGroupKey(e: SeasonGrouped): string {
  if (!e.season || e.season_year == null) return 'tba';
  return `${e.season.toLowerCase()}${e.season_year}`;
}

/// Display name for a season group.
export function seasonGroupLabel(e: SeasonGrouped): string {
  if (!e.season || e.season_year == null) return 'TBA';
  return `${SEASON_LABELS[e.season] ?? e.season} ${e.season_year}`;
}

export interface SeasonGroup<T> {
  key: string;
  label: string;
  /// 'This season' / 'Next season' on the soonest group that has not already
  /// passed; null on every other group.
  chip: string | null;
  entries: T[];
}

/// Absolute season index, so seasons compare across year boundaries.
function absIndex(season: string, year: number): number {
  return year * 4 + (SEASON_ORDER[season] ?? 0);
}

/// Group rows into seasons, nearest first, TBA last.
///
/// The marker is computed against `current` (today's season) rather than
/// against list position, so it stays correct even when the list happens to
/// contain a season that has already started.
export function groupBySeason<T extends SeasonGrouped>(
  entries: T[],
  current: { season: string; year: number },
): SeasonGroup<T>[] {
  const byKey = new Map<string, SeasonGroup<T>>();
  for (const e of entries) {
    const key = seasonGroupKey(e);
    let g = byKey.get(key);
    if (!g) {
      g = { key, label: seasonGroupLabel(e), chip: null, entries: [] };
      byKey.set(key, g);
    }
    g.entries.push(e);
  }

  const groups = [...byKey.values()];
  // TBA has no position on the calendar, so it is pinned last rather than
  // sorted; every dated group orders ascending.
  groups.sort((a, b) => {
    if (a.key === 'tba') return b.key === 'tba' ? 0 : 1;
    if (b.key === 'tba') return -1;
    return seasonSortVal(a.entries[0]!) - seasonSortVal(b.entries[0]!);
  });

  const currentAbs = absIndex(current.season, current.year);
  const soonest = groups.find((g) => {
    if (g.key === 'tba') return false;
    const e = g.entries[0]!;
    return absIndex(e.season as string, e.season_year as number) >= currentAbs;
  });
  if (soonest) {
    const e = soonest.entries[0]!;
    const isCurrent = absIndex(e.season as string, e.season_year as number) === currentAbs;
    soonest.chip = isCurrent ? 'This season' : 'Next season';
  }

  return groups;
}

/// One rendered row: either a season header or an anime.
///
/// Grouped and ungrouped modes both reduce to a list of these, so the template
/// needs a single `{#each}` and the row markup exists in exactly one place.
export type DisplayRow<T> =
  | { kind: 'group'; group: SeasonGroup<T> }
  | { kind: 'entry'; entry: T };

/// Interleave group headers with their entries, skipping collapsed bodies.
/// A season absent from `collapsed` is open.
export function flattenGroups<T>(
  groups: SeasonGroup<T>[],
  collapsed: Record<string, boolean>,
): DisplayRow<T>[] {
  const out: DisplayRow<T>[] = [];
  for (const group of groups) {
    out.push({ kind: 'group', group });
    if (!collapsed[group.key]) {
      for (const entry of group.entries) out.push({ kind: 'entry', entry });
    }
  }
  return out;
}

/// The ungrouped equivalent: every row is an anime.
export function asDisplayRows<T>(entries: T[]): DisplayRow<T>[] {
  return entries.map((entry) => ({ kind: 'entry', entry }));
}

/// The airing fields the Watching column needs. Structural, so CalendarEntry
/// and test fixtures both satisfy it.
export interface AiringLike {
  anime_id: number;
  next_episode: number | null;
  airing_at: number | null;
}

/// Earliest still-future airing per anime.
///
/// `get_calendar` returns one entry per airing episode across a window that
/// reaches ~a month into the past, so past entries are filtered out rather
/// than assumed absent. A show with nothing upcoming is simply missing from
/// the map, which the column renders as a dash.
export function nextAiringByAnime<T extends AiringLike>(
  entries: T[],
  nowSec: number,
): Map<number, T> {
  const out = new Map<number, T>();
  for (const e of entries) {
    if (e.airing_at == null || e.airing_at <= nowSec) continue;
    const existing = out.get(e.anime_id);
    if (!existing || (existing.airing_at as number) > e.airing_at) {
      out.set(e.anime_id, e);
    }
  }
  return out;
}

/// Countdown at day/hour granularity: "6d 14h" / "14h 3m" / "3m".
///
/// Deliberately separate from DetailView's formatCountdown and CalendarView's
/// countdown: those two already disagree (seconds tier, "Aired" vs "airing
/// now"), so unifying them would change what those views display.
export function formatAiringCountdown(secs: number): string {
  if (secs <= 0) return 'airing now';
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

/// Sort position by next airing. Shows with nothing upcoming sort last.
export function nextAiringSortVal(
  animeId: number,
  map: Map<number, AiringLike>,
): number {
  const hit = map.get(animeId);
  return hit?.airing_at ?? Number.POSITIVE_INFINITY;
}
