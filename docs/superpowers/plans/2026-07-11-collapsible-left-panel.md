# Collapsible Left Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the left navigation rail in the AniVault desktop app collapse into an icon-only view, with the choice persisted across restarts.

**Architecture:** A single `collapsed: boolean` in `App.svelte` drives CSS classes on the existing `.rail` element (width transition + label hiding) and is passed down to `NowPlaying.svelte` so it can swap its card for a status dot. No new store, no new top-level component.

**Tech Stack:** Svelte 5, Vite, TypeScript, `lucide-svelte` (new dependency) for icons.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-11-collapsible-left-panel-design.md`
- localStorage key is exactly `anivault-rail-collapsed`, storing the string `'true'`/`'false'`, read/written inside try/catch (matches the existing pattern in `next/src/lib/LibraryView.svelte`).
- Desktop only: all collapsed-specific styling must be neutralized inside the existing `@media (max-width: 768px)` block in `next/src/App.svelte` — the toggle button must not render there, and the horizontal mobile nav must look and behave exactly as it does today.
- Only one new dependency: `lucide-svelte` (npm registry confirms `1.0.1`, peer dep `svelte: '^3 || ^4 || ^5.0.0-next.42'`, compatible with this project's `svelte: ^5.25.0`).
- No new test infrastructure (no `@testing-library/svelte` or similar) — this repo's frontend tests are plain `vitest` assertions/filesystem checks only. Verification for this feature is `npm run check` (TypeScript) + `npm run test` (existing suite must stay green) + a manual run-through in the dev app.
- Commands below assume the working directory is `next/` (this project's frontend package root).

---

### Task 1: Add `lucide-svelte` dependency

**Files:**
- Modify: `next/package.json` (via `npm install`, not hand-edited)
- Modify: `next/package-lock.json` (via `npm install`)

**Interfaces:**
- Produces: `lucide-svelte` importable from any `.svelte` file, e.g. `import { Search } from 'lucide-svelte';`, used by Task 3.

- [ ] **Step 1: Install the package**

Run from `next/`:
```
npm install lucide-svelte@^1.0.1
```
Expected: `package.json` gains a `"lucide-svelte": "^1.0.1"` line under `dependencies`, and `package-lock.json` is updated. No errors.

- [ ] **Step 2: Verify the project still typechecks**

Run from `next/`:
```
npm run check
```
Expected: passes with no new errors (the dependency isn't used yet, so this just confirms the install didn't break anything).

- [ ] **Step 3: Commit**

```bash
git add next/package.json next/package-lock.json
git commit -m "chore: add lucide-svelte for sidebar icons"
```

---

### Task 2: `NowPlaying.svelte` — collapsed status dot

**Files:**
- Modify: `next/src/lib/NowPlaying.svelte`

**Interfaces:**
- Consumes: nothing new (all existing state: `status`, `lastEvent` from `next/src/lib/NowPlaying.svelte:7-9`).
- Produces: a new exported prop `collapsed: boolean` (default `false`) on `NowPlaying`, consumed by `App.svelte` in Task 3 as `<NowPlaying events={latestEvents} {collapsed} />`.

- [ ] **Step 1: Add the `collapsed` prop and derived dot state**

In `next/src/lib/NowPlaying.svelte`, change line 5 from:
```svelte
  export let events: EngineEvent[] = [];
```
to:
```svelte
  export let events: EngineEvent[] = [];
  export let collapsed = false;
```

Then, directly after the existing reactive block at lines 21-34 (the `$: { const last = events.at(-1); ... }` block), add two new reactive declarations:
```svelte
  $: dotActive = status.active && status.watching !== null;
  $: dotTitle = status.watching
    ? (lastEvent ?? `Tracking ${status.watching.player_name}`)
    : status.active
      ? 'Waiting for playback…'
      : 'Tracking stopped';
```

- [ ] **Step 2: Branch the markup on `collapsed`**

Wrap the existing `<section class="now-playing-card">...</section>` block (currently lines 107-170) in an `{#if}/{:else}` so collapsed mode renders a dot instead. Replace:
```svelte
<section class="now-playing-card">
  <div class="np-header">
```
with:
```svelte
{#if collapsed}
  <div class="np-dot-wrap" title={dotTitle}>
    <span class="np-dot" class:active={dotActive}></span>
  </div>
{:else}
<section class="now-playing-card">
  <div class="np-header">
```
and replace the closing `</section>` (currently the last line of the markup, line 170) with:
```svelte
</section>
{/if}
```

- [ ] **Step 3: Add CSS for the dot**

In the `<style>` block, after the existing `.now-playing-card` rule (`next/src/lib/NowPlaying.svelte:173-183`), add:
```css
  .np-dot-wrap {
    display: flex;
    justify-content: center;
    padding: 0.4rem 0;
  }

  .np-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--color-muted);
  }

  .np-dot.active {
    background: #7ee87e;
    box-shadow: 0 0 6px rgba(126, 232, 126, 0.6);
  }
```

- [ ] **Step 4: Verify it typechecks**

Run from `next/`:
```
npm run check
```
Expected: passes with no errors. (`App.svelte` doesn't pass `collapsed` yet — that's fine, the prop has a default.)

- [ ] **Step 5: Commit**

```bash
git add next/src/lib/NowPlaying.svelte
git commit -m "feat: NowPlaying renders a status dot when collapsed"
```

---

### Task 3: `App.svelte` — collapsible rail, icons, persistence, toggle

**Files:**
- Modify: `next/src/App.svelte`

**Interfaces:**
- Consumes: `lucide-svelte` icon components (Task 1), `NowPlaying`'s `collapsed` prop (Task 2).
- Produces: nothing consumed elsewhere — this is the top-level shell.

- [ ] **Step 1: Import icons**

At the top of the `<script>` block in `next/src/App.svelte`, after the existing `bannerUrl` import (line 14), add:
```svelte
  import {
    LayoutDashboard,
    Library,
    CalendarRange,
    Search,
    Calendar,
    History,
    BarChart3,
    Settings as SettingsIcon,
    ChevronLeft,
    ChevronRight,
  } from 'lucide-svelte';
```

- [ ] **Step 2: Add the icon map**

Directly after the `navItems` array (`next/src/App.svelte:18-27`), add:
```svelte
  const navIcons: Partial<Record<View, typeof LayoutDashboard>> = {
    dashboard: LayoutDashboard,
    library: Library,
    season: CalendarRange,
    search: Search,
    calendar: Calendar,
    history: History,
    stats: BarChart3,
    settings: SettingsIcon,
  };
```

- [ ] **Step 3: Add collapsed state and persistence**

After the existing `let eventIntervalId` declaration (line 33), add:
```svelte
  const RAIL_COLLAPSED_KEY = 'anivault-rail-collapsed';

  function loadCollapsed(): boolean {
    try { return localStorage.getItem(RAIL_COLLAPSED_KEY) === 'true'; }
    catch { return false; }
  }

  function persistCollapsed(value: boolean) {
    try { localStorage.setItem(RAIL_COLLAPSED_KEY, String(value)); } catch {}
  }

  let collapsed = loadCollapsed();

  function toggleCollapse() {
    collapsed = !collapsed;
    persistCollapsed(collapsed);
  }
```

- [ ] **Step 4: Update the rail markup**

Replace the `<aside class="rail" aria-label="Main navigation">...</aside>` block (`next/src/App.svelte:100-121`) with:
```svelte
  <aside class="rail" class:collapsed aria-label="Main navigation">
    <div class="rail-top">
      <div class="brand-block">
        <img class="brand-banner" src={bannerUrl} alt="AniVault" />
        <div class="brand-label">AniVault</div>
      </div>
      <button
        type="button"
        class="collapse-toggle"
        aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        on:click={toggleCollapse}
      >
        <svelte:component this={collapsed ? ChevronRight : ChevronLeft} size={16} />
      </button>
    </div>
    <nav class="nav-list">
      {#each navItems as item}
        <button
          type="button"
          class="nav-item"
          class:active={isNavActive(item.id)}
          class:subtle-active={currentView === 'detail' && item.id === 'library'}
          title={item.label}
          aria-label={item.label}
          on:click={() => setView(item.id)}
        >
          <svelte:component this={navIcons[item.id]} class="nav-icon" size={18} />
          <span class="nav-label">{item.label}</span>
        </button>
      {/each}
    </nav>
    <div class="now-playing-sidebar">
      <NowPlaying events={latestEvents} {collapsed} />
    </div>
  </aside>
```

- [ ] **Step 5: Update the CSS**

In `next/src/App.svelte:146-243` (the `<style>` block, up to but not including the `@media` block), make these changes:

Change `.shell`'s grid columns — replace:
```css
  .shell {
    display: grid;
    grid-template-columns: 16rem 1fr;
    min-height: 100vh;
  }
```
with:
```css
  .shell {
    display: grid;
    grid-template-columns: auto 1fr;
    min-height: 100vh;
  }
```

Add `width`/`transition` to `.rail` and a `.collapsed` variant — replace:
```css
  .rail {
    border-right: 1px solid rgb(255 255 255 / 8%);
    background: rgb(10 13 20 / 72%);
    padding: 1.5rem;
    backdrop-filter: blur(24px);
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }
```
with:
```css
  .rail {
    border-right: 1px solid rgb(255 255 255 / 8%);
    background: rgb(10 13 20 / 72%);
    padding: 1.5rem;
    backdrop-filter: blur(24px);
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    width: 16rem;
    transition: width 0.2s ease, padding 0.2s ease;
  }

  .rail.collapsed {
    width: 4.5rem;
    padding: 1.5rem 0.75rem;
  }

  .rail-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .rail.collapsed .rail-top {
    flex-direction: column;
    gap: 0.75rem;
  }

  .rail.collapsed .brand-banner {
    width: 32px;
    height: 32px;
    max-width: none;
    object-fit: cover;
    border-radius: 8px;
  }

  .rail.collapsed .brand-label {
    display: none;
  }

  .collapse-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.75rem;
    height: 1.75rem;
    border: 1px solid rgb(255 255 255 / 12%);
    border-radius: 8px;
    background: transparent;
    color: var(--color-muted);
    cursor: pointer;
    flex-shrink: 0;
    padding: 0;
  }

  .collapse-toggle:hover {
    color: var(--color-text);
    background: rgb(255 255 255 / 8%);
  }

  .collapse-toggle:focus-visible {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }
```

Change `.nav-item` from block to flex layout, and add icon/label rules — replace:
```css
  .nav-item {
    display: block;
    width: 100%;
    border: 0;
    border-radius: 999px;
    padding: 0.8rem 1rem;
    text-align: left;
    color: var(--color-muted);
    background: transparent;
    cursor: pointer;
    font-family: inherit;
    font-size: 0.9rem;
    transition: background 0.15s ease, color 0.15s ease;
  }
```
with:
```css
  .nav-item {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    width: 100%;
    border: 0;
    border-radius: 999px;
    padding: 0.8rem 1rem;
    text-align: left;
    color: var(--color-muted);
    background: transparent;
    cursor: pointer;
    font-family: inherit;
    font-size: 0.9rem;
    transition: background 0.15s ease, color 0.15s ease;
  }

  .nav-icon {
    flex-shrink: 0;
  }

  .nav-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .rail.collapsed .nav-item {
    justify-content: center;
    padding: 0.8rem 0;
  }

  .rail.collapsed .nav-label {
    display: none;
  }
```

- [ ] **Step 6: Neutralize collapsed styling on mobile**

Inside the existing `@media (max-width: 768px) { ... }` block (`next/src/App.svelte:244-279`), add these rules right after the existing `.nav-item { width: auto; padding: 0.6rem 0.9rem; }` rule:
```css
    .collapse-toggle {
      display: none;
    }

    .rail.collapsed {
      width: auto;
      padding: 1rem;
    }

    .rail.collapsed .rail-top {
      flex-direction: row;
    }

    .rail.collapsed .brand-banner {
      width: 100%;
      height: auto;
      max-width: 10rem;
      object-fit: contain;
      border-radius: 12px;
    }

    .rail.collapsed .brand-label {
      display: block;
    }

    .rail.collapsed .nav-item {
      justify-content: flex-start;
      padding: 0.6rem 0.9rem;
    }

    .rail.collapsed .nav-label {
      display: inline;
    }
```

- [ ] **Step 7: Verify it typechecks and existing tests still pass**

Run from `next/`:
```
npm run check
npm run test
```
Expected: both pass with no errors and no failing tests.

- [ ] **Step 8: Commit**

```bash
git add next/src/App.svelte
git commit -m "feat: collapsible left navigation rail"
```

---

### Task 4: Manual verification

**Files:** none (no code changes — this task only verifies Tasks 1-3)

**Interfaces:** none.

- [ ] **Step 1: Start the dev app**

Run from `next/`:
```
npm run dev
```
Open the printed local URL in a browser at a width above 768px (or run via Tauri dev if that's the normal workflow for this project).

- [ ] **Step 2: Walk through the checklist**

Verify, and note any failures:
1. Rail starts expanded (full banner, "AniVault" label, text nav items, chevron pointing left).
2. Click the chevron: rail animates to icon-only width; banner becomes a small square crop; label disappears; nav items show only icons, centered; chevron now points right; NowPlaying card is replaced by a single status dot.
3. Hover a collapsed nav icon: a native tooltip with the view name (e.g. "Library") appears.
4. Click the chevron again: rail expands back to the original layout, text labels return, NowPlaying full card returns.
5. Collapse the rail, then reload the page (or restart the Tauri dev window): rail stays collapsed (localStorage persisted).
6. Expand it again, reload: rail stays expanded.
7. Resize the window below 768px: mobile horizontal nav bar appears as before, with no chevron button visible, regardless of the collapsed state set in step 5/6.
8. Resize back above 768px: rail returns to whichever collapsed/expanded state was last set.

- [ ] **Step 3: Report results**

If every item in Step 2 passes, the feature is complete. If anything fails, note which checklist item and what was observed, then fix the relevant task's code before re-running this checklist.

---

## Self-Review Notes

- Spec coverage: state/persistence (Task 3 Step 3), rail layout/CSS/toggle placement (Task 3 Steps 4-5), icon mapping (Task 3 Steps 1-2), NowPlaying dot (Task 2), mobile exclusion (Task 3 Step 6), dependency addition (Task 1), manual-only testing per spec (Task 4) — all covered.
- Type consistency checked: `collapsed` prop name/type matches between `NowPlaying.svelte` (Task 2) and `App.svelte`'s usage (Task 3 Step 4); `navIcons` keys match `View` values used by `navItems`; localStorage key string `anivault-rail-collapsed` matches spec exactly.
