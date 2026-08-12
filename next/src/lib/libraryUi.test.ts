import { describe, expect, it } from 'vitest';
import { normalizeStatusFilter } from './libraryUi';

const KNOWN = [null, 'watching', 'completed', 'on_hold', 'dropped', 'plan_to_watch'];

describe('normalizeStatusFilter', () => {
  it('passes a known status through', () => {
    expect(normalizeStatusFilter('watching', KNOWN)).toBe('watching');
  });

  it('maps a removed status to All', () => {
    expect(normalizeStatusFilter('unlisted', KNOWN)).toBeNull();
  });

  it('maps an empty string to All', () => {
    expect(normalizeStatusFilter('', KNOWN)).toBeNull();
  });

  it('maps null to All', () => {
    expect(normalizeStatusFilter(null, KNOWN)).toBeNull();
  });

  it('maps any unrecognised value to All', () => {
    expect(normalizeStatusFilter('nonsense', KNOWN)).toBeNull();
  });
});
