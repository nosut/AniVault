// Pure helpers for LibraryView, kept out of the component for testability.

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
