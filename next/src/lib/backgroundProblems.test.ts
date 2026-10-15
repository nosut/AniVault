import { describe, it, expect, vi } from 'vitest';

vi.mock('./api', () => ({
  getBackgroundProblems: vi.fn(async () => []),
  scanLibraryFolders: vi.fn(async () => ({})),
  triggerSync: vi.fn(async () => ({})),
  setTrackingEnabled: vi.fn(async () => ({})),
  refreshCalendar: vi.fn(async () => []),
}));

import { triggerSync } from './api';
import { visibleNotices } from './backgroundProblems';

const problem = (area: string, since = 100) => ({
  area,
  summary: "Changes aren't reaching AniList.",
  detail: 'error sending request for url (https://graphql.anilist.co/)',
  since,
});

describe('visibleNotices', () => {
  it('explains a problem in plain words with the raw error kept for details', () => {
    const [n] = visibleNotices([problem('sync')], {});
    expect(n!.message).toMatch(/^Changes aren't reaching AniList\. AniVault couldn't connect to the internet/);
    expect(n!.details).toContain('graphql.anilist.co');
  });

  it('hides a dismissed problem until it happens again', () => {
    expect(visibleNotices([problem('sync', 100)], { sync: 100 })).toEqual([]);
    expect(visibleNotices([problem('sync', 200)], { sync: 100 })).toHaveLength(1);
  });

  it('offers a retry only where one makes sense, and it runs the fix', async () => {
    const onRetried = vi.fn();
    const [sync, watcher] = visibleNotices([problem('sync'), problem('library_watcher')], {}, {}, onRetried);
    expect(watcher!.retry).toBeUndefined();
    sync!.retry!();
    await new Promise((r) => setTimeout(r, 0));
    expect(triggerSync).toHaveBeenCalled();
    expect(onRetried).toHaveBeenCalled();
  });

  it("uses the area's own next step instead of competing cause advice", () => {
    const [n] = visibleNotices([{
      area: 'sync_queue',
      summary: "A list change couldn't be queued for AniList, so AniList may not show it.",
      detail: 'error returned from database: (code: 5) database is locked',
      since: 1,
    }], {});
    expect(n!.message).toBe("A list change couldn't be queued for AniList, so AniList may not show it. Make the change again to resend it.");
  });

  it('does not say Try again when there is no button and the cause is unknown', () => {
    const [n] = visibleNotices([{ area: 'worker', summary: 'AniList sync stopped unexpectedly and was restarted.', detail: 'panic', since: 1 }], {});
    expect(n!.message).toBe('AniList sync stopped unexpectedly and was restarted. Show details has the technical information to report.');
    expect(n!.retry).toBeUndefined();
  });

  it('says when a retry still failed', () => {
    const [n] = visibleNotices([problem('sync')], {}, { sync: true });
    expect(n!.message).toMatch(/^Still not working\. Changes aren't reaching AniList\./);
  });
});
