// Background problems reported by the engine (sync stalled, a library folder
// offline, ...). App polls them and shows each as an ErrorNotice. The engine
// keeps them as current state, so a problem stays until the work recovers.
import { writable } from 'svelte/store';
import { getBackgroundProblems, scanLibraryFolders, triggerSync, type BackgroundProblem } from './api';
import { explainError, type ExplainedError } from './errors';

export const backgroundProblems = writable<BackgroundProblem[]>([]);

/// Problems the user closed, by area, with the `since` they closed. A new
/// occurrence (a later `since`) shows again.
const dismissed = writable<Record<string, number>>({});

export function dismissProblem(p: BackgroundProblem) {
  dismissed.update((d) => ({ ...d, [p.area]: p.since }));
}

/// The fix to offer for each area, where retrying makes sense.
const RETRY: Record<string, () => Promise<unknown>> = {
  sync: () => triggerSync(),
  library_scan: () => scanLibraryFolders(),
  library_folders: () => scanLibraryFolders(),
};

export type BackgroundNotice = ExplainedError & { problem: BackgroundProblem; retry?: () => void };

/** The problems to show: not dismissed, in plain words, with a retry. */
export function visibleNotices(
  problems: BackgroundProblem[],
  closed: Record<string, number>,
  onRetried: () => void = () => {},
): BackgroundNotice[] {
  return problems
    .filter((p) => closed[p.area] !== p.since)
    .map((p) => {
      const run = RETRY[p.area];
      return {
        ...explainError(p.summary, p.detail),
        problem: p,
        retry: run ? () => { void run().catch(() => {}).finally(onRetried); } : undefined,
      };
    });
}

export const dismissedProblems = { subscribe: dismissed.subscribe };

/** Read the board; a failed read keeps the last known list. */
export async function refreshBackgroundProblems() {
  try {
    backgroundProblems.set(await getBackgroundProblems());
  } catch {
    // The read itself failing is covered by every other view's own errors.
  }
}
