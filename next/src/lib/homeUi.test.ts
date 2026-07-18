import { describe, expect, it } from 'vitest';
import { airedAgoShort, isSameLocalDay, syncPill, todayRowLabel } from './homeUi';

const NOON = Math.floor(new Date(2026, 6, 17, 12, 0, 0).getTime() / 1000);

describe('isSameLocalDay', () => {
  it('is true within the same local day', () => {
    const morning = Math.floor(new Date(2026, 6, 17, 1, 0, 0).getTime() / 1000);
    expect(isSameLocalDay(morning, NOON)).toBe(true);
  });

  it('is false across days', () => {
    const yesterday = Math.floor(new Date(2026, 6, 16, 23, 0, 0).getTime() / 1000);
    expect(isSameLocalDay(yesterday, NOON)).toBe(false);
  });
});

describe('todayRowLabel', () => {
  it('shows a countdown for upcoming episodes', () => {
    expect(todayRowLabel(NOON + 2 * 3600 + 14 * 60, NOON)).toBe('in 2h 14m');
    expect(todayRowLabel(NOON + 20 * 60, NOON)).toBe('in 20m');
  });

  it('shows how long ago an aired episode aired', () => {
    expect(todayRowLabel(NOON - 9 * 3600, NOON)).toBe('Aired · 9h ago');
    expect(todayRowLabel(NOON - 30 * 60, NOON)).toBe('Aired · 30m ago');
  });
});

describe('airedAgoShort', () => {
  it('is today for the same local day', () => {
    expect(airedAgoShort(NOON - 3600, NOON)).toBe('today');
  });

  it('counts days otherwise', () => {
    expect(airedAgoShort(NOON - 4 * 86400, NOON)).toBe('4d ago');
  });
});

describe('syncPill', () => {
  it('reports not connected', () => {
    expect(syncPill(false, null)).toEqual({ text: 'not connected', ok: false });
  });

  it('reports failures first', () => {
    expect(syncPill(true, { pending: 2, failed: 3, blocked: 0, last_sync_at: null }))
      .toEqual({ text: '3 failed', ok: false });
  });

  it('reports pending pushes', () => {
    expect(syncPill(true, { pending: 2, failed: 0, blocked: 0, last_sync_at: null }))
      .toEqual({ text: '2 pending', ok: true });
  });

  it('reports synced when idle', () => {
    expect(syncPill(true, { pending: 0, failed: 0, blocked: 0, last_sync_at: 1000 }))
      .toEqual({ text: 'synced', ok: true });
  });
});
