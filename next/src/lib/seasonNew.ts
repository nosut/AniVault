// Pure helpers for the Seasons page's "new since your last visit" group, kept
// out of the component for testability — same split as seasonUi.ts.

/**
 * Split a season listing into the entries flagged as new and everything else.
 *
 * Flagged entries are held out of `rest` so the page can render them in the
 * band without also repeating them in the main grid. Source order — AniList's
 * popularity sort — is preserved within each partition.
 */
export function partitionNew<T extends { id: number }>(
  entries: T[],
  newIds: Set<number>,
): { fresh: T[]; rest: T[] } {
  const fresh: T[] = [];
  const rest: T[] = [];
  for (const entry of entries) {
    if (newIds.has(entry.id)) fresh.push(entry);
    else rest.push(entry);
  }
  return { fresh, rest };
}
