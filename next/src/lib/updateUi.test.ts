// @vitest-environment jsdom
import { describe, expect, it, beforeEach } from 'vitest';
import { dismissUpdate, loadDismissedUpdate, shouldShowUpdate } from './updateUi';

const info = (latest: string, available = true) => ({
  current: '1.0.7', latest, url: 'https://example.com', update_available: available,
});

describe('update banner logic', () => {
  beforeEach(() => localStorage.clear());

  it('shows when an update is available and not dismissed', () => {
    expect(shouldShowUpdate(info('v1.0.8'), null)).toBe(true);
  });

  it('never shows when no update is available', () => {
    expect(shouldShowUpdate(info('v1.0.7', false), null)).toBe(false);
  });

  it('stays hidden for a dismissed version but reappears for the next one', () => {
    expect(shouldShowUpdate(info('v1.0.8'), 'v1.0.8')).toBe(false);
    expect(shouldShowUpdate(info('v1.0.9'), 'v1.0.8')).toBe(true);
  });

  it('round-trips dismissal through localStorage', () => {
    expect(loadDismissedUpdate()).toBeNull();
    dismissUpdate('v1.0.8');
    expect(loadDismissedUpdate()).toBe('v1.0.8');
  });
});
