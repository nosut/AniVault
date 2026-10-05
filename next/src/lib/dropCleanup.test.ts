import { describe, expect, it } from 'vitest';
import type { StatusChangePreview } from './api';
import { cleanupItems, promptFor, requestsFor, resumeItems, sharedWarnings, DEFAULT_CLEANUP_SETTINGS, type CleanupSettings } from './dropCleanup';

const settings: CleanupSettings = { ...DEFAULT_CLEANUP_SETTINGS };

function preview(anime_id: number, extra: Partial<StatusChangePreview> = {}): StatusChangePreview {
  return { anime_id, sonarr: null, sonarr_missing: false, sonarr_error: null, file_count: 0, total_bytes: 0, ...extra };
}

const series = (sonarr_id: number, monitored: boolean, shared_with: string[] = []) => ({ sonarr_id, title: `Series ${sonarr_id}`, monitored, shared_with });

describe('promptFor', () => {
  it('offers cleanup when a show moves into a chosen status', () => {
    expect(promptFor(settings, 'watching', 'dropped')).toBe('drop');
    expect(promptFor(settings, undefined, 'dropped')).toBe('drop');
  });

  it('stays quiet for other statuses, no-op moves, and with both cleanup offers off', () => {
    expect(promptFor(settings, 'watching', 'completed')).toBeNull();
    expect(promptFor(settings, 'dropped', 'dropped')).toBeNull();
    expect(promptFor({ ...settings, offerSonarr: false, offerFiles: false }, 'watching', 'dropped')).toBeNull();
  });

  it('does not offer cleanup again when moving between two cleanup statuses', () => {
    const both = { ...settings, statuses: ['dropped', 'on_hold'] };
    expect(promptFor(both, 'dropped', 'on_hold')).toBeNull();
    expect(promptFor(both, 'on_hold', 'watching')).toBe('resume');
  });

  it('honours extra statuses', () => {
    expect(promptFor({ ...settings, statuses: ['dropped', 'completed'] }, 'watching', 'completed')).toBe('drop');
  });

  it('offers the reverse when a show leaves a chosen status for Watching or Planning', () => {
    expect(promptFor(settings, 'dropped', 'watching')).toBe('resume');
    expect(promptFor(settings, 'dropped', 'plan_to_watch')).toBe('resume');
  });

  it('does not offer the reverse from other statuses, to other statuses, or when turned off', () => {
    expect(promptFor(settings, 'completed', 'watching')).toBeNull();
    expect(promptFor(settings, null, 'watching')).toBeNull();
    expect(promptFor(settings, 'dropped', 'completed')).toBeNull();
    expect(promptFor({ ...settings, offerResume: false }, 'dropped', 'watching')).toBeNull();
  });
});

describe('cleanupItems', () => {
  const shows = [
    { animeId: 1, title: 'In Sonarr with files' },
    { animeId: 2, title: 'Nothing to clean' },
    { animeId: 3, title: 'Files only' },
  ];
  const previews = [
    preview(1, { sonarr: series(10, true), file_count: 4, total_bytes: 4000 }),
    preview(2),
    preview(3, { file_count: 2, total_bytes: 100 }),
  ];

  it('drops shows with nothing to clean up', () => {
    expect(cleanupItems(shows, previews, settings).map((i) => i.animeId)).toEqual([1, 3]);
  });

  it('hides the parts whose offer is turned off', () => {
    const items = cleanupItems(shows, previews, { ...settings, offerFiles: false });
    expect(items).toEqual([{ animeId: 1, title: 'In Sonarr with files', sonarr: series(10, true), fileCount: 0, totalBytes: 0 }]);
    expect(cleanupItems(shows, previews, { ...settings, offerSonarr: false }).map((i) => i.animeId)).toEqual([1, 3]);
  });
});

describe('sharedWarnings', () => {
  it('names active shows on the same series, except ones being dropped together', () => {
    const items = [
      { animeId: 1, title: 'Part 1', sonarr: series(10, true, ['Part 2', 'Part 3']), fileCount: 0, totalBytes: 0 },
      { animeId: 2, title: 'Part 2', sonarr: series(10, true, ['Part 1', 'Part 3']), fileCount: 0, totalBytes: 0 },
    ];
    expect(sharedWarnings(items)).toEqual(['Part 3']);
  });
});

describe('resumeItems', () => {
  it('keeps unmonitored and missing shows, skipping monitored and untracked ones', () => {
    const shows = [1, 2, 3, 4].map((id) => ({ animeId: id, title: `Show ${id}` }));
    const items = resumeItems(shows, [
      preview(1, { sonarr: series(10, false) }),
      preview(2, { sonarr: series(20, true) }),
      preview(3, { sonarr_missing: true }),
      preview(4),
    ]);
    expect(items.map((i) => [i.animeId, i.kind])).toEqual([[1, 'unmonitored'], [3, 'missing']]);
  });

  it('lists a series shared by two shows only once', () => {
    const shows = [1, 2].map((id) => ({ animeId: id, title: `Show ${id}` }));
    const items = resumeItems(shows, [preview(1, { sonarr: series(10, false) }), preview(2, { sonarr: series(10, false) })]);
    expect(items.map((i) => i.animeId)).toEqual([1]);
  });
});

describe('requestsFor', () => {
  const change = (animeId: number, from: string | null, to: string) => ({ animeId, title: `Show ${animeId}`, from, to });
  const withCompleted = { ...settings, statuses: ['dropped', 'completed'] };

  it('groups a batch into one prompt per kind and target status', () => {
    const reqs = requestsFor(withCompleted, [
      change(1, 'watching', 'dropped'),
      change(2, 'watching', 'dropped'),
      change(3, 'watching', 'completed'),
      change(4, 'dropped', 'watching'),
      change(5, 'watching', 'on_hold'),
    ]);
    expect(reqs).toEqual([
      { kind: 'drop', status: 'dropped', shows: [{ animeId: 1, title: 'Show 1' }, { animeId: 2, title: 'Show 2' }] },
      { kind: 'drop', status: 'completed', shows: [{ animeId: 3, title: 'Show 3' }] },
      { kind: 'resume', status: 'watching', shows: [{ animeId: 4, title: 'Show 4' }] },
    ]);
  });

  it('reads several moves of one show as a single move from its first status', () => {
    expect(requestsFor(settings, [change(1, 'dropped', 'watching'), change(1, 'watching', 'dropped')])).toEqual([]);
    expect(requestsFor(settings, [change(1, 'watching', 'on_hold'), change(1, 'on_hold', 'dropped')])).toEqual([
      { kind: 'drop', status: 'dropped', shows: [{ animeId: 1, title: 'Show 1' }] },
    ]);
  });
});
