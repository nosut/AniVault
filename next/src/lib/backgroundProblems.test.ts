import { describe, it, expect, vi } from 'vitest';

vi.mock('./api', () => ({
  getBackgroundProblems: vi.fn(async () => []),
  scanLibraryFolders: vi.fn(async () => ({})),
  triggerSync: vi.fn(async () => ({})),
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
    const [sync, watcher] = visibleNotices([problem('sync'), problem('library_watcher')], {}, onRetried);
    expect(watcher!.retry).toBeUndefined();
    sync!.retry!();
    await new Promise((r) => setTimeout(r, 0));
    expect(triggerSync).toHaveBeenCalled();
    expect(onRetried).toHaveBeenCalled();
  });
});
