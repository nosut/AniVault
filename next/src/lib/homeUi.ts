// Pure helpers for the Home (dashboard) view.
import type { AniListSyncStatus } from './api';

export function isSameLocalDay(aSec: number, bSec: number): boolean {
  const a = new Date(aSec * 1000);
  const b = new Date(bSec * 1000);
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

/// Countdown / aired label for a row in the "Airing today" list.
export function todayRowLabel(airingAt: number, nowSec: number): string {
  const diff = airingAt - nowSec;
  if (diff > 0) {
    const h = Math.floor(diff / 3600);
    const m = Math.floor((diff % 3600) / 60);
    return h > 0 ? `in ${h}h ${m}m` : `in ${m}m`;
  }
  const ago = -diff;
  const h = Math.floor(ago / 3600);
  if (h > 0) return `Aired · ${h}h ago`;
  return `Aired · ${Math.floor(ago / 60)}m ago`;
}

/// Compact age label for the missing-downloads list.
export function airedAgoShort(airingAt: number, nowSec: number): string {
  if (isSameLocalDay(airingAt, nowSec)) return 'today';
  const days = Math.max(1, Math.floor((nowSec - airingAt) / 86400));
  return `${days}d ago`;
}

/// One-line AniList status for the header pill.
export function syncPill(
  connected: boolean,
  status: AniListSyncStatus | null,
): { text: string; ok: boolean } {
  if (!connected) return { text: 'not connected', ok: false };
  if (status && status.failed > 0) return { text: `${status.failed} failed`, ok: false };
  if (status && status.pending > 0) return { text: `${status.pending} pending`, ok: true };
  return { text: 'synced', ok: true };
}
