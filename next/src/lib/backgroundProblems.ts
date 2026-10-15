// Background problems reported by the engine (sync stalled, a library folder
// offline, ...). App polls them and shows each as an ErrorNotice. The engine
// keeps them as current state, so a problem stays until the work recovers.
import { writable } from 'svelte/store';
import { getBackgroundProblems, refreshCalendar, scanLibraryFolders, setTrackingEnabled, triggerSync, type BackgroundProblem } from './api';
import { causeAdvice, errorCause } from './errors';

export const backgroundProblems = writable<BackgroundProblem[]>([]);

/// Problems the user closed, by area, with the `since` they closed. A new
/// occurrence (a later `since`) shows again.
const dismissed = writable<Record<string, number>>({});

export function dismissProblem(p: BackgroundProblem) {
  dismissed.update((d) => ({ ...d, [p.area]: p.since }));
}

/// Areas whose last Try again failed, so the notice says so instead of
/// looking unchanged.
const retryFailed = writable<Record<string, boolean>>({});
export const failedRetries = { subscribe: retryFailed.subscribe };

/// The fix to offer for each area, where retrying makes sense. Not for
/// library_folders: a scan can't repair the list, setting the folders again does.
const RETRY: Record<string, () => Promise<unknown>> = {
  sync: () => triggerSync(),
  library_scan: () => scanLibraryFolders(),
  tracking: () => setTrackingEnabled(true),
  calendar: () => refreshCalendar(),
};

/// What to do, for areas where the fix is a step only the user can take.
/// Used instead of the cause's advice, which would compete with it.
const NEXT_STEP: Record<string, string> = {
  sync_queue: 'Make the change again to resend it.',
  library_folders: 'Add your library folders again in Settings to replace the saved list.',
  tracking_setting: "Turn tracking off in Settings if you don't want it.",
  startup_entry: 'Turn "Launch AniVault when Windows starts" off and on again in Settings to retry.',
};

export interface BackgroundNotice {
  message: string;
  details: string;
  problem: BackgroundProblem;
  retry?: () => void;
}

/** The problems to show: not dismissed, in plain words, with a retry. */
export function visibleNotices(
  problems: BackgroundProblem[],
  closed: Record<string, number>,
  failed: Record<string, boolean> = {},
  onRetried: () => void = () => {},
): BackgroundNotice[] {
  return problems
    .filter((p) => closed[p.area] !== p.since)
    .map((p) => {
      const run = RETRY[p.area];
      const cause = errorCause(p.detail);
      const advice = NEXT_STEP[p.area]
        ?? (cause !== 'unknown' ? causeAdvice(cause)
          : run ? causeAdvice('unknown')
          : 'Show details has the technical information to report.');
      const lead = failed[p.area] ? 'Still not working. ' : '';
      return {
        message: `${lead}${p.summary} ${advice}`,
        details: p.detail,
        problem: p,
        retry: run
          ? () => {
              void run()
                .then(() => retryFailed.update((f) => ({ ...f, [p.area]: false })))
                .catch(() => retryFailed.update((f) => ({ ...f, [p.area]: true })))
                .finally(onRetried);
            }
          : undefined,
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
