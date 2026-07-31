// Pure helpers for CalendarView, kept out of the component for testability.

export type EpisodeMarker = 'have' | 'missing' | 'future';

/// Download-status marker for a calendar episode: 'have' when the file is in
/// the library, 'missing' when the episode already aired without a file, and
/// 'future' when nothing is expected yet (not aired, or air time unknown).
export function episodeMarker(
  entry: { has_file: boolean; airing_at: number | null },
  nowSec: number,
): EpisodeMarker {
  if (entry.has_file) return 'have';
  if (entry.airing_at != null && entry.airing_at <= nowSec) return 'missing';
  return 'future';
}

/// Human-readable name for each download-status marker, used by both the dot's
/// tooltip and the entry's accessible label.
export const markerLabels = {
  have: 'Downloaded',
  missing: 'Not downloaded',
  future: 'Upcoming',
} as const;

/// Accessible label for one calendar entry: title, episode number, download
/// state, and whether it has already been watched. Download and watched are
/// independent facts, so watched is appended rather than replacing the marker.
export function entryLabel(
  entry: { title: string; next_episode: number | null; watched: boolean },
  marker: EpisodeMarker,
): string {
  const ep = entry.next_episode ?? '?';
  const state = entry.watched ? `${markerLabels[marker]}, watched` : markerLabels[marker];
  return `${entry.title} Ep ${ep} (${state})`;
}
