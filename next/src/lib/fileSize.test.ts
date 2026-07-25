import { describe, it, expect } from 'vitest';
import { formatBytes } from './fileSize';

describe('formatBytes', () => {
  it('formats across units', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(5 * 1024 * 1024)).toBe('5.0 MB');
    expect(formatBytes(12.4 * 1024 * 1024 * 1024)).toBe('12.4 GB');
  });
  it('rounds large values within a unit to whole numbers', () => {
    expect(formatBytes(250 * 1024 * 1024)).toBe('250 MB');
  });
});
