import { describe, expect, it } from 'vitest';
import { cleanupItems, triggersCleanup, DEFAULT_CLEANUP_SETTINGS, type CleanupSettings } from './dropCleanup';

const settings: CleanupSettings = { ...DEFAULT_CLEANUP_SETTINGS };

describe('triggersCleanup', () => {
  it('prompts when a show moves into a chosen status', () => {
    expect(triggersCleanup(settings, 'watching', 'dropped')).toBe(true);
    expect(triggersCleanup(settings, undefined, 'dropped')).toBe(true);
  });

  it('stays quiet for other statuses, no-op moves, and with both offers off', () => {
    expect(triggersCleanup(settings, 'watching', 'completed')).toBe(false);
    expect(triggersCleanup(settings, 'dropped', 'dropped')).toBe(false);
    expect(triggersCleanup({ ...settings, offerSonarr: false, offerFiles: false }, 'watching', 'dropped')).toBe(false);
  });

  it('honours extra statuses', () => {
    expect(triggersCleanup({ ...settings, statuses: ['dropped', 'completed'] }, 'watching', 'completed')).toBe(true);
  });
});

describe('cleanupItems', () => {
  const shows = [
    { animeId: 1, title: 'In Sonarr with files' },
    { animeId: 2, title: 'Nothing to clean' },
    { animeId: 3, title: 'Files only' },
  ];
  const previews = [
    { anime_id: 1, sonarr: { sonarr_id: 10, title: 'A', monitored: true }, file_count: 4, total_bytes: 4000 },
    { anime_id: 2, sonarr: null, file_count: 0, total_bytes: 0 },
    { anime_id: 3, sonarr: null, file_count: 2, total_bytes: 100 },
  ];

  it('drops shows with nothing to clean up', () => {
    expect(cleanupItems(shows, previews, settings).map((i) => i.animeId)).toEqual([1, 3]);
  });

  it('hides the parts whose offer is turned off', () => {
    const items = cleanupItems(shows, previews, { ...settings, offerFiles: false });
    expect(items).toEqual([{ animeId: 1, title: 'In Sonarr with files', sonarr: previews[0]!.sonarr, fileCount: 0, totalBytes: 0 }]);
    expect(cleanupItems(shows, previews, { ...settings, offerSonarr: false }).map((i) => i.animeId)).toEqual([1, 3]);
  });
});
