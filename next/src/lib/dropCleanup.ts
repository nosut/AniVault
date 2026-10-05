// When a show moves to Dropped (or another chosen status), offer to stop or
// remove it in Sonarr and to delete its episode files. When it moves back to
// Watching or Planning, offer the reverse: monitor it again, or add it back.
// The settings decide whether each offer appears; the dialogs in App.svelte
// do the asking.

import { writable } from 'svelte/store';
import { getSetting, type CleanupSonarrSeries, type StatusChangePreview } from './api';

export const CLEANUP_SONARR_KEY = 'cleanup.offer_sonarr';
export const CLEANUP_FILES_KEY = 'cleanup.offer_files';
export const CLEANUP_STATUSES_KEY = 'cleanup.statuses';
export const CLEANUP_RESUME_KEY = 'cleanup.offer_resume';

export const CLEANUP_STATUS_OPTIONS: { value: string; label: string }[] = [
  { value: 'dropped', label: 'Dropped' },
  { value: 'completed', label: 'Completed' },
  { value: 'on_hold', label: 'On Hold' },
];

export const STATUS_LABELS: Record<string, string> = {
  watching: 'Watching',
  plan_to_watch: 'Planning',
  dropped: 'Dropped',
  completed: 'Completed',
  on_hold: 'On Hold',
};

/// The statuses Sonarr should be fetching for.
const ACTIVE_STATUSES = ['watching', 'plan_to_watch'];

export interface CleanupSettings {
  offerSonarr: boolean;
  offerFiles: boolean;
  /// Moving into one of these offers cleanup; moving out of one into an
  /// active status offers the reverse.
  statuses: string[];
  offerResume: boolean;
}

export const DEFAULT_CLEANUP_SETTINGS: CleanupSettings = {
  offerSonarr: true,
  offerFiles: true,
  statuses: ['dropped'],
  offerResume: true,
};

export async function loadCleanupSettings(): Promise<CleanupSettings> {
  try {
    const [offerSonarr, offerFiles, statuses, offerResume] = await Promise.all([
      getSetting<boolean>(CLEANUP_SONARR_KEY),
      getSetting<boolean>(CLEANUP_FILES_KEY),
      getSetting<string[]>(CLEANUP_STATUSES_KEY),
      getSetting<boolean>(CLEANUP_RESUME_KEY),
    ]);
    return {
      offerSonarr: offerSonarr ?? DEFAULT_CLEANUP_SETTINGS.offerSonarr,
      offerFiles: offerFiles ?? DEFAULT_CLEANUP_SETTINGS.offerFiles,
      statuses: Array.isArray(statuses) ? statuses : DEFAULT_CLEANUP_SETTINGS.statuses,
      offerResume: offerResume ?? DEFAULT_CLEANUP_SETTINGS.offerResume,
    };
  } catch {
    return DEFAULT_CLEANUP_SETTINGS;
  }
}

export type PromptKind = 'drop' | 'resume';

/// Which prompt, if any, moving a show from `from` to `to` should raise.
export function promptFor(settings: CleanupSettings, from: string | null | undefined, to: string | null | undefined): PromptKind | null {
  if (!to || to === from) return null;
  if (settings.statuses.includes(to)) {
    // Moving between two cleanup statuses (Dropped to On Hold) was already
    // offered on the way in.
    if (from && settings.statuses.includes(from)) return null;
    return settings.offerSonarr || settings.offerFiles ? 'drop' : null;
  }
  if (settings.offerResume && ACTIVE_STATUSES.includes(to) && from && settings.statuses.includes(from)) {
    return 'resume';
  }
  return null;
}

export type SonarrAction = 'keep' | 'unmonitor' | 'remove';

export interface CleanupShow {
  animeId: number;
  title: string;
}

/// One show in the drop prompt: only the parts the settings offer and the
/// show has.
export interface CleanupItem extends CleanupShow {
  sonarr: CleanupSonarrSeries | null;
  fileCount: number;
  totalBytes: number;
}

export function cleanupItems(shows: CleanupShow[], previews: StatusChangePreview[], settings: CleanupSettings): CleanupItem[] {
  const byId = new Map(previews.map((p) => [p.anime_id, p]));
  return shows
    .map((s) => {
      const p = byId.get(s.animeId);
      return {
        ...s,
        sonarr: settings.offerSonarr ? (p?.sonarr ?? null) : null,
        fileCount: settings.offerFiles ? (p?.file_count ?? 0) : 0,
        totalBytes: settings.offerFiles ? (p?.total_bytes ?? 0) : 0,
      };
    })
    .filter((i) => i.sonarr !== null || i.fileCount > 0);
}

/// One show in the resume prompt: either its series is unmonitored, or it is
/// not in Sonarr at all.
export type ResumeItem = CleanupShow & (
  | { kind: 'unmonitored'; sonarr: CleanupSonarrSeries }
  | { kind: 'missing' }
);

export function resumeItems(shows: CleanupShow[], previews: StatusChangePreview[]): ResumeItem[] {
  const byId = new Map(previews.map((p) => [p.anime_id, p]));
  const items: ResumeItem[] = [];
  for (const s of shows) {
    const p = byId.get(s.animeId);
    if (p?.sonarr && !p.sonarr.monitored) items.push({ ...s, kind: 'unmonitored', sonarr: p.sonarr });
    else if (p?.sonarr_missing) items.push({ ...s, kind: 'missing' });
  }
  // Two shows on one series need it monitored only once.
  const seen = new Set<number>();
  return items.filter((i) => {
    if (i.kind !== 'unmonitored') return true;
    if (seen.has(i.sonarr.sonarr_id)) return false;
    seen.add(i.sonarr.sonarr_id);
    return true;
  });
}

/// The other active shows a set of drop items would affect, by series.
export function sharedWarnings(items: CleanupItem[]): string[] {
  const titles = new Set<string>();
  const dropping = new Set(items.map((i) => i.title));
  for (const i of items) for (const t of i.sonarr?.shared_with ?? []) if (!dropping.has(t)) titles.add(t);
  return [...titles];
}

export interface CleanupRequest {
  kind: PromptKind;
  status: string;
  shows: CleanupShow[];
}

/// One status transition, as the engine's `StatusChanged` event reports it.
export interface StatusChange {
  animeId: number;
  title: string;
  from: string | null;
  to: string;
}

/// The prompts a batch of transitions raises: one per kind and target status,
/// each show once (its latest transition wins).
export function requestsFor(settings: CleanupSettings, changes: StatusChange[]): CleanupRequest[] {
  const latest = new Map<number, StatusChange>();
  for (const c of changes) {
    const prev = latest.get(c.animeId);
    // Keep the earliest `from` so Watching → Dropped → On Hold reads as one move.
    latest.set(c.animeId, prev ? { ...c, from: prev.from } : c);
  }
  const requests = new Map<string, CleanupRequest>();
  for (const c of latest.values()) {
    const kind = promptFor(settings, c.from, c.to);
    if (!kind) continue;
    const key = `${kind}:${c.to}`;
    const req = requests.get(key) ?? { kind, status: c.to, shows: [] };
    req.shows.push({ animeId: c.animeId, title: c.title });
    requests.set(key, req);
  }
  return [...requests.values()];
}

/// Prompts waiting to be shown; App.svelte renders the first and shifts it
/// off when it closes.
export const cleanupQueue = writable<CleanupRequest[]>([]);

/// Bumped with the affected show ids after a dialog applies, so open views
/// can reload Sonarr and file state.
export const cleanupApplied = writable<{ animeIds: number[] } | null>(null);

// Transitions arrive one event per show, a batch change over several polls.
// Collect them briefly so a batch raises one prompt rather than one per show.
const SETTLE_MS = 600;
let pending: StatusChange[] = [];
let settleTimer: ReturnType<typeof setTimeout> | null = null;

export function noteStatusChanges(changes: StatusChange[]): void {
  if (changes.length === 0) return;
  pending.push(...changes);
  if (settleTimer) clearTimeout(settleTimer);
  settleTimer = setTimeout(() => void flushStatusChanges(), SETTLE_MS);
}

async function flushStatusChanges(): Promise<void> {
  settleTimer = null;
  const changes = pending;
  pending = [];
  const requests = requestsFor(await loadCleanupSettings(), changes);
  if (requests.length > 0) cleanupQueue.update((q) => [...q, ...requests]);
}
