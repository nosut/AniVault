# Collapsible Left Panel

## Problem

The left navigation rail (`.rail` in `App.svelte`) is a fixed 16rem-wide sidebar with a brand banner, text nav items, and a `NowPlaying` status card. Users have no way to reclaim that horizontal space for content-heavy views (Library, Calendar, Stats). We want an icon-only collapsed mode, toggled by the user, that persists across app restarts.

## Scope

Desktop layout only (viewport width > 768px, where the rail renders as a vertical sidebar). The existing mobile layout (horizontal top bar below 768px) is unaffected — no toggle is rendered there and it keeps its current behavior.

## Architecture

A single boolean, `collapsed`, owned by `App.svelte`, drives CSS class toggles on the existing `.rail` element. No new Svelte store, no new top-level component — the feature only touches `App.svelte` and `NowPlaying.svelte`.

## State & persistence

- `collapsed: boolean` in `App.svelte`, initialized on mount from `localStorage.getItem('anivault-rail-collapsed')` (`'true'` → `true`, anything else → `false`, default expanded).
- Read/write wrapped in try/catch, following the existing pattern in `LibraryView.svelte` (`loadPref`/`persistPref`):
  ```ts
  function loadCollapsed(): boolean {
    try { return localStorage.getItem('anivault-rail-collapsed') === 'true'; }
    catch { return false; }
  }
  function persistCollapsed(value: boolean) {
    try { localStorage.setItem('anivault-rail-collapsed', String(value)); } catch {}
  }
  ```
- `toggleCollapse()` flips `collapsed` and calls `persistCollapsed`.

## Layout changes (`App.svelte`)

- `.rail` width becomes a variable: `16rem` expanded, `4.5rem` collapsed, animated via `transition: width 0.2s ease`. The parent `.shell` grid changes from `grid-template-columns: 16rem 1fr` to `grid-template-columns: auto 1fr` so the content column follows the rail's animated width.
- A new `.rail-top` wrapper holds the brand block and the toggle button together:
  - Expanded: banner image + "AniVault" label + toggle button, laid out in a row.
  - Collapsed: banner shrinks to a small square crop (`object-fit: cover`, ~32px, via CSS only — same `<img>`, no new asset), label hidden (`display: none`), toggle centered underneath in a column layout.
- Toggle button: a `lucide-svelte` `ChevronLeft` (expanded, "collapse" affordance) / `ChevronRight` (collapsed, "expand" affordance) icon, `aria-label` set to "Collapse sidebar" / "Expand sidebar" accordingly.
- Each `.nav-item` button gets a leading icon (`lucide-svelte`) plus the existing label `<span>`. CSS (not `{#if}`) hides the label text and centers the icon when `.rail.collapsed`, so no markup swap is needed — just class-driven style changes. Each button keeps a persistent `aria-label={item.label}` and gets a `title={item.label}` for native tooltips when collapsed.
- Suggested icon mapping (may be adjusted during implementation for a closer semantic fit):

  | Nav item | Icon |
  |---|---|
  | Dashboard | `LayoutDashboard` |
  | Library | `Library` |
  | Season | `CalendarRange` |
  | Search | `Search` |
  | Calendar | `Calendar` |
  | History | `History` |
  | Stats | `BarChart3` |
  | Settings | `Settings` |

## NowPlaying collapsed behavior

- `NowPlaying.svelte` gains a new `collapsed: boolean = false` export prop, passed from `App.svelte` as `<NowPlaying events={latestEvents} {collapsed} />`.
- Polling and status logic (`poll`, `startPolling`, `handleStart`/`handleStop`/`handleConfirm`) are untouched.
- Render output branches on `collapsed`:
  - `true`: render a small status dot — green when `status.active && status.watching`, gray otherwise — with a `title` attribute containing the current status text (`lastEvent`, or "Tracking active"/"Tracking stopped").
  - `false`: existing full card, unchanged.

## Dependencies

- Add `lucide-svelte` as a new dependency (confirmed compatible with Svelte 5) for nav icons and the chevron toggle. No other new dependencies.

## Error handling

- Only new failure surface is `localStorage` access, already handled by the try/catch wrappers above (mirrors existing codebase pattern) — failures silently fall back to expanded state and non-persistence, never throwing.

## Testing

- No component-testing library (e.g. `@testing-library/svelte`) exists in this repo; existing frontend tests are plain vitest assertions and filesystem checks (`brand.test.ts`, `smoke.test.ts`). No new test infrastructure will be introduced for this UI toggle.
- Verification is manual: run the app (`npm run dev` / Tauri dev), toggle the rail, confirm nav icons/tooltips, brand crop, NowPlaying dot, restart-persistence, and that the sub-768px layout is unaffected.

## Out of scope

- Mobile/narrow-layout collapse (horizontal bar stays as-is).
- Per-item customization of collapsed icons beyond the mapping above.
- Keyboard shortcut for toggling (mouse/touch only, consistent with rest of nav).
