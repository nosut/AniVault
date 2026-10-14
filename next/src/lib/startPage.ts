// Which page the app opens to. A plain UI preference, so it lives in
// the prefs store like the other view state (rail collapse, calendar month).
import { getPref, setPref } from './prefs';

const KEY = 'anivault-start-page';

export const START_PAGE_OPTIONS = [
  { value: 'dashboard', label: 'Dashboard' },
  { value: 'library', label: 'Library' },
  { value: 'season', label: 'Season' },
  { value: 'search', label: 'Search' },
  { value: 'calendar', label: 'Calendar' },
  { value: 'history', label: 'History' },
  { value: 'stats', label: 'Stats' },
] as const;

export type StartPage = (typeof START_PAGE_OPTIONS)[number]['value'];

export function loadStartPage(): StartPage {
  try {
    const raw = getPref(KEY);
    if (raw && START_PAGE_OPTIONS.some((o) => o.value === raw)) return raw as StartPage;
  } catch {}
  return 'dashboard';
}

export function saveStartPage(page: StartPage) {
  try { setPref(KEY, page); } catch {}
}
