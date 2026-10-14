// One app-wide notice for problems from quick actions that have no spot of
// their own on screen: playing an episode, opening a folder or a link. App
// shows `appProblem` at the top of the page with ErrorNotice.
import { writable } from 'svelte/store';
import { openEpisodeFile, openContainingFolder } from './api';
import { explainError, type ExplainedError } from './errors';

export type AppProblem = ExplainedError & { retry?: () => void };

export const appProblem = writable<AppProblem | null>(null);

export function reportProblem(summary: string, e: unknown, retry?: () => void) {
  appProblem.set({ ...explainError(summary, e), retry });
}

// Promise.resolve().then keeps a synchronous throw on the same path as a
// rejected promise.
const attempt = (run: () => Promise<unknown>, onError: (e: unknown) => void) =>
  void Promise.resolve().then(run).catch(onError);

/** Open an episode in the default player, reporting a failure. */
export function playFile(path: string) {
  appProblem.set(null);
  attempt(() => openEpisodeFile(path), (e) =>
    reportProblem("The episode couldn't be opened.", e, () => playFile(path)));
}

/** Show a file in Explorer, reporting a failure. */
export function showInFolder(path: string) {
  appProblem.set(null);
  attempt(() => openContainingFolder(path), (e) =>
    reportProblem("The folder couldn't be opened.", e, () => showInFolder(path)));
}

/** Open a web page in the default browser, reporting a failure. */
export function openLink(url: string) {
  appProblem.set(null);
  attempt(() => openEpisodeFile(url), (e) =>
    reportProblem("The page couldn't be opened in your browser.", e, () => openLink(url)));
}
