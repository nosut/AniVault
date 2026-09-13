import { describe, expect, it } from 'vitest';
import { selectedWantedTags, toggleWantedTag, wantedTagRows } from './sonarrUi';

const sonarrTags = [
  { id: 1, label: '1 - nosut', series_count: 40 },
  { id: 2, label: 'anime', series_count: 120 },
  { id: 3, label: 'mine', series_count: 12 },
];

describe('Sonarr import tag selector', () => {
  it('lists every Sonarr tag and checks the saved ones, ignoring case and spacing', () => {
    const rows = wantedTagRows(sonarrTags, [' Anime ']);
    expect(rows).toEqual([
      { label: '1 - nosut', seriesCount: 40, checked: false, missing: false },
      { label: 'anime', seriesCount: 120, checked: true, missing: false },
      { label: 'mine', seriesCount: 12, checked: false, missing: false },
    ]);
  });

  it('with nothing saved, checks the default tags that exist in this Sonarr', () => {
    const rows = wantedTagRows([{ id: 3, label: 'Mine', series_count: 12 }, { id: 2, label: 'anime', series_count: 1 }], null);
    expect(rows.map((r) => [r.label, r.checked, r.missing])).toEqual([
      ['Mine', true, false],
      ['anime', false, false],
    ]);
  });

  it('keeps a saved tag that Sonarr no longer has, flagged so it can be unchecked', () => {
    const rows = wantedTagRows(sonarrTags, ['mine', 'old-tag']);
    expect(rows.at(-1)).toEqual({ label: 'old-tag', seriesCount: null, checked: true, missing: true });
  });

  it('an empty saved list means no filter: nothing is checked', () => {
    expect(wantedTagRows(sonarrTags, []).some((r) => r.checked)).toBe(false);
  });

  it('toggling produces the list to save, dropping a missing tag once unchecked', () => {
    const rows = wantedTagRows(sonarrTags, ['mine', 'old-tag']);
    const withAnime = toggleWantedTag(rows, 'anime');
    expect(selectedWantedTags(withAnime)).toEqual(['anime', 'mine', 'old-tag']);

    const withoutOld = toggleWantedTag(withAnime, 'old-tag');
    expect(withoutOld.some((r) => r.label === 'old-tag')).toBe(false);
    expect(selectedWantedTags(withoutOld)).toEqual(['anime', 'mine']);
  });
});
