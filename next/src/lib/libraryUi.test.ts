import { describe, expect, it } from 'vitest';
import {
  airingSoonMarker, asDisplayRows, flattenGroups, formatAiringCountdown, groupBySeason,
  nextAiringByAnime, nextAiringSortVal, normalizeStatusFilter,
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

const NOW = 1_760_000_000;
const air = (anime_id: number, next_episode: number | null, airing_at: number | null) =>
  ({ anime_id, next_episode, airing_at });

describe('nextAiringByAnime', () => {
  it('picks the earliest future entry for a show', () => {
    const map = nextAiringByAnime([
      air(1, 9, NOW + 8 * 86400),
      air(1, 8, NOW + 86400),
      air(1, 10, NOW + 15 * 86400),
    ], NOW);
    expect(map.get(1)?.next_episode).toBe(8);
  });

  it('ignores entries that have already aired', () => {
    const map = nextAiringByAnime([
      air(1, 7, NOW - 86400),
      air(1, 8, NOW + 86400),
    ], NOW);
    expect(map.get(1)?.next_episode).toBe(8);
  });

  it('omits a show whose entries are all in the past', () => {
    expect(nextAiringByAnime([air(1, 7, NOW - 86400)], NOW).has(1)).toBe(false);
  });

  it('omits an entry with no airing time', () => {
    expect(nextAiringByAnime([air(1, 7, null)], NOW).has(1)).toBe(false);
  });

  it('keeps shows separate', () => {
    const map = nextAiringByAnime([
      air(1, 8, NOW + 86400),
      air(2, 3, NOW + 2 * 86400),
    ], NOW);
    expect(map.get(1)?.next_episode).toBe(8);
    expect(map.get(2)?.next_episode).toBe(3);
  });

  it('is empty for no entries', () => {
    expect(nextAiringByAnime([], NOW).size).toBe(0);
  });
});

describe('formatAiringCountdown', () => {
  it('renders days and hours', () => {
    expect(formatAiringCountdown(6 * 86400 + 14 * 3600)).toBe('6d 14h');
  });

  it('renders hours and minutes under a day', () => {
    expect(formatAiringCountdown(14 * 3600 + 3 * 60)).toBe('14h 3m');
  });

  it('renders minutes under an hour', () => {
    expect(formatAiringCountdown(3 * 60)).toBe('3m');
  });

  it('renders airing now at or past zero', () => {
    expect(formatAiringCountdown(0)).toBe('airing now');
    expect(formatAiringCountdown(-60)).toBe('airing now');
  });
});

describe('nextAiringSortVal', () => {
  it('returns the airing time when there is one', () => {
    const map = nextAiringByAnime([air(1, 8, NOW + 86400)], NOW);
    expect(nextAiringSortVal(1, map)).toBe(NOW + 86400);
  });

  it('returns Infinity for a show with no airing, so it sorts last', () => {
    expect(nextAiringSortVal(1, nextAiringByAnime([], NOW))).toBe(Number.POSITIVE_INFINITY);
  });
});

describe('airingSoonMarker', () => {
  const DAY = 86400;
  const ptw = (airing_status: string | null = 'NOT_YET_RELEASED', status = 'plan_to_watch') =>
    ({ anime_id: 1, status, airing_status });
  const mapOf = (next_episode: number | null, airing_at: number) =>
    nextAiringByAnime([air(1, next_episode, airing_at)], NOW);
  const none = nextAiringByAnime([], NOW);

  it('marks a premiere inside the week', () => {
    expect(airingSoonMarker(ptw(), mapOf(1, NOW + 3 * DAY + 4 * 3600), NOW))
      .toEqual({ kind: 'premiere', label: 'Premieres in 3d 4h', soon: false });
  });

  it('marks a premiere exactly a week out', () => {
    expect(airingSoonMarker(ptw(), mapOf(1, NOW + 7 * DAY), NOW)?.kind).toBe('premiere');
  });

  it('leaves a premiere more than a week out unmarked', () => {
    expect(airingSoonMarker(ptw(), mapOf(1, NOW + 7 * DAY + 60), NOW)).toBeNull();
  });

  it('flags a premiere under a day away as soon', () => {
    expect(airingSoonMarker(ptw(), mapOf(1, NOW + 5 * 3600), NOW))
      .toEqual({ kind: 'premiere', label: 'Premieres in 5h 0m', soon: true });
  });

  it('marks a show already mid-season with its next episode', () => {
    expect(airingSoonMarker(ptw('RELEASING'), mapOf(5, NOW + 2 * DAY), NOW))
      .toEqual({ kind: 'airing', label: 'Airing · Ep 5 in 2d 0h', soon: false });
  });

  it('keeps a mid-season show marked when its next episode is over a week out', () => {
    expect(airingSoonMarker(ptw('RELEASING'), mapOf(5, NOW + 12 * DAY), NOW)?.kind).toBe('airing');
  });

  it('trusts the calendar over a stale local airing status', () => {
    expect(airingSoonMarker(ptw('NOT_YET_RELEASED'), mapOf(4, NOW + DAY), NOW)?.kind).toBe('airing');
  });

  it('marks a releasing show with nothing upcoming as plain Airing', () => {
    expect(airingSoonMarker(ptw('RELEASING'), none, NOW))
      .toEqual({ kind: 'airing', label: 'Airing', soon: false });
  });

  it('leaves a finished show unmarked', () => {
    expect(airingSoonMarker(ptw('FINISHED'), none, NOW)).toBeNull();
  });

  it('leaves an unannounced show unmarked', () => {
    expect(airingSoonMarker(ptw(null), none, NOW)).toBeNull();
  });

  it('only marks plan-to-watch entries', () => {
    expect(airingSoonMarker(ptw('RELEASING', 'watching'), mapOf(5, NOW + DAY), NOW)).toBeNull();
  });
});
