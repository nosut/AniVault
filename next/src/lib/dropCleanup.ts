// When a show moves to Dropped (or another chosen status), offer to stop or
// remove it in Sonarr and to delete its episode files. The settings decide
// whether each offer appears; the dialog in App.svelte does the asking.

import { writable } from 'svelte/store';
import { getSetting, type DropCleanupPreview } from './api';

export const CLEANUP_SONARR_KEY = 'cleanup.offer_sonarr';
export const CLEANUP_FILES_KEY = 'cleanup.offer_files';
export const CLEANUP_STATUSES_KEY = 'cleanup.statuses';

export const CLEANUP_STATUS_OPTIONS: { value: string; label: string }[] = [
  { value: 'dropped', label: 'Dropped' },
  { value: 'completed', label: 'Completed' },
  { value: 'on_hold', label: 'On Hold' },
];

export interface CleanupSettings {
  offerSonarr: boolean;
  offerFiles: boolean;
  statuses: string[];
}

export const DEFAULT_CLEANUP_SETTINGS: CleanupSettings = {
  offerSonarr: true,
  offerFiles: true,
  statuses: ['dropped'],
};

export async function loadCleanupSettings(): Promise<CleanupSettings> {
  try {
    const [offerSonarr, offerFiles, statuses] = await Promise.all([
      getSetting<boolean>(CLEANUP_SONARR_KEY),
      getSetting<boolean>(CLEANUP_FILES_KEY),
      getSetting<string[]>(CLEANUP_STATUSES_KEY),
    ]);
    return {
      offerSonarr: offerSonarr ?? DEFAULT_CLEANUP_SETTINGS.offerSonarr,
      offerFiles: offerFiles ?? DEFAULT_CLEANUP_SETTINGS.offerFiles,
      statuses: Array.isArray(statuses) ? statuses : DEFAULT_CLEANUP_SETTINGS.statuses,
    };
  } catch {
    return DEFAULT_CLEANUP_SETTINGS;
  }
}

/// Whether moving a show from `from` to `to` should raise the cleanup prompt.
export function triggersCleanup(settings: CleanupSettings, from: string | null | undefined, to: string | null | undefined): boolean {
  if (!to || to === from) return false;
  if (!settings.offerSonarr && !settings.offerFiles) return false;
  return settings.statuses.includes(to);
}

export type SonarrAction = 'keep' | 'unmonitor' | 'remove';

export interface CleanupShow {
  animeId: number;
  title: string;
}

/// One show in the prompt: only the parts the settings offer and the show has.
export interface CleanupItem extends CleanupShow {
  sonarr: DropCleanupPreview['sonarr'];
  fileCount: number;
  totalBytes: number;
}

export function cleanupItems(shows: CleanupShow[], previews: DropCleanupPreview[], settings: CleanupSettings): CleanupItem[] {
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

export interface CleanupRequest {
  status: string;
  shows: CleanupShow[];
}

/// Pending prompt; App.svelte renders the dialog while this is set.
export const cleanupRequest = writable<CleanupRequest | null>(null);

/// Call after a status change succeeds. `shows` carry each show's previous
/// status so a no-op move does not prompt.
export async function offerDropCleanup(status: string, shows: (CleanupShow & { from?: string | null })[]): Promise<void> {
  const settings = await loadCleanupSettings();
  const moved = shows.filter((s) => triggersCleanup(settings, s.from, status));
  if (moved.length === 0) return;
  cleanupRequest.set({ status, shows: moved.map(({ animeId, title }) => ({ animeId, title })) });
}

/// Bumped with the cleaned-up show ids after the dialog applies, so open
/// views can reload Sonarr and file state.
export const cleanupApplied = writable<{ animeIds: number[] } | null>(null);
