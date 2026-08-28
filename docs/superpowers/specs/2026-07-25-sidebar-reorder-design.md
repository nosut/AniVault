# Reorderable Sidebar Navigation

Date: 2026-07-25

## Goal

Let the user drag the left sidebar's navigation items into whatever order they
like, and have that order persist across restarts.

## Scope

In scope:

- Reordering the nine existing nav items by dragging them within the sidebar.
- Keyboard reordering (Alt+Arrow) so the feature is reachable without a mouse.
- Resetting to the default order.
- Persisting the order locally.

Out of scope:

- Hiding or showing nav items. All nine stay visible; only their order changes.
- Any Settings-page UI. The whole interaction happens in the sidebar.
- Reordering while the rail is collapsed (see "Collapsed rail" below).

## Current state

`next/src/App.svelte` hardcodes the navigation as a `navItems` array (line 36)
paired with a `navIcons` lookup (line 48), rendered by an `{#each}` inside
`<nav class="nav-list">` (line 234). Order is fixed at compile time.

Two existing patterns this design follows:

- `next/src/lib/startPage.ts` — a small module holding a UI preference with
  `load`/`save` helpers over `localStorage`, unit-tested in `startPage.test.ts`.
  Every other view preference (rail collapse, calendar month, library filters)
  uses `localStorage` the same way.
- `next/src/lib/LibraryView.svelte` — native HTML5 drag-and-drop
  (`handleDragStart`, line 261; the status-tab `on:drop`, line 594). The project
  has no drag-and-drop dependency and does not need one.

## Design

### 1. `next/src/lib/navOrder.ts`

A new module owning the default order, persistence, and the reordering
arithmetic. It has no Tauri or Svelte dependencies, so it is directly testable.

```ts
export type NavId =
  | 'dashboard' | 'library' | 'collection' | 'season'
  | 'search' | 'calendar' | 'history' | 'stats' | 'settings';

export const DEFAULT_NAV_ITEMS: { id: NavId; label: string }[];

export function loadNavOrder(): NavId[];
export function saveNavOrder(order: NavId[]): void;
export function clearNavOrder(): void;
export function moveNavItem(order: NavId[], from: number, to: number): NavId[];
export function reconcile(stored: unknown): NavId[];
```

`DEFAULT_NAV_ITEMS` holds the nine items in the order `App.svelte` uses today:
Dashboard, Library, Collection, Season, Search, Calendar, History, Stats,
Settings. Icons stay in `App.svelte` — they are component imports and belong
with the component.

Storage key: `anivault-nav-order`, a JSON array of ids. All `localStorage`
access is wrapped in `try`/`catch` like the neighbouring modules.

`reconcile(stored)` turns whatever is in storage into a valid, complete order:

1. Not an array, or JSON that fails to parse: return the default order.
2. Drop entries that are not known `NavId`s.
3. Drop duplicates, keeping the first occurrence.
4. Append any default id missing from the result, preserving default relative
   order.

Rule 4 is the important one. When a future version adds a nav item, users with a
saved order get the new item appended at the bottom rather than losing access to
it. Rules 2 and 3 mean a hand-edited or corrupted value degrades instead of
breaking the sidebar.

`loadNavOrder()` reads the key and returns `reconcile(...)` of it.
`clearNavOrder()` removes the key.

`moveNavItem(order, from, to)` is pure: it returns a new array with the item at
`from` removed and re-inserted so it lands at index `to` in the returned array.
`to` is a landing index, not an insertion point — callers holding an insertion
point must convert first (see section 3). Out-of-range indices are clamped;
`from === to` returns an equivalent order. The input array is not mutated.

### 2. `App.svelte` wiring

Remove the hardcoded `navItems` array. Keep `navIcons`. Add:

```ts
let navOrder: NavId[] = loadNavOrder();
$: navItems = navOrder.map((id) => DEFAULT_NAV_ITEMS.find((i) => i.id === id)!);
```

Because `reconcile` guarantees every id in `navOrder` is a known id, the lookup
always resolves.

### 3. Drag interaction

Native HTML5 drag-and-drop, matching `LibraryView`:

- Each `.nav-item` gets `draggable={!collapsed}`.
- `on:dragstart` records the dragged index, sets
  `dataTransfer.effectAllowed = 'move'` and calls `dataTransfer.setData(...)`.
  The `setData` call is required for the `drop` event to fire in WebView2 —
  `LibraryView.svelte:265` documents the same constraint.
- `on:dragover` per item calls `preventDefault()`, sets
  `dataTransfer.dropEffect = 'move'`, and computes the insertion point:
  `dropIndex = pointerY < itemMidpoint ? i : i + 1`.
- `on:drop` and `on:dragend` commit through `moveNavItem`, call
  `saveNavOrder`, and clear the drag state. `dropIndex` is an insertion point in
  the pre-move array, so the landing index passed to `moveNavItem` is
  `dropIndex > from ? dropIndex - 1 : dropIndex`.

Because the browser only begins a native drag after its own movement threshold,
an ordinary click still navigates. A `justDragged` flag set on `dragstart` and
checked at the top of the nav-item click handler guards the edge case where a
click event is delivered after a drop.

Visual feedback:

- The item being dragged renders at `opacity: .4`.
- The insertion point renders as a 2px accent-coloured rule, via
  `.nav-item.drop-above::before` and `.nav-item.drop-below::after`. The
  `drop-below` variant covers dropping past the last item.

### 4. Keyboard reordering

With a nav item focused, `Alt+ArrowUp` / `Alt+ArrowDown` moves it one slot,
clamped at the ends. The handler calls `preventDefault()`, applies
`moveNavItem`, saves, then restores focus to the moved button after `tick()`
using a `bind:this` array of button elements — otherwise focus would follow the
index rather than the item.

A visually hidden `aria-live="polite"` region announces the result, e.g.
"Library moved to position 3 of 9".

### 5. Reset

Right-clicking the nav list opens a single-item popup, "Reset sidebar order",
which sets `navOrder` back to the defaults and calls `clearNavOrder()`. It uses
the same menu-plus-backdrop structure as `LibraryView`'s context menu
(`.ctx-menu` / `.ctx-backdrop`); `App.svelte` gets its own minimal copy rather
than sharing, since the two menus have nothing else in common.

### 6. Collapsed rail

Reordering — both drag and keyboard — is available only when the sidebar is
expanded. `draggable` is bound to `!collapsed` and the keydown handler returns
early when collapsed. This keeps the rule to a single sentence and avoids
drop-target arithmetic on a rail with no labels.

## Error handling

- `localStorage` throwing (read or write) is caught and ignored, as elsewhere in
  the codebase. A failed read yields the default order; a failed write means the
  order applies for the session but does not persist.
- Malformed stored data is handled by `reconcile` rather than by throwing.
- A drag that ends outside the nav list fires `dragend` without `drop`; the
  state is cleared and the order is unchanged.

## Testing

`next/src/lib/navOrder.test.ts`, mirroring `startPage.test.ts`
(`// @vitest-environment jsdom`, `localStorage.clear()` in `beforeEach`):

- Defaults when nothing is stored.
- Save/load round-trip.
- Unknown ids are dropped.
- Ids missing from the stored order are appended at the end, in default order.
- Duplicate ids collapse to one.
- Corrupt JSON falls back to defaults.
- `moveNavItem`: move down, move up, clamped indices, `from === to` no-op, and
  that it does not mutate its input.

No component test for `App.svelte`. Mounting it requires mocking `./api` plus
ten child views, and no `App.svelte` test exists today. Keeping the ordering
logic in a pure module is what makes that acceptable — the behaviour under test
is all in `navOrder.ts`.

Verification: `npm run verify` in `next/` (typecheck, vitest, `cargo check
--tests`).

## Release

This is a user-facing change, so per `CLAUDE.md` it ships with a patch version
bump, an installer build, and a tagged GitHub release — only once the user asks
for the build.
