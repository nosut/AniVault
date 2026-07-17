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
    // Fix the viewed month to July 2026 regardless of the real clock.
    localStorage.setItem('anivault-calendar-date', new Date(2026, 6, 15).toISOString());
    localStorage.setItem('anivault-calendar-view', 'month');
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
});
