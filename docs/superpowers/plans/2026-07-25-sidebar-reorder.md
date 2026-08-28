# Reorderable Sidebar Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the user drag the left sidebar's nav items into any order, persisted across restarts.

**Architecture:** A new pure module `next/src/lib/navOrder.ts` owns the default order, `localStorage` persistence, and the reordering arithmetic — all unit-tested. `next/src/App.svelte` drops its hardcoded `navItems` array and renders from that stored order, adding native HTML5 drag-and-drop, Alt+Arrow keyboard reordering, and a right-click reset.

**Tech Stack:** Svelte 5 (used in legacy/Svelte-4 syntax: `let`, `$:`, `on:click`), TypeScript, Vitest + jsdom, native HTML5 drag-and-drop (no DnD dependency), Tauri 2 / WebView2.

**Spec:** `docs/superpowers/specs/2026-07-25-sidebar-reorder-design.md`

## Global Constraints

- Write Svelte in the existing legacy syntax used throughout this file: `let` for state, `$:` for derived, `on:event={...}` for handlers. Do **not** introduce runes (`$state`, `$derived`, `$props`).
- Every `localStorage` access is wrapped in `try`/`catch` and degrades silently, matching `next/src/lib/startPage.ts`.
- No new npm dependencies. Drag-and-drop uses the native HTML5 API, as `next/src/lib/LibraryView.svelte` already does.
- `dataTransfer.setData(...)` must be called in `dragstart` — without it the `drop` event does not fire in WebView2 (documented at `next/src/lib/LibraryView.svelte:265`).
- All nine nav items stay visible at all times. This feature reorders only; it never hides.
- Reordering (drag **and** keyboard) is available only when the sidebar is expanded, never when `collapsed` is true.
- Dragging is additionally restricted to the vertical desktop rail (`isDesktopRail`). Below 769px this file's media query lays the nav out horizontally, where the vertical `clientY` midpoint math is meaningless. Keyboard reordering is not affected and stays available in that layout.
- Shared CSS classes (`.sr-only`, the `.ctx-*` menu classes) go in the global `next/src/styles/tokens.css`, never copied into `App.svelte`'s scoped `<style>` block. Component-specific styles (the drag feedback) stay in `App.svelte`.
- Do not modify `LibraryView.svelte` or `CollectionView.svelte`. Their existing local copies of those classes stay as they are; consolidating them is out of scope.
- Verification command is `npm run verify` run from `next/` — it runs `tsc --noEmit`, `svelte-check`, `vitest run`, and `cargo check --tests`.
- Do **not** bump the version, build an installer, or create a release. `CLAUDE.md` requires the user to ask for a build first.

## File Structure

- **Create** `next/src/lib/navOrder.ts` — default nav items, persistence, `reconcile`, `moveNavItem`. No Svelte or Tauri imports.
- **Create** `next/src/lib/navOrder.test.ts` — unit tests for the above.
- **Modify** `next/src/App.svelte` — replace the hardcoded `navItems` const (line 36), add drag/keyboard/reset handlers to the script block, extend the `<nav class="nav-list">` markup (line 234), add component-scoped CSS for the drag feedback.
- **Modify** `next/src/styles/tokens.css` — add the shared `.sr-only` and context-menu classes. This is the app's global stylesheet, imported once from `next/src/main.ts`.

**Decision (made before execution, overrides any "copy the CSS" reading):** `.sr-only` and the context-menu classes are *not* duplicated into `App.svelte`. Svelte scopes component styles, so shared classes go in `tokens.css` and components just use them. `LibraryView.svelte` and `CollectionView.svelte` keep their existing local copies — deleting those is a separate cleanup and out of scope for this feature.

No component test for `App.svelte`: mounting it requires mocking `./api` plus ten child views, and no `App.svelte` test exists today. The reordering logic lives in `navOrder.ts` precisely so it is covered without that.

---

### Task 1: `navOrder` module

The pure module. Everything testable about this feature lives here.

**Files:**
- Create: `next/src/lib/navOrder.ts`
- Test: `next/src/lib/navOrder.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `type NavId = 'dashboard' | 'library' | 'collection' | 'season' | 'search' | 'calendar' | 'history' | 'stats' | 'settings'`
  - `const DEFAULT_NAV_ITEMS: { id: NavId; label: string }[]`
  - `function reconcile(stored: unknown): NavId[]`
  - `function loadNavOrder(): NavId[]`
  - `function saveNavOrder(order: NavId[]): void`
  - `function clearNavOrder(): void`
  - `function moveNavItem(order: NavId[], from: number, to: number): NavId[]`

- [ ] **Step 1: Write the failing test**

Create `next/src/lib/navOrder.test.ts`. This mirrors `next/src/lib/startPage.test.ts` — read that file first for the house style.

```ts
// @vitest-environment jsdom
import { describe, expect, it, beforeEach } from 'vitest';
import {
  DEFAULT_NAV_ITEMS,
  clearNavOrder,
  loadNavOrder,
  moveNavItem,
  reconcile,
  saveNavOrder,
  type NavId,
} from './navOrder';

const DEFAULT_ORDER = DEFAULT_NAV_ITEMS.map((i) => i.id);

describe('sidebar nav order', () => {
  beforeEach(() => localStorage.clear());

  it('defaults to the shipped order when nothing is stored', () => {
    expect(loadNavOrder()).toEqual(DEFAULT_ORDER);
    expect(DEFAULT_ORDER[0]).toBe('dashboard');
    expect(DEFAULT_ORDER).toHaveLength(9);
  });

  it('round-trips a saved order', () => {
    const custom: NavId[] = ['settings', 'library', 'dashboard', 'collection',
      'season', 'search', 'calendar', 'history', 'stats'];
    saveNavOrder(custom);
    expect(loadNavOrder()).toEqual(custom);
  });

  it('drops ids that are not real nav items', () => {
    expect(reconcile(['library', 'detail', 'garbage', 'dashboard']).slice(0, 2))
      .toEqual(['library', 'dashboard']);
    expect(reconcile(['library', 'detail'])).not.toContain('detail');
  });

  it('appends nav items missing from the stored order, in default order', () => {
    // Simulates a stored order written before new items existed.
    const result = reconcile(['stats', 'library']);
    expect(result.slice(0, 2)).toEqual(['stats', 'library']);
    expect(result).toHaveLength(DEFAULT_ORDER.length);
    for (const id of DEFAULT_ORDER) expect(result).toContain(id);
    // The appended tail keeps default relative order.
    expect(result.slice(2)).toEqual(
      DEFAULT_ORDER.filter((id) => id !== 'stats' && id !== 'library'),
    );
  });

  it('collapses duplicate ids, keeping the first occurrence', () => {
    const result = reconcile(['library', 'library', 'dashboard']);
    expect(result.filter((id) => id === 'library')).toHaveLength(1);
    expect(result.slice(0, 2)).toEqual(['library', 'dashboard']);
  });

  it('falls back to defaults for corrupt or non-array values', () => {
    localStorage.setItem('anivault-nav-order', 'not json{');
    expect(loadNavOrder()).toEqual(DEFAULT_ORDER);
    expect(reconcile('library')).toEqual(DEFAULT_ORDER);
    expect(reconcile(null)).toEqual(DEFAULT_ORDER);
    expect(reconcile({ id: 'library' })).toEqual(DEFAULT_ORDER);
  });

  it('clears back to the default order', () => {
    saveNavOrder(['settings', 'dashboard']);
    clearNavOrder();
    expect(loadNavOrder()).toEqual(DEFAULT_ORDER);
  });

  it('moves an item down to the given landing index', () => {
    const order: NavId[] = ['dashboard', 'library', 'collection', 'season'];
    expect(moveNavItem(order, 0, 2))
      .toEqual(['library', 'collection', 'dashboard', 'season']);
  });

  it('moves an item up to the given landing index', () => {
    const order: NavId[] = ['dashboard', 'library', 'collection', 'season'];
    expect(moveNavItem(order, 3, 1))
      .toEqual(['dashboard', 'season', 'library', 'collection']);
  });

  it('clamps out-of-range targets and ignores an invalid source', () => {
    const order: NavId[] = ['dashboard', 'library', 'collection'];
    expect(moveNavItem(order, 0, 99))
      .toEqual(['library', 'collection', 'dashboard']);
    expect(moveNavItem(order, 2, -5))
      .toEqual(['collection', 'dashboard', 'library']);
    expect(moveNavItem(order, 7, 0)).toEqual(order);
  });

  it('is a no-op when from equals to, and never mutates its input', () => {
    const order: NavId[] = ['dashboard', 'library', 'collection'];
    expect(moveNavItem(order, 1, 1)).toEqual(order);
    moveNavItem(order, 0, 2);
    expect(order).toEqual(['dashboard', 'library', 'collection']);
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd next && npx vitest run src/lib/navOrder.test.ts
```

Expected: FAIL — `Failed to resolve import "./navOrder"`.

- [ ] **Step 3: Write the implementation**

Create `next/src/lib/navOrder.ts`:

```ts
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
  const [item] = next.splice(from, 1);
  next.splice(target, 0, item);
  return next;
}
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
cd next && npx vitest run src/lib/navOrder.test.ts
```

Expected: PASS, 11 tests.

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/navOrder.ts next/src/lib/navOrder.test.ts
git commit -m "feat: add nav order preference module"
```

---

### Task 2: Render the sidebar from the stored order

Wire `App.svelte` to `navOrder.ts` with no interaction yet. After this task the sidebar looks and behaves exactly as before, but its order comes from `localStorage`.

**Files:**
- Modify: `next/src/App.svelte:36-46` (the `navItems` const), `next/src/App.svelte:234-249` (the nav markup)

**Interfaces:**
- Consumes: `DEFAULT_NAV_ITEMS`, `loadNavOrder`, `NavId` from Task 1.
- Produces: `navOrder: NavId[]` and the derived `navItems` binding, both used by Tasks 3-5.

- [ ] **Step 1: Add the import**

In `next/src/App.svelte`, below the existing `import { loadStartPage } from './lib/startPage';` (line 17), add:

```ts
  import { DEFAULT_NAV_ITEMS, loadNavOrder, type NavId } from './lib/navOrder';
```

- [ ] **Step 2: Replace the hardcoded array**

Delete the whole `const navItems = [...]` block at lines 36-46 and put this in its place. Leave the `navIcons` map (lines 48-58) exactly as it is — icons are component imports and belong in the component.

```ts
  let navOrder: NavId[] = loadNavOrder();
  $: navItems = navOrder.map((id) => DEFAULT_NAV_ITEMS.find((item) => item.id === id)!);
```

The non-null assertion is safe: `reconcile` guarantees every id in `navOrder` is a known `NavId`, so the lookup always resolves.

- [ ] **Step 3: Key the each block**

In the markup at line 235, change `{#each navItems as item}` to:

```svelte
      {#each navItems as item, i (item.id)}
```

The key keeps each button bound to its item across reorders, and `i` is needed by Tasks 3 and 4. Nothing else in the `{#each}` body changes in this task.

- [ ] **Step 4: Verify**

```bash
cd next && npm run verify
```

Expected: PASS. `i` being unused for now is not an error.

Then check by hand — build is not needed, `npm run dev` is enough:

```bash
cd next && npm run dev
```

Confirm the sidebar still lists Dashboard, Library, Collection, Season, Search, Calendar, History, Stats, Settings in that order and every item still navigates. Stop the dev server when done.

- [ ] **Step 5: Commit**

```bash
git add next/src/App.svelte
git commit -m "refactor: render sidebar nav from stored order"
```

---

### Task 3: Drag to reorder

**Files:**
- Modify: `next/src/App.svelte` (script block, nav markup, `<style>` block near `.nav-item` at line 413)

**Interfaces:**
- Consumes: `navOrder`, `navItems` from Task 2; `moveNavItem`, `saveNavOrder` from Task 1.
- Produces: `dragIndex: number | null`, `dropIndex: number | null`, `justDragged: boolean`, `commitNavDrop()`, `handleNavClick(id: NavId)` — Task 4 reuses none of these, but must not remove them.

- [ ] **Step 1: Extend the import**

Change the Task 2 import line to:

```ts
  import { DEFAULT_NAV_ITEMS, loadNavOrder, moveNavItem, saveNavOrder, type NavId } from './lib/navOrder';
```

- [ ] **Step 2: Add drag state and handlers**

In the script block, immediately after the `$: navItems = ...` line from Task 2, add:

```ts
  let dragIndex: number | null = null;
  let dropIndex: number | null = null;
  // A drag can be followed by a click event; this stops that click from also
  // navigating. It is cleared on the next pointerdown, so a genuine click is
  // never swallowed.
  let justDragged = false;

  function handleNavDragStart(e: DragEvent, index: number) {
    // Below 769px the rail lays the nav out horizontally (see the media query
    // near the end of the style block). The drop math below is vertical-only,
    // so dragging is restricted to the vertical desktop rail.
    if (collapsed || !isDesktopRail) return;
    dragIndex = index;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = 'move';
      // Required for the drop event to fire in Chromium/WebView2.
      e.dataTransfer.setData('text/plain', navOrder[index]);
    }
  }

  function handleNavDragOver(e: DragEvent, index: number) {
    if (dragIndex === null) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    dropIndex = e.clientY < rect.top + rect.height / 2 ? index : index + 1;
  }

  // Only a real drop reorders. `dragend` must NOT commit: it fires for every
  // drag, including one released outside the sidebar, where dropIndex still
  // holds the last position the pointer crossed. Committing there would
  // reorder on a cancelled drag.
  function commitNavDrop() {
    if (dragIndex !== null && dropIndex !== null) {
      // dropIndex is an insertion point in the pre-move array; moveNavItem
      // wants the index the item lands on, which is one lower when the item
      // is moving down past its own slot.
      const to = dropIndex > dragIndex ? dropIndex - 1 : dropIndex;
      if (to !== dragIndex) {
        navOrder = moveNavItem(navOrder, dragIndex, to);
        saveNavOrder(navOrder);
      }
      justDragged = true;
    }
    clearNavDragState();
  }

  // Clears drag state without reordering. Runs on dragend, which fires after
  // drop on a successful drag (harmless — state is already clear) and on its
  // own when the drag is cancelled or released off-target.
  function clearNavDragState() {
    dragIndex = null;
    dropIndex = null;
  }

  function handleNavClick(id: NavId, e: MouseEvent) {
    // Only swallow a pointer-generated click. A keyboard activation (Enter or
    // Space) arrives with detail === 0 and no preceding pointerdown, so it
    // must never be eaten by a flag left over from a mouse drag.
    if (justDragged && e.detail > 0) {
      justDragged = false;
      return;
    }
    justDragged = false;
    setView(id);
  }
```

- [ ] **Step 3: Wire the markup**

Replace the `<button>` inside the `{#each}` (lines 236-247) with:

```svelte
        <button
          type="button"
          class="nav-item"
          class:active={isNavActive(item.id)}
          class:subtle-active={currentView === 'detail' && item.id === 'library'}
          class:dragging={dragIndex === i}
          class:drop-above={dragIndex !== null && dropIndex === i}
          class:drop-below={dragIndex !== null && dropIndex === navItems.length && i === navItems.length - 1}
          draggable={!collapsed && isDesktopRail}
          title={item.label}
          aria-label={item.label}
          on:pointerdown={() => (justDragged = false)}
          on:dragstart={(e) => handleNavDragStart(e, i)}
          on:dragover={(e) => handleNavDragOver(e, i)}
          on:drop|preventDefault={commitNavDrop}
          on:dragend={clearNavDragState}
          on:click={(e) => handleNavClick(item.id, e)}
        >
          <svelte:component this={navIcons[item.id]} class="nav-icon" size={18} />
          <span class="nav-label">{item.label}</span>
        </button>
```

`drop-below` only ever applies to the last item, and covers dropping past the end of the list.

- [ ] **Step 4: Add the CSS**

In the `<style>` block, directly after the `.nav-item:focus-visible` rule (ends line 462), add:

```css
  .nav-item.dragging {
    opacity: 0.4;
  }

  .nav-item.drop-above,
  .nav-item.drop-below {
    position: relative;
  }

  .nav-item.drop-above::before,
  .nav-item.drop-below::after {
    content: '';
    position: absolute;
    left: 0.5rem;
    right: 0.5rem;
    height: 2px;
    border-radius: 2px;
    background: var(--color-accent);
  }

  .nav-item.drop-above::before {
    top: -2px;
  }

  .nav-item.drop-below::after {
    bottom: -2px;
  }
```

- [ ] **Step 5: Verify**

```bash
cd next && npm run verify
```

Expected: PASS.

```bash
cd next && npm run dev
```

Check by hand, with the sidebar expanded:
1. Drag Library below Search — an accent line shows the insertion point while dragging, and Library lands there on release.
2. Drag an item past the last one — it goes to the bottom.
3. Click a nav item normally — it still navigates, and does not reorder.
4. Reload the app — the new order is still there.
5. Collapse the sidebar and try to drag — nothing moves.
6. Drag an item and release outside the sidebar — the order is unchanged and no accent line is left behind.
7. Narrow the window below 769px so the nav goes horizontal — dragging is disabled there, and clicking still navigates.

Stop the dev server when done.

- [ ] **Step 6: Commit**

```bash
git add next/src/App.svelte
git commit -m "feat: drag to reorder sidebar nav items"
```

---

### Task 4: Keyboard reordering

Native HTML5 drag is mouse-only, so without this the feature is unreachable by keyboard.

**Files:**
- Modify: `next/src/App.svelte` (script block, nav markup, `<style>` block)

**Interfaces:**
- Consumes: `navOrder`, `navItems`, `moveNavItem`, `saveNavOrder`.
- Produces: `navButtons: HTMLButtonElement[]`, `navAnnouncement: string` — Task 5 writes to `navAnnouncement`.

- [ ] **Step 1: Import `tick`**

Change line 2 of `next/src/App.svelte` to:

```ts
  import { onMount, onDestroy, tick } from 'svelte';
```

- [ ] **Step 2: Add the keyboard handler**

In the script block, after `handleNavClick` from Task 3, add:

```ts
  let navButtons: HTMLButtonElement[] = [];
  let navAnnouncement = '';

  async function handleNavKeydown(e: KeyboardEvent, index: number) {
    if (collapsed) return;
    if (!e.altKey || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return;
    const to = e.key === 'ArrowUp' ? index - 1 : index + 1;
    if (to < 0 || to >= navOrder.length) return;
    e.preventDefault();
    const label = navItems[index].label;
    navOrder = moveNavItem(navOrder, index, to);
    saveNavOrder(navOrder);
    navAnnouncement = `${label} moved to position ${to + 1} of ${navOrder.length}`;
    // Focus follows the item, not the index it used to sit at.
    await tick();
    navButtons[to]?.focus();
  }
```

- [ ] **Step 3: Wire the markup**

Add two attributes to the nav `<button>` from Task 3 — put `bind:this` directly above `draggable`, and the keydown handler directly above `on:click`:

```svelte
          bind:this={navButtons[i]}
```

```svelte
          on:keydown={(e) => handleNavKeydown(e, i)}
```

Then, immediately after the closing `</nav>` tag, add the live region:

```svelte
    <div class="sr-only" aria-live="polite">{navAnnouncement}</div>
```

- [ ] **Step 4: Add the `.sr-only` CSS globally**

Svelte scopes component styles, so a shared utility class belongs in the global stylesheet, not copied into `App.svelte`. Add this to the **end of `next/src/styles/tokens.css`** (imported once from `next/src/main.ts`, so it applies everywhere). Do **not** add it to `App.svelte`'s `<style>` block — Svelte would report it as an unused selector there anyway once the markup uses the global class.

Leave the existing local copies in `CollectionView.svelte:1002` and `LibraryView.svelte` untouched; removing them is a separate cleanup, out of scope here.

```css
/* Visually hidden, still announced by screen readers. */
.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border-width: 0;
}
```

- [ ] **Step 5: Verify**

```bash
cd next && npm run verify
```

Expected: PASS.

```bash
cd next && npm run dev
```

Check by hand:
1. Press Tab until a nav item has the focus ring, then Alt+Down — the item moves down one slot and keeps focus.
2. Alt+Up on the first item does nothing, Alt+Down on the last does nothing.
3. Plain Arrow keys and plain Enter/Space still behave normally (no reorder, Enter navigates).
4. Reload — the keyboard-made order persisted.
5. Collapse the sidebar, focus an item, Alt+Down — nothing moves.

Stop the dev server when done.

- [ ] **Step 6: Commit**

```bash
git add next/src/App.svelte next/src/styles/tokens.css
git commit -m "feat: keyboard reordering for sidebar nav"
```

---

### Task 5: Reset to default order

**Files:**
- Modify: `next/src/App.svelte` (script block, nav markup, `<style>` block)

**Interfaces:**
- Consumes: `navOrder`, `navAnnouncement`, `DEFAULT_NAV_ITEMS`, `clearNavOrder`.
- Produces: nothing used elsewhere.

- [ ] **Step 1: Extend the import**

Change the navOrder import to its final form:

```ts
  import { DEFAULT_NAV_ITEMS, clearNavOrder, loadNavOrder, moveNavItem, saveNavOrder, type NavId } from './lib/navOrder';
```

- [ ] **Step 2: Add the menu state and handlers**

In the script block, after `handleNavKeydown` from Task 4, add:

```ts
  let navCtxMenu: { x: number; y: number } | null = null;

  function openNavContextMenu(e: MouseEvent) {
    e.preventDefault();
    // Clamp so a right-click near the window edge does not push the menu
    // off-screen, matching LibraryView.svelte:451.
    navCtxMenu = {
      x: Math.min(e.clientX, window.innerWidth - 200),
      y: Math.min(e.clientY, window.innerHeight - 80),
    };
  }

  function resetNavOrder() {
    navOrder = DEFAULT_NAV_ITEMS.map((item) => item.id);
    clearNavOrder();
    navAnnouncement = 'Sidebar order reset to default';
    navCtxMenu = null;
  }
```

- [ ] **Step 3: Wire the markup**

Add the handler to the `<nav>` opening tag (line 234):

```svelte
    <nav class="nav-list" on:contextmenu={openNavContextMenu}>
```

Then add the menu — **not inside `<aside class="rail">`**. Put it as a direct child of `<main class="shell">`, immediately after the closing `</section>` of `.content`.

This placement is load-bearing, not stylistic. `.rail` sets `backdrop-filter: blur(24px)` (`App.svelte:436`), which per Filter Effects L2 makes it a containing block for `position: fixed` descendants — implemented in Chromium, which is what WebView2 runs. Mounted inside the rail, the menu's `left`/`top` would resolve against the rail's padding box instead of the viewport, `.ctx-backdrop { inset: 0 }` would cover only the 16rem rail instead of the screen (so clicking the content area would not dismiss it), and the rail's `overflow-y: auto` (`App.svelte:442`) would clip the 11rem-wide menu. `.shell` (`App.svelte:424-430`) is a plain grid with no filter, transform, or overflow, so fixed positioning there resolves against the viewport as intended. `LibraryView`'s copy works only because it sits under `.content`, which has no filter.

The markup mirrors `next/src/lib/LibraryView.svelte:895-921`. Note `openNavContextMenu` clamps the coordinates the same way LibraryView does (`LibraryView.svelte:451`), so a right-click near the window edge does not push the menu off-screen.

```svelte
    {#if navCtxMenu}
      <div
        class="ctx-backdrop"
        role="presentation"
        on:click={() => (navCtxMenu = null)}
        on:contextmenu|preventDefault={() => (navCtxMenu = null)}
      ></div>
      <div class="ctx-menu" style="left: {navCtxMenu.x}px; top: {navCtxMenu.y}px;" role="menu">
        <button class="ctx-item" role="menuitem" on:click={resetNavOrder}>Reset sidebar order</button>
      </div>
    {/if}
```

- [ ] **Step 4: Add the CSS globally**

Add this to the **end of `next/src/styles/tokens.css`**, below the `.sr-only` rule from Task 4. Do **not** put it in `App.svelte`'s `<style>` block — Svelte scopes component styles, so a shared menu style belongs in the global sheet.

The values match `next/src/lib/LibraryView.svelte`'s menu so the two look identical. Leave LibraryView's own component-scoped copy in place; deduplicating it is a separate cleanup, out of scope here.

```css
/* Shared context-menu chrome. LibraryView still carries its own scoped copy;
   consolidating that is a separate cleanup. */
.ctx-backdrop {
  position: fixed;
  inset: 0;
  z-index: 40;
}

.ctx-menu {
  position: fixed;
  z-index: 41;
  min-width: 11rem;
  background: rgba(16, 21, 32, 0.98);
  border: 1px solid rgba(var(--color-accent-rgb), 0.25);
  border-radius: 10px;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
  padding: 0.35rem;
  display: grid;
  gap: 0.15rem;
}

.ctx-menu .ctx-item {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  width: 100%;
  text-align: left;
  border: none;
  background: transparent;
  color: var(--color-text);
  font-family: var(--font-ui);
  font-size: 0.82rem;
  padding: 0.4rem 0.55rem;
  border-radius: 6px;
  cursor: pointer;
  white-space: nowrap;
}

.ctx-menu .ctx-item:hover:not(:disabled) {
  /* Same accent tint LibraryView.svelte:1401 and CollectionView.svelte:796
     use, so every context menu in the app hovers identically. */
  background: rgba(var(--color-accent-rgb), 0.15);
}
```

- [ ] **Step 5: Verify**

```bash
cd next && npm run verify
```

Expected: PASS.

```bash
cd next && npm run dev
```

Check by hand:
1. Reorder a few items, then right-click anywhere in the nav list — the menu opens at the cursor.
2. Click "Reset sidebar order" — the order returns to Dashboard, Library, Collection, Season, Search, Calendar, History, Stats, Settings.
3. Reload — the default order persisted (the stored key was removed, not overwritten).
4. Right-click and then left-click elsewhere — the menu closes without resetting.
5. **Regression check:** go to Library, right-click a row — LibraryView's own context menu still renders correctly. Its scoped `.ctx-menu` rules have higher specificity than the new global ones, so nothing should have shifted, but confirm it visually.

Stop the dev server when done.

- [ ] **Step 6: Commit**

```bash
git add next/src/App.svelte next/src/styles/tokens.css
git commit -m "feat: reset sidebar order from nav context menu"
```

---

## Done

At this point the feature is complete and `npm run verify` passes. Per `CLAUDE.md` this is a user-facing change and wants a patch version bump, an installer build, and a tagged GitHub release — **ask the user before doing any of that.**
