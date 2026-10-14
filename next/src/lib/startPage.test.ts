// @vitest-environment jsdom
import { describe, expect, it, beforeEach } from 'vitest';
import { loadStartPage, saveStartPage, START_PAGE_OPTIONS } from './startPage';
import { resetPrefs, setPref } from './prefs';

describe('start page preference', () => {
  beforeEach(() => resetPrefs());

  it('defaults to the dashboard', () => {
    expect(loadStartPage()).toBe('dashboard');
  });

  it('round-trips a saved choice', () => {
    saveStartPage('library');
    expect(loadStartPage()).toBe('library');
  });

  it('falls back to dashboard for unknown saved values', () => {
    setPref('anivault-start-page', 'detail');
    expect(loadStartPage()).toBe('dashboard');
    setPref('anivault-start-page', 'garbage');
    expect(loadStartPage()).toBe('dashboard');
  });

  it('offers only real navigable views', () => {
    const values = START_PAGE_OPTIONS.map((o) => o.value);
    expect(values).toContain('dashboard');
    expect(values).toContain('library');
    expect(values).not.toContain('detail');
    expect(values).not.toContain('settings');
  });
});
