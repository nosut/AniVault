// Pure helpers for SeasonView, kept out of the component for testability.

const SEASON_ORDER = ['WINTER', 'SPRING', 'SUMMER', 'FALL'] as const;
const SEASON_LABELS: Record<string, string> = {
  WINTER: 'Winter', SPRING: 'Spring', SUMMER: 'Summer', FALL: 'Fall',
};

function absIndex(season: string, year: number): number {
  return year * 4 + SEASON_ORDER.indexOf(season as (typeof SEASON_ORDER)[number]);
}

/// How many seasons ahead (positive) or behind (negative) a season is from
/// another.
export function seasonOffset(
  season: string, year: number,
  fromSeason: string, fromYear: number,
): number {
  return absIndex(season, year) - absIndex(fromSeason, fromYear);
}

/// The season `n` steps after the given one.
export function addSeasons(season: string, year: number, n: number): { season: string; year: number } {
  const abs = absIndex(season, year) + n;
  const idx = ((abs % 4) + 4) % 4;
  return { season: SEASON_ORDER[idx] ?? 'WINTER', year: Math.floor(abs / 4) };
}

/// Release label for a far-future / unscheduled show: "Winter 2028" when a
/// season is assigned, the bare year when only a start year is known, "TBA"
/// otherwise.
export function futureLabel(entry: {
  season?: string | null;
  season_year?: number | null;
  start_year?: number | null;
}): string {
  if (entry.season && entry.season_year != null) {
    return `${SEASON_LABELS[entry.season] ?? entry.season} ${entry.season_year}`;
  }
  if (entry.start_year != null) return String(entry.start_year);
  return 'TBA';
}
