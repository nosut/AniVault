import { describe, expect, it } from 'vitest';
import { formatWantedTags, parseWantedTags } from './sonarrUi';

describe('Sonarr wanted tags input', () => {
  it('splits a comma-separated list, trimming and dropping blanks and repeats', () => {
    expect(parseWantedTags(' 1 - nosut, mine ,, Mine,anime ')).toEqual(['1 - nosut', 'mine', 'anime']);
  });

  it('reads an empty input as no preference, so the default applies', () => {
    expect(parseWantedTags('')).toBeNull();
    expect(parseWantedTags(' , ')).toBeNull();
  });

  it('formats stored tags back into the input', () => {
    expect(formatWantedTags(['1 - nosut', 'mine'])).toBe('1 - nosut, mine');
    expect(formatWantedTags(null)).toBe('');
  });
});
