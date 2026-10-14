// View preferences (tabs, sorts, layout, start page) persisted in the app's
// SQLite settings table. They used to live in WebView2 localStorage, whose
// LevelDB can corrupt and then silently drop every later write on each launch.
// SQLite reports a failed write, so a failure here raises `prefsError` and the
// App shows it instead of losing the change unnoticed.
//
// Everything is loaded once before the App mounts, so reads stay synchronous
// like the localStorage calls they replace. Writes update memory at once and
// reach the database through an ordered queue that keeps only the latest
// value per key.
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';
import type { InvokeFn } from './api';

/// Prefix shared by every key the app ever wrote to localStorage.
const LEGACY_PREFIX = 'anivault-';

export const prefsError = writable<string | null>(null);

let values = new Map<string, string>();
let writer: InvokeFn | null = null;
const pending = new Map<string, string | null>();
let flushing: Promise<void> | null = null;

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

function readLegacy(): Record<string, string> {
  const out: Record<string, string> = {};
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (!key?.startsWith(LEGACY_PREFIX)) continue;
      const value = localStorage.getItem(key);
      if (value !== null) out[key] = value;
    }
  } catch { /* storage unavailable: nothing to import */ }
  return out;
}

export async function initPrefs(invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  writer = invokeFn;
  type UiPrefs = { prefs: Record<string, string>; imported: boolean };
  let loaded: UiPrefs;
  try {
    loaded = await invokeFn<UiPrefs>('get_ui_prefs');
  } catch (e) {
    prefsError.set(`Couldn't load your saved settings (${message(e)}). Using defaults for now.`);
    return;
  }
  if (!loaded.imported) {
    const legacy = readLegacy();
    try {
      await invokeFn('import_ui_prefs', { prefs: legacy });
      loaded = await invokeFn<UiPrefs>('get_ui_prefs');
    } catch (e) {
      prefsError.set(`Couldn't move your settings into the database (${message(e)}). They'll be retried next launch.`);
      loaded = { prefs: { ...legacy, ...loaded.prefs }, imported: false };
    }
  }
  values = new Map(Object.entries(loaded.prefs));
}

export function getPref(key: string): string | null {
  return values.get(key) ?? null;
}

export function setPref(key: string, value: string) {
  if (values.get(key) === value) return;
  values.set(key, value);
  queue(key, value);
}

export function removePref(key: string) {
  if (!values.has(key)) return;
  values.delete(key);
  queue(key, null);
}

function queue(key: string, value: string | null) {
  if (!writer) return;
  pending.set(key, value);
  if (!flushing) flushing = drain();
}

async function drain() {
  try {
    while (pending.size) {
      const [key, value] = pending.entries().next().value as [string, string | null];
      pending.delete(key);
      try {
        await writer!('set_ui_pref', { key, value });
      } catch (e) {
        prefsError.set(`Couldn't save a setting (${message(e)}). The change may not survive a restart.`);
      }
    }
  } finally {
    flushing = null;
  }
}

/** Resolves once every queued write has been attempted. */
export async function flushPrefs() {
  while (flushing) await flushing;
}

/** Test helper: start over with these values and no database behind them. */
export function resetPrefs(initial: Record<string, string> = {}) {
  values = new Map(Object.entries(initial));
  writer = null;
  pending.clear();
  flushing = null;
  prefsError.set(null);
}
