// Sidebar navigation order. A plain UI preference, so it lives in
// localStorage like the other view state (rail collapse, start page).

export type NavId =
  | 'dashboard' | 'library' | 'collection' | 'season'
  | 'search' | 'calendar' | 'history' | 'stats' | 'settings';

export const DEFAULT_NAV_ITEMS: { id: NavId; label: string }[] = [
  { id: 'dashboard', label: 'Dashboard' },
  { id: 'library', label: 'Library' },
  { id: 'collection', label: 'Collection' },
  { id: 'season', label: 'Season' },
  { id: 'search', label: 'Search' },
  { id: 'calendar', label: 'Calendar' },
  { id: 'history', label: 'History' },
  { id: 'stats', label: 'Stats' },
  { id: 'settings', label: 'Settings' },
];

const KEY = 'anivault-nav-order';

const DEFAULT_ORDER: NavId[] = DEFAULT_NAV_ITEMS.map((item) => item.id);

function isNavId(value: unknown): value is NavId {
  return typeof value === 'string' && DEFAULT_ORDER.includes(value as NavId);
}

// Turn whatever is in storage into a valid, complete order: drop unknown ids
// and duplicates, then append anything the stored order is missing. That
// append is what keeps a nav item added in a future version reachable for
// users who already have a saved order.
//
// Nav ids are a persistence format, not just an internal identifier: they are
// written to localStorage verbatim and read back through isNavId. Renaming an
// existing id here would make every stored order fail isNavId for that entry,
// silently discarding the user's chosen position for it (it would just be
// re-appended at the end as if new). A rename must instead go through an
// alias map (old id -> new id) applied to `stored` before the isNavId filter
// below, so existing positions carry over.
export function reconcile(stored: unknown): NavId[] {
  if (!Array.isArray(stored)) return [...DEFAULT_ORDER];
  const seen = new Set<NavId>();
  const order: NavId[] = [];
  for (const entry of stored) {
    if (!isNavId(entry) || seen.has(entry)) continue;
    seen.add(entry);
    order.push(entry);
  }
  for (const id of DEFAULT_ORDER) {
    if (!seen.has(id)) order.push(id);
  }
  return order;
}

export function loadNavOrder(): NavId[] {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [...DEFAULT_ORDER];
    return reconcile(JSON.parse(raw));
  } catch {
    return [...DEFAULT_ORDER];
  }
}

export function saveNavOrder(order: NavId[]) {
  try { localStorage.setItem(KEY, JSON.stringify(order)); } catch {}
}

export function clearNavOrder() {
  try { localStorage.removeItem(KEY); } catch {}
}

// `to` is the index the item should end up at in the returned array, not an
// insertion point in the input. A caller holding an insertion point must
// subtract one when the item is moving downward.
export function moveNavItem(order: NavId[], from: number, to: number): NavId[] {
  const next = [...order];
  if (from < 0 || from >= next.length) return next;
  const target = Math.max(0, Math.min(to, next.length - 1));
  const [item] = next.splice(from, 1) as [NavId];
  next.splice(target, 0, item);
  return next;
}
