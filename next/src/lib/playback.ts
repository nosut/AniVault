// What is playing, and the show it belongs to, shared by the sidebar Now
// Playing card and the Now Playing page. The status poll runs only while
// something is subscribed through startPlaybackPolling.
import { derived, get, writable } from 'svelte/store';
import {
  fetchAnimeDetail,
  getEpisodeFiles,
  getTrackingStatus,
  type AnimeDetail,
  type EngineEvent,
  type FileIndexEntry,
  type TrackingStatus,
} from './api';
import { explainError, type ExplainedError } from './errors';
import { getPref, setPref } from './prefs';

export interface ShowContext {
  animeId: number;
  detail: AnimeDetail;
  files: FileIndexEntry[];
}

export const POLL_MS = 2000;
export const LAST_SHOW_KEY = 'anivault-now-playing-last-show';
/** Settings toggle: open the Now Playing page when an episode starts. Off unless 'true'. */
export const AUTO_OPEN_KEY = 'anivault-now-playing-auto-open';

export const trackingStatus = writable<TrackingStatus>({ active: false, watching: null });
export const trackingPollError = writable<ExplainedError | null>(null);
/** The show playing now, or the last one identified. */
export const currentShow = writable<ShowContext | null>(null);
export const showError = writable<ExplainedError | null>(null);

/** The episode on screen, when it belongs to the current show. */
export const playingEpisode = derived([trackingStatus, currentShow], ([$status, $show]) => {
  const w = $status.watching;
  if (!w || !$show || w.anime_id !== $show.animeId) return null;
  return w.episode_guess ?? null;
});

let subscribers = 0;
let intervalId: ReturnType<typeof setInterval> | null = null;
let polling = false;
let loadingId: number | null = null;
let restored = false;

function readLastShow(): number | null {
  const v = Number(getPref(LAST_SHOW_KEY));
  return Number.isInteger(v) && v > 0 ? v : null;
}

function writeLastShow(id: number) {
  setPref(LAST_SHOW_KEY, String(id));
}

/** The identity of what's playing: show and episode, or null when nothing identified plays. */
export function playbackKey(status: TrackingStatus): string | null {
  const w = status.watching;
  return w?.anime_id != null ? `${w.anime_id}:${w.episode_guess ?? '?'}` : null;
}

/**
 * Whether a status update should open the Now Playing page: only when the
 * setting is on and a different episode has started. `lastKey` is the last
 * identified episode; a gap with nothing identified does not reset it, so a
 * brief blip in the same episode never reopens the page.
 */
export function autoOpenStep(lastKey: string | null, status: TrackingStatus, enabled: boolean): { open: boolean; key: string | null } {
  const key = playbackKey(status);
  if (key == null) return { open: false, key: lastKey };
  return { open: enabled && key !== lastKey, key };
}

export async function pollTrackingStatus() {
  if (polling) return;
  polling = true;
  try {
    const status = await getTrackingStatus();
    trackingStatus.set(status);
    trackingPollError.set(null);
    const id = status.watching?.anime_id;
    if (id != null && id !== get(currentShow)?.animeId) void loadShow(id);
  } catch (e) {
    trackingPollError.set(explainError("AniVault couldn't check what's playing.", e));
  } finally {
    polling = false;
  }
}

/** Load a show's detail and files into currentShow. */
export async function loadShow(animeId: number) {
  if (loadingId === animeId) return;
  loadingId = animeId;
  try {
    const [detail, files] = await Promise.all([fetchAnimeDetail(animeId), getEpisodeFiles(animeId)]);
    // A newer request (another show started) wins.
    if (loadingId !== animeId) return;
    currentShow.set({ animeId, detail, files });
    showError.set(null);
    writeLastShow(animeId);
  } catch (e) {
    if (loadingId === animeId) showError.set(explainError("The show that's playing couldn't be loaded.", e));
  } finally {
    if (loadingId === animeId) loadingId = null;
  }
}

/** Reload the current show when progress moves on for it. */
export function noteEngineEvents(events: EngineEvent[]) {
  const show = get(currentShow);
  if (!show) return;
  for (const ev of events) {
    if ('ProgressAdvanced' in ev && ev.ProgressAdvanced.anime_id === show.animeId) {
      void loadShow(show.animeId);
      return;
    }
  }
}

/** Start polling; returns the function that stops this subscriber's share. */
export function startPlaybackPolling(): () => void {
  subscribers += 1;
  if (!restored) {
    restored = true;
    const last = readLastShow();
    if (last != null && !get(currentShow)) void loadShow(last);
  }
  if (subscribers === 1) {
    void pollTrackingStatus();
    intervalId = setInterval(pollTrackingStatus, POLL_MS);
  }
  let stopped = false;
  return () => {
    if (stopped) return;
    stopped = true;
    subscribers -= 1;
    if (subscribers === 0 && intervalId) {
      clearInterval(intervalId);
      intervalId = null;
    }
  };
}

/** Test hook: forget all module state. */
export function resetPlaybackForTests() {
  if (intervalId) clearInterval(intervalId);
  intervalId = null;
  subscribers = 0;
  polling = false;
  loadingId = null;
  restored = false;
  trackingStatus.set({ active: false, watching: null });
  trackingPollError.set(null);
  currentShow.set(null);
  showError.set(null);
}

/** The display title for a show, matching the series page's choice. */
export function showTitle(d: AnimeDetail): string {
  try {
    const t = JSON.parse(d.titles_json || '{}');
    return t.english || t.english_derived || t.romaji || t.japanese || t.native || `Anime #${d.anime_id}`;
  } catch {
    return `Anime #${d.anime_id}`;
  }
}

/** Flatten season groups into one run of tiles (for the sidebar strip). */
export function flatTiles<T extends { tiles: unknown[] }>(seasons: T[]): T['tiles'] {
  return seasons.flatMap((s) => s.tiles) as T['tiles'];
}
