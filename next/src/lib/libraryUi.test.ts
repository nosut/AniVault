import { describe, expect, it } from 'vitest';
import {
  asDisplayRows, flattenGroups, groupBySeason, normalizeStatusFilter,
  seasonGroupKey, seasonGroupLabel, seasonSortVal,
} from './libraryUi';

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

const CURRENT = { season: 'SUMMER', year: 2026 };
const show = (title: string, season: string | null, season_year: number | null) =>
  ({ title, season, season_year });

describe('seasonGroupKey', () => {
  it('combines season and year', () => {
    expect(seasonGroupKey(show('a', 'FALL', 2026))).toBe('fall2026');
  });

  it('is tba when the year is missing', () => {
    expect(seasonGroupKey(show('a', 'FALL', null))).toBe('tba');
  });

  it('is tba when the season is missing', () => {
    expect(seasonGroupKey(show('a', null, 2026))).toBe('tba');
  });
});

describe('seasonGroupLabel', () => {
  it('renders a readable season', () => {
    expect(seasonGroupLabel(show('a', 'FALL', 2026))).toBe('Fall 2026');
  });

  it('renders TBA when undated', () => {
    expect(seasonGroupLabel(show('a', null, null))).toBe('TBA');
  });
});

describe('groupBySeason', () => {
  it('returns nothing for an empty list', () => {
    expect(groupBySeason([], CURRENT)).toEqual([]);
  });

  it('orders groups ascending by season', () => {
    const groups = groupBySeason([
      show('c', 'SPRING', 2027),
      show('a', 'FALL', 2026),
      show('b', 'WINTER', 2027),
    ], CURRENT);
    expect(groups.map((g) => g.label)).toEqual(['Fall 2026', 'Winter 2027', 'Spring 2027']);
  });

  it('collects every show for a season into one group', () => {
    const groups = groupBySeason([
      show('a', 'FALL', 2026),
      show('b', 'FALL', 2026),
    ], CURRENT);
    expect(groups).toHaveLength(1);
    expect(groups[0]!.entries.map((e) => e.title)).toEqual(['a', 'b']);
  });

  it('puts TBA last regardless of input order', () => {
    const groups = groupBySeason([
      show('a', null, null),
      show('b', 'FALL', 2026),
    ], CURRENT);
    expect(groups.map((g) => g.key)).toEqual(['fall2026', 'tba']);
  });

  it('marks the soonest future season as next, and only that one', () => {
    const groups = groupBySeason([
      show('a', 'FALL', 2026),
      show('b', 'WINTER', 2027),
      show('c', null, null),
    ], CURRENT);
    expect(groups.map((g) => g.chip)).toEqual(['Next season', null, null]);
  });

  it('says This season when the soonest group is the current one', () => {
    const groups = groupBySeason([
      show('a', 'SUMMER', 2026),
      show('b', 'FALL', 2026),
    ], CURRENT);
    expect(groups.map((g) => g.chip)).toEqual(['This season', null]);
  });

  it('never marks a past season', () => {
    const groups = groupBySeason([
      show('a', 'WINTER', 2026),
      show('b', 'FALL', 2026),
    ], CURRENT);
    expect(groups.map((g) => g.chip)).toEqual([null, 'Next season']);
  });

  it('marks nothing when every group is in the past', () => {
    expect(groupBySeason([show('a', 'WINTER', 2026)], CURRENT).map((g) => g.chip)).toEqual([null]);
  });

  it('marks nothing when the only group is TBA', () => {
    expect(groupBySeason([show('a', null, null)], CURRENT).map((g) => g.chip)).toEqual([null]);
  });
});

describe('seasonSortVal', () => {
  it('orders within a year by season', () => {
    expect(seasonSortVal(show('a', 'WINTER', 2026)))
      .toBeLessThan(seasonSortVal(show('b', 'FALL', 2026)));
  });

  it('sorts undated shows last', () => {
    expect(seasonSortVal(show('a', null, null))).toBe(Number.POSITIVE_INFINITY);
  });
});

describe('flattenGroups', () => {
  const groups = groupBySeason([
    show('a', 'FALL', 2026),
    show('b', 'FALL', 2026),
    show('c', 'WINTER', 2027),
  ], CURRENT);

  it('emits a header before each group and its entries after', () => {
    const rows = flattenGroups(groups, {});
    expect(rows.map((r) => r.kind)).toEqual(['group', 'entry', 'entry', 'group', 'entry']);
  });

  it('omits the entries of a collapsed group but keeps its header', () => {
    const rows = flattenGroups(groups, { fall2026: true });
    expect(rows.map((r) => r.kind)).toEqual(['group', 'group', 'entry']);
  });

  it('treats a season absent from the map as open', () => {
    expect(flattenGroups(groups, { winter2027: false })).toHaveLength(5);
  });

  it('returns nothing for no groups', () => {
    expect(flattenGroups([], {})).toEqual([]);
  });
});

describe('asDisplayRows', () => {
  it('wraps a flat list as entry rows', () => {
    expect(asDisplayRows([show('a', 'FALL', 2026)]))
      .toEqual([{ kind: 'entry', entry: show('a', 'FALL', 2026) }]);
  });

  it('returns nothing for an empty list', () => {
    expect(asDisplayRows([])).toEqual([]);
  });
});
