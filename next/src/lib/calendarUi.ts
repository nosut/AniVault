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
