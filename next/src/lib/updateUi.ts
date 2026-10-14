// Update-notice logic: show a banner when a newer release exists, and
// remember per-version dismissal so the same release never nags twice.
import { getPref, setPref } from './prefs';
import type { UpdateInfo } from './api';

const KEY = 'anivault-dismissed-update';

export function loadDismissedUpdate(): string | null {
  try { return getPref(KEY); } catch { return null; }
}

export function dismissUpdate(tag: string) {
  try { setPref(KEY, tag); } catch {}
}

export function shouldShowUpdate(
  info: Pick<UpdateInfo, 'update_available' | 'latest'>,
  dismissed: string | null,
): boolean {
  return info.update_available && info.latest !== dismissed;
}
