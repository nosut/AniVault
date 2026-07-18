// Update-notice logic: show a banner when a newer release exists, and
// remember per-version dismissal so the same release never nags twice.
import type { UpdateInfo } from './api';

const KEY = 'anivault-dismissed-update';

export function loadDismissedUpdate(): string | null {
  try { return localStorage.getItem(KEY); } catch { return null; }
}

export function dismissUpdate(tag: string) {
  try { localStorage.setItem(KEY, tag); } catch {}
}

export function shouldShowUpdate(
  info: Pick<UpdateInfo, 'update_available' | 'latest'>,
  dismissed: string | null,
): boolean {
  return info.update_available && info.latest !== dismissed;
}
