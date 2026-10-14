// View preferences (tabs, sorts, layout, start page) persisted in the app's
// SQLite settings table. They used to live in WebView2 localStorage, whose
// LevelDB can corrupt and then silently drop every later write on each launch.
// SQLite reports a failed write, so a failure here raises `prefsError` and the
// App shows it instead of losing the change unnoticed: a plain message from
// describePrefsProblem, with the technical detail behind "Show details".
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

/// What went wrong, and the raw error text for the details panel.
export type PrefsProblem = { kind: 'load' | 'import' | 'save'; detail: string };

export const prefsError = writable<PrefsProblem | null>(null);

/// The words a user sees for each problem: what happened and what it means
/// for them, never the error itself. Every kind is fixed by trying again.
export function describePrefsProblem(problem: PrefsProblem): { message: string; action: string } {
  switch (problem.kind) {
    case 'load':
      return {
        message: "Your saved view settings couldn't be loaded, so AniVault is showing its default views for now. Nothing has been lost.",
        action: 'Try again',
      };
    case 'import':
      return {
        message: "AniVault couldn't move your view settings over from the previous version. They still apply for now, and AniVault will try again the next time it starts.",
        action: 'Try again',
      };
    case 'save':
      return {
        message: "A change to your view settings couldn't be saved, so it may be undone the next time AniVault starts.",
        action: 'Try again',
      };
  }
}

let values = new Map<string, string>();
let writer: InvokeFn | null = null;
const pending = new Map<string, string | null>();
/// Keys whose last save failed, retried by retryFailedSaves.
const failed = new Set<string>();
let flushing: Promise<void> | null = null;

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));
const report = (kind: PrefsProblem['kind'], step: string, e: unknown) =>
  prefsError.set({ kind, detail: `${step} failed at ${new Date().toISOString()}: ${message(e)}` });

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
    report('load', 'get_ui_prefs', e);
    return;
  }
  if (!loaded.imported) {
    const legacy = readLegacy();
    try {
      await invokeFn('import_ui_prefs', { prefs: legacy });
      loaded = await invokeFn<UiPrefs>('get_ui_prefs');
    } catch (e) {
      report('import', 'import_ui_prefs', e);
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
        failed.delete(key);
        if (failed.size === 0) prefsError.update((p) => (p?.kind === 'save' ? null : p));
      } catch (e) {
        failed.add(key);
        report('save', `set_ui_pref "${key}"`, e);
      }
    }
  } finally {
    flushing = null;
  }
}

/** Queue every setting whose last save failed, with its current value. */
export function retryFailedSaves() {
  for (const key of failed) queue(key, values.get(key) ?? null);
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
  failed.clear();
  flushing = null;
  prefsError.set(null);
}
