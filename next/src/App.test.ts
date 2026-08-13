// @vitest-environment jsdom
//
// Covers the fix where SeasonView used to unmount when the user opened a
// card's detail view (currentView flips out of the {#if} chain it lived in),
// discarding the in-memory "new since last visit" band. Remounting on Back
// re-ran load() -> diff_season, which re-baselines the season and wipes the
// band before the user has looked at the rest of it. SeasonView is now kept
// permanently mounted and merely hidden with display:none/contents.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, unmount, flushSync, tick } from 'svelte';

const seasonEntries = [
  { id: 1, title: 'Established Show', image_url: null, episodes: 12, status: 'NOT_YET_RELEASED', format: 'TV', average_score: null, popularity: 5000 },
  { id: 7, title: 'Brand New Show', image_url: null, episodes: 12, status: 'NOT_YET_RELEASED', format: 'TV', average_score: null, popularity: 100 },
];

// App.svelte pulls in every top-level view, each of which imports its own
// slice of ./lib/api. Only SeasonView and DetailView actually get exercised
// here (the rest never mount because currentView never points at them), but
// every export still has to resolve to something await-able so components
// that call it in a best-effort try/catch (or a bare `.then()`) don't blow
// up. Auto-mocking every function export to `async () => undefined` covers
// that; the handful the test cares about get their own return value below.
vi.mock('./lib/api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./lib/api')>();
  const mocked: Record<string, unknown> = { ...actual };
  for (const [key, value] of Object.entries(actual as Record<string, unknown>)) {
    if (typeof value === 'function') mocked[key] = vi.fn(async () => undefined);
  }
  return mocked;
});

import { getSeasonAnime, diffSeason, getLibraryIds, fetchAnimeDetail, getTrackingStatus, searchLibrary, getCalendar } from './lib/api';
import App from './App.svelte';

async function settle() {
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
  flushSync();
}

describe('App keeps the Seasons view mounted across a detail round trip', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    document.body.innerHTML = '<div id="app"></div>';
    localStorage.clear();
    localStorage.setItem('anivault-start-page', 'season');
    // jsdom has no matchMedia implementation; App.svelte's onMount uses it to
    // track the desktop-vs-mobile rail layout.
    window.matchMedia = window.matchMedia ?? ((query: string) => ({
      matches: true,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    })) as unknown as typeof window.matchMedia;
    vi.mocked(getSeasonAnime).mockResolvedValue(seasonEntries as never);
    vi.mocked(diffSeason).mockResolvedValue({ first_visit: false, new_ids: [7] });
    vi.mocked(getLibraryIds).mockResolvedValue([]);
    // DetailView renders its "Back" button unconditionally, so a failed
    // detail fetch is enough to exercise the round trip without needing to
    // fake a full AnimeDetail payload.
    vi.mocked(fetchAnimeDetail).mockRejectedValue(new Error('not needed for this test'));
    // NowPlaying (always mounted in the sidebar) assigns this straight onto
    // its `status` state; the generic undefined-returning mock would clobber
    // its `{ active: false, watching: null }` default and crash on render.
    vi.mocked(getTrackingStatus).mockResolvedValue({ active: false, watching: null });
  });

  it('keeps the new-releases band after opening and closing a detail view, without re-diffing', async () => {
    const app = mount(App, { target: document.getElementById('app')! });
    await settle();

    expect(getSeasonAnime).toHaveBeenCalledTimes(1);
    expect(diffSeason).toHaveBeenCalledTimes(1);
    expect(document.querySelector('.new-band')).toBeTruthy();
    expect(document.querySelector('.group-count')?.textContent).toBe('1');

    // SeasonView is visible: its wrapper must not add a box of its own.
    const slot = document.querySelector('.season-view-slot') as HTMLElement;
    expect(slot).toBeTruthy();
    expect(slot.style.display).toBe('contents');

    // Open the new show's detail view via the band card.
    const card = document.querySelector('.new-band .poster-card') as HTMLElement;
    card.click();
    await settle();

    expect(document.querySelector('[aria-label="Anime detail"]')).toBeTruthy();
    // SeasonView must still be in the DOM (not unmounted) but hidden.
    expect(document.querySelector('.season-view')).toBeTruthy();
    expect(slot.style.display).toBe('none');

    // Navigate back.
    const back = document.querySelector('[aria-label="Back"]') as HTMLElement;
    back.click();
    await settle();

    // The band is exactly as it was, and load()/diff_season did not re-run.
    expect(slot.style.display).toBe('contents');
    expect(document.querySelector('.new-band')).toBeTruthy();
    expect(document.querySelector('.group-count')?.textContent).toBe('1');
    expect(getSeasonAnime).toHaveBeenCalledTimes(1);
    expect(diffSeason).toHaveBeenCalledTimes(1);

    unmount(app);
  });
});

// Covers the regression where commit 20e6e886's always-mounted Seasons
// wrapper sat unconditionally in `.content`, outside the {#if} chain. Since
// SeasonView's onMount unconditionally calls load() -> diff_season(...,
// record: true), that meant every app launch silently recorded a Seasons
// baseline regardless of the configured start page -- degrading the "new
// since you last viewed Seasons" band into "new since your last app launch".
// The fix latches a one-way `seasonEverOpened` flag the first time
// currentView becomes 'season', and only renders the always-mounted wrapper
// once that flag is set.
describe('App does not mount the Seasons view before it has ever been opened', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    document.body.innerHTML = '<div id="app"></div>';
    localStorage.clear();
    localStorage.setItem('anivault-start-page', 'library');
    window.matchMedia = window.matchMedia ?? ((query: string) => ({
      matches: true,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    })) as unknown as typeof window.matchMedia;
    vi.mocked(getSeasonAnime).mockResolvedValue(seasonEntries as never);
    vi.mocked(diffSeason).mockResolvedValue({ first_visit: false, new_ids: [7] });
    vi.mocked(getLibraryIds).mockResolvedValue([]);
    vi.mocked(fetchAnimeDetail).mockRejectedValue(new Error('not needed for this test'));
    vi.mocked(getTrackingStatus).mockResolvedValue({ active: false, watching: null });
    // LibraryView (the start page here) spreads its search results directly
    // into a reactive `sortedEntries` outside any try/catch, so — unlike the
    // rest of this file's best-effort-await components — it needs a real
    // array back rather than the generic undefined-returning automock.
    vi.mocked(searchLibrary).mockResolvedValue([]);
    // Same reason: LibraryView's next-episode column feeds the calendar
    // straight into nextAiringByAnime's `for...of` outside any try/catch
    // (the try/catch only guards the await, not a bad-shaped resolve), so it
    // needs a real array back rather than the generic undefined-returning
    // automock.
    vi.mocked(getCalendar).mockResolvedValue([]);
  });

  it('skips getSeasonAnime/diffSeason on a library start page, then loads Season normally on first visit and still preserves the round trip', async () => {
    const app = mount(App, { target: document.getElementById('app')! });
    await settle();

    // The Seasons wrapper must not even be in the DOM yet -- SeasonView never
    // mounted, so its onMount never ran.
    expect(document.querySelector('.season-view-slot')).toBeNull();
    expect(getSeasonAnime).not.toHaveBeenCalled();
    expect(diffSeason).not.toHaveBeenCalled();

    // Navigate to Season for the first time via the nav rail.
    const seasonNav = document.querySelector('[aria-label="Season"]') as HTMLElement;
    seasonNav.click();
    await settle();

    expect(getSeasonAnime).toHaveBeenCalledTimes(1);
    expect(diffSeason).toHaveBeenCalledTimes(1);
    const slot = document.querySelector('.season-view-slot') as HTMLElement;
    expect(slot).toBeTruthy();
    expect(slot.style.display).toBe('contents');
    expect(document.querySelector('.new-band')).toBeTruthy();
    expect(document.querySelector('.group-count')?.textContent).toBe('1');

    // The latch must not have broken commit 20e6e886's detail round trip:
    // opening a card's detail view keeps SeasonView mounted (hidden, not
    // unmounted), and returning does not re-run load()/diff_season.
    const card = document.querySelector('.new-band .poster-card') as HTMLElement;
    card.click();
    await settle();

    expect(document.querySelector('[aria-label="Anime detail"]')).toBeTruthy();
    expect(document.querySelector('.season-view')).toBeTruthy();
    expect(slot.style.display).toBe('none');

    const back = document.querySelector('[aria-label="Back"]') as HTMLElement;
    back.click();
    await settle();

    expect(slot.style.display).toBe('contents');
    expect(document.querySelector('.new-band')).toBeTruthy();
    expect(document.querySelector('.group-count')?.textContent).toBe('1');
    expect(getSeasonAnime).toHaveBeenCalledTimes(1);
    expect(diffSeason).toHaveBeenCalledTimes(1);

    unmount(app);
  });
});
