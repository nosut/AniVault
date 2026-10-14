// @vitest-environment jsdom
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import {
  initPrefs, getPref, setPref, removePref, resetPrefs, flushPrefs, prefsError,
  retryFailedSaves, describePrefsProblem,
} from './prefs';

type Call = { command: string; args?: Record<string, unknown> };

function fakeBackend(stored: Record<string, string>, imported: boolean) {
  const calls: Call[] = [];
  const invoke = vi.fn(async (command: string, args?: Record<string, unknown>) => {
    calls.push({ command, args });
    switch (command) {
      case 'get_ui_prefs': return { prefs: { ...stored }, imported } as never;
      case 'import_ui_prefs': {
        const incoming = args!.prefs as Record<string, string>;
        for (const [k, v] of Object.entries(incoming)) if (!(k in stored)) stored[k] = v;
        imported = true;
        return undefined as never;
      }
      case 'set_ui_pref': {
        const { key, value } = args as { key: string; value: string | null };
        if (value === null) delete stored[key]; else stored[key] = value;
        return undefined as never;
      }
    }
    throw new Error(`unexpected ${command}`);
  });
  return { invoke, calls, stored };
}

describe('prefs', () => {
  beforeEach(() => {
    localStorage.clear();
    resetPrefs();
  });

  it('loads saved preferences before anything reads them', async () => {
    const backend = fakeBackend({ 'anivault-library-filter': 'completed' }, true);
    await initPrefs(backend.invoke);
    expect(getPref('anivault-library-filter')).toBe('completed');
    expect(getPref('missing')).toBeNull();
  });

  it('imports the old localStorage values once, keeping newer database values', async () => {
    localStorage.setItem('anivault-library-filter', 'plan_to_watch');
    localStorage.setItem('anivault-start-page', 'library');
    localStorage.setItem('unrelated', 'x');
    const backend = fakeBackend({ 'anivault-start-page': 'season' }, false);

    await initPrefs(backend.invoke);

    expect(backend.calls.find((c) => c.command === 'import_ui_prefs')?.args).toEqual({
      prefs: { 'anivault-library-filter': 'plan_to_watch', 'anivault-start-page': 'library' },
    });
    expect(getPref('anivault-library-filter')).toBe('plan_to_watch');
    expect(getPref('anivault-start-page')).toBe('season');
    expect(getPref('unrelated')).toBeNull();
  });

  it('skips the import once it has run', async () => {
    localStorage.setItem('anivault-library-filter', 'plan_to_watch');
    const backend = fakeBackend({}, true);
    await initPrefs(backend.invoke);
    expect(backend.calls.map((c) => c.command)).toEqual(['get_ui_prefs']);
    expect(getPref('anivault-library-filter')).toBeNull();
  });

  it('reads a write back at once and saves it to the database', async () => {
    const backend = fakeBackend({}, true);
    await initPrefs(backend.invoke);

    setPref('anivault-library-filter', 'watching');
    expect(getPref('anivault-library-filter')).toBe('watching');
    await flushPrefs();
    expect(backend.stored['anivault-library-filter']).toBe('watching');

    removePref('anivault-library-filter');
    expect(getPref('anivault-library-filter')).toBeNull();
    await flushPrefs();
    expect(backend.stored).toEqual({});
  });

  it('saves only the latest of several quick writes to one key, in order', async () => {
    const backend = fakeBackend({}, true);
    await initPrefs(backend.invoke);

    setPref('anivault-library-query', 'a');
    setPref('anivault-library-query', 'ab');
    setPref('anivault-library-query', 'abc');
    await flushPrefs();

    expect(backend.stored['anivault-library-query']).toBe('abc');
    const writes = backend.calls.filter((c) => c.command === 'set_ui_pref');
    expect(writes.at(-1)?.args).toEqual({ key: 'anivault-library-query', value: 'abc' });
  });

  it('warns when a save fails, and keeps the value for this session', async () => {
    const backend = fakeBackend({}, true);
    await initPrefs(backend.invoke);
    backend.invoke.mockImplementation(async () => { throw new Error('database is locked'); });

    setPref('anivault-library-filter', 'dropped');
    await flushPrefs();

    expect(getPref('anivault-library-filter')).toBe('dropped');
    expect(get(prefsError)).toEqual({ kind: 'save', detail: expect.stringContaining('database is locked') });
  });

  it('retries a failed save and clears the warning once it goes through', async () => {
    const backend = fakeBackend({}, true);
    await initPrefs(backend.invoke);
    const working = backend.invoke.getMockImplementation()!;
    backend.invoke.mockImplementation(async () => { throw new Error('database is locked'); });
    setPref('anivault-library-filter', 'dropped');
    await flushPrefs();
    expect(get(prefsError)?.kind).toBe('save');

    backend.invoke.mockImplementation(working);
    retryFailedSaves();
    await flushPrefs();

    expect(backend.stored['anivault-library-filter']).toBe('dropped');
    expect(get(prefsError)).toBeNull();
  });

  it('clears a save warning on its own when a later save of that setting works', async () => {
    const backend = fakeBackend({}, true);
    await initPrefs(backend.invoke);
    const working = backend.invoke.getMockImplementation()!;
    backend.invoke.mockImplementationOnce(async () => { throw new Error('busy'); });
    setPref('anivault-library-filter', 'dropped');
    await flushPrefs();
    expect(get(prefsError)?.kind).toBe('save');

    backend.invoke.mockImplementation(working);
    setPref('anivault-library-filter', 'watching');
    await flushPrefs();
    expect(get(prefsError)).toBeNull();
  });

  it('warns when the saved preferences cannot be loaded', async () => {
    const invoke = vi.fn(async () => { throw new Error('no database'); });
    await initPrefs(invoke);
    expect(get(prefsError)).toEqual({ kind: 'load', detail: expect.stringContaining('no database') });
    expect(getPref('anivault-library-filter')).toBeNull();
  });

  it('falls back to the old localStorage values when the import fails', async () => {
    localStorage.setItem('anivault-library-filter', 'on_hold');
    const backend = fakeBackend({}, false);
    backend.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_ui_prefs') return { prefs: {}, imported: false } as never;
      throw new Error('disk full');
    });

    await initPrefs(backend.invoke);

    expect(getPref('anivault-library-filter')).toBe('on_hold');
    expect(get(prefsError)).toEqual({ kind: 'import', detail: expect.stringContaining('disk full') });
  });
});

describe('describePrefsProblem', () => {
  const kinds = ['load', 'import', 'save'] as const;

  it.each(kinds)('gives a plain %s message with no technical text in it', (kind) => {
    const text = describePrefsProblem({ kind, detail: 'SqliteError: database is locked (code 5)' });
    expect(text.message).not.toMatch(/sqlite|locked|code|error/i);
    expect(text.message.length).toBeGreaterThan(20);
    expect(text.action).toBe('Try again');
  });
});
