// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { mount, unmount, flushSync, tick } from 'svelte';
import type { CalendarEntry } from './api';

// Weekly episodes for one show across July–September 2026, in local time so
// day-cell placement is timezone-stable.
function weekly(animeId: number, title: string, firstEp: number): CalendarEntry[] {
  const out: CalendarEntry[] = [];
  const start = new Date(2026, 6, 3, 13, 0, 0); // Fri Jul 3 2026, 13:00 local
  for (let i = 0; i < 11; i++) {
    const airing = Math.floor(start.getTime() / 1000) + i * 7 * 86400;
    out.push({
      anime_id: animeId,
      title,
      image_url: null,
      episode_count: null,
      progress: null,
      next_episode: firstEp + i,
      airing_at: airing,
      time_until_airing: 0,
      has_file: false,
      // Everything up to and including Ep5 counts as watched.
      watched: firstEp + i <= 5,
    });
  }
  return out;
}

vi.mock('./api', () => ({
  getCalendar: vi.fn(async () => weekly(1, 'Alpha Show', 3)),
  getLibraryStats: vi.fn(async () => ({
    total: 1, watching: 1, completed: 0, on_hold: 0, dropped: 0, plan_to_watch: 0,
  })),
}));

import CalendarView from './CalendarView.svelte';

function renderedEpisodes(): string[] {
  return [...document.querySelectorAll('.cal-day-entry .cal-entry-ep')].map(
    (el) => el.textContent ?? '',
  );
}
function headerText(): string {
  return document.querySelector('.cal-nav h2')?.textContent ?? '';
}

async function settle() {
  // load() resolves entries, then Svelte re-renders.
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
  flushSync();
}

describe('CalendarView month paging', () => {
  beforeEach(() => {
    document.body.innerHTML = '<div id="app"></div>';
    localStorage.clear();
    sessionStorage.clear();
    localStorage.setItem('anivault-calendar-view', 'month');
    // Fix the clock to July 2026 so the calendar opens on that month
    // regardless of the real date. Timers stay real for settle().
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(new Date(2026, 6, 15, 12, 0, 0));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('shows next month episodes after clicking the next-month arrow', async () => {
    const app = mount(CalendarView, { target: document.getElementById('app')! });
    await settle();

    expect(headerText()).toBe('July 2026');
    // July: Fri Jul 3, 10, 17, 24, 31 → Ep3..Ep7
    expect(renderedEpisodes()).toEqual(['Ep3', 'Ep4', 'Ep5', 'Ep6', 'Ep7']);

    const next = document.querySelector<HTMLButtonElement>('button[aria-label="Next month"]')!;
    next.click();
    flushSync();
    await tick();

    expect(headerText()).toBe('August 2026');
    // August: Fri Aug 7, 14, 21, 28 → Ep8..Ep11
    expect(renderedEpisodes()).toEqual(['Ep8', 'Ep9', 'Ep10', 'Ep11']);

    await unmount(app);
  });

  it('jumps back to the current month via the Today button', async () => {
    const app = mount(CalendarView, { target: document.getElementById('app')! });
    await settle();

    const next = document.querySelector<HTMLButtonElement>('button[aria-label="Next month"]')!;
    next.click();
    flushSync();
    next.click();
    flushSync();
    await tick();
    expect(headerText()).toBe('September 2026');

    const today = document.querySelector<HTMLButtonElement>('button[aria-label="Go to current month"]')!;
    expect(today).not.toBeNull();
    today.click();
    flushSync();
    await tick();

    const now = new Date();
    const monthNames = ['January','February','March','April','May','June','July','August','September','October','November','December'];
    expect(headerText()).toBe(`${monthNames[now.getMonth()]} ${now.getFullYear()}`);

    await unmount(app);
  });

  it('marks watched episodes with a check and dims the row', async () => {
    const app = mount(CalendarView, { target: document.getElementById('app')! });
    await settle();

    expect(headerText()).toBe('July 2026');
    // July renders Ep3..Ep7; the fixture marks Ep3, Ep4 and Ep5 as watched.
    expect(document.querySelectorAll('.cal-day-entry.watched')).toHaveLength(3);
    expect(document.querySelectorAll('.cal-day-entry .ep-check')).toHaveLength(3);

    // Ep3 aired 2026-07-03 with no local file: not downloaded, but watched.
    const first = document.querySelector<HTMLElement>('.cal-day-entry')!;
    expect(first.getAttribute('aria-label')).toBe('Alpha Show Ep 3 (Not downloaded, watched)');

    // Focusing the entry opens the hover tooltip; a watched entry adds a
    // "Watched" line to it.
    first.dispatchEvent(new FocusEvent('focus'));
    flushSync();
    expect(document.querySelector('.tip-watched')?.textContent).toBe('✓ Watched');

    await unmount(app);
  });
});

describe('CalendarView startup month', () => {
  beforeEach(() => {
    document.body.innerHTML = '<div id="app"></div>';
    localStorage.clear();
    sessionStorage.clear();
    localStorage.setItem('anivault-calendar-view', 'month');
    vi.useFakeTimers({ toFake: ['Date'] });
    // Mon Aug 3 2026: a month on from the July session below.
    vi.setSystemTime(new Date(2026, 7, 3, 9, 0, 0));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function storeViewDate(viewed: Date, savedAt: Date) {
    sessionStorage.setItem(
      'anivault-calendar-date',
      JSON.stringify({ viewed: viewed.toISOString(), savedAt: savedAt.toISOString() }),
    );
  }

  it('opens on the current month, ignoring a month left over from an earlier month', async () => {
    storeViewDate(new Date(2026, 6, 15), new Date(2026, 6, 15));

    const app = mount(CalendarView, { target: document.getElementById('app')! });
    await settle();

    expect(headerText()).toBe('August 2026');
    await unmount(app);
  });

  it('keeps the month browsed earlier in the same session', async () => {
    storeViewDate(new Date(2026, 9, 1), new Date(2026, 7, 3, 8, 0, 0));

    const app = mount(CalendarView, { target: document.getElementById('app')! });
    await settle();

    expect(headerText()).toBe('October 2026');
    await unmount(app);
  });
});

describe('CalendarView agenda view', () => {
  beforeEach(() => {
    document.body.innerHTML = '<div id="app"></div>';
    localStorage.clear();
    localStorage.setItem('anivault-calendar-view', 'agenda');
    // Fake only Date so the agenda's from-today-onward filter is deterministic;
    // timers stay real (settle() below relies on a genuine setTimeout tick).
    vi.useFakeTimers({ toFake: ['Date'] });
    // Fri Jul 17 2026, 18:00 local: after Ep5 airs (13:00 same day), so Ep5 is
    // both watched and already aired — the case that must not double-dim.
    vi.setSystemTime(new Date(2026, 6, 17, 18, 0, 0));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('marks a watched agenda row and keeps its aired badge at full opacity', async () => {
    const app = mount(CalendarView, { target: document.getElementById('app')! });
    await settle();

    expect(headerText()).toBe('Agenda');

    // Ep3 and Ep4 already scrolled off (aired before today); Ep5 airs today and
    // is the only watched entry still in the from-today-onward agenda.
    const watchedRows = document.querySelectorAll('.agenda-row.watched');
    expect(watchedRows).toHaveLength(1);

    const row = watchedRows[0]!;
    const check = row.querySelector('.agenda-check')!;
    expect(check.getAttribute('aria-hidden')).toBe('true');
    // The checkmark is aria-hidden, so the accessible name must come from
    // elsewhere: a visually-hidden sibling carries the word for screen readers.
    expect(row.querySelector('.sr-only')?.textContent).toBe('Watched');

    // Ep5 aired earlier today, so its countdown badge also carries .aired —
    // this is the compounding case (.agenda-row.watched + .agenda-countdown.aired)
    // the anti-double-dim rule (`.agenda-row.watched .agenda-countdown.aired`)
    // exists to correct. jsdom doesn't compute the cascade for scoped Svelte
    // styles, so this only pins the DOM shape the CSS rule targets, not the
    // rendered opacity.
    const badge = row.querySelector('.agenda-countdown.aired');
    expect(badge).not.toBeNull();

    await unmount(app);
  });
});
