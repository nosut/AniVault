<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { drainEngineEvents, onEngineEventsReady, checkForUpdate, openEpisodeFile, getUpNext, notifyUpNext, getSetting, type EngineEvent, type SeasonAnimeEntry, type UpdateInfo, type WatchHistoryEntry, type UpNext } from './lib/api';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { dismissUpdate, loadDismissedUpdate, shouldShowUpdate } from './lib/updateUi';
  import { latestPlaybackEnded, samePrompt, type PromptKey } from './lib/upNext';
  import NowPlaying from './lib/NowPlaying.svelte';
  import DashboardView from './lib/DashboardView.svelte';
  import LibraryView from './lib/LibraryView.svelte';
  import CollectionView from './lib/CollectionView.svelte';
  import DetailView from './lib/DetailView.svelte';
  import SettingsView from './lib/SettingsView.svelte';
  import CalendarView from './lib/CalendarView.svelte';
  import StatsView from './lib/StatsView.svelte';
  import HistoryView from './lib/HistoryView.svelte';
  import SeasonView from './lib/SeasonView.svelte';
  import SearchView from './lib/SearchView.svelte';
  import { loadStartPage } from './lib/startPage';
  import { DEFAULT_NAV_ITEMS, clearNavOrder, loadNavOrder, moveNavItem, saveNavOrder, type NavId } from './lib/navOrder';
  import bannerUrl from './assets/banner.png';
  import iconUrl from '../src-tauri/icons/icon.png';
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
    HardDrive,
  } from 'lucide-svelte';

  type View = 'dashboard' | 'library' | 'collection' | 'season' | 'search' | 'calendar' | 'history' | 'detail' | 'stats' | 'settings';

  let navOrder: NavId[] = loadNavOrder();
  $: navItems = navOrder.map((id) => DEFAULT_NAV_ITEMS.find((item) => item.id === id)!);

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
      e.dataTransfer.setData('text/plain', navOrder[index]!);
    }
  }

  function handleNavDragOver(e: DragEvent, index: number) {
    if (dragIndex === null) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    dropIndex = e.clientY < rect.top + rect.height / 2 ? index : index + 1;
  }

  // Fires while the pointer is over the nav's own background — in particular
  // the inter-item gap the drop indicator is drawn into (see .nav-item.drop-
  // above/.drop-below, offset -2px/-2px into that gap). Without this, dragover
  // is only bound on the buttons, so the gap never gets preventDefault() and
  // the browser refuses the drop there even though the indicator promised
  // one. dropIndex is deliberately left untouched: it already holds the last
  // insertion point computed by handleNavDragOver while crossing a button, and
  // that is exactly where the indicator is pointing.
  function handleNavListDragOver(e: DragEvent) {
    if (dragIndex === null) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
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

  // Clears drag state. Called on a successful drop (from commitNavDrop, after
  // it has already committed the reorder) and on dragend, which fires after
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

  let navButtons: HTMLButtonElement[] = [];
  let navAnnouncement = '';

  async function handleNavKeydown(e: KeyboardEvent, index: number) {
    if (collapsed) return;
    if (!e.altKey || e.ctrlKey || e.shiftKey || e.metaKey) return;
    if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
    const to = e.key === 'ArrowUp' ? index - 1 : index + 1;
    if (to < 0 || to >= navOrder.length) return;
    e.preventDefault();
    const label = navItems[index]!.label;
    navOrder = moveNavItem(navOrder, index, to);
    saveNavOrder(navOrder);
    navAnnouncement = `${label} moved to position ${to + 1} of ${navOrder.length}`;
    // Focus follows the item, not the index it used to sit at.
    await tick();
    navButtons[to]?.focus();
  }

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

  const navIcons: Partial<Record<View, typeof LayoutDashboard>> = {
    dashboard: LayoutDashboard,
    library: Library,
    collection: HardDrive,
    season: CalendarRange,
    search: Search,
    calendar: Calendar,
    history: History,
    stats: BarChart3,
    settings: SettingsIcon,
  };

  let currentView: View = loadStartPage();
  let previousView: View = currentView;
  let detailAnimeId: number | null = null;
  let searchQuery = '';
  let searchEntries: SeasonAnimeEntry[] = [];
  let searchHasSearched = false;
  let historyEntries: WatchHistoryEntry[] = [];
  let historyQuery = '';
  let historyOffset = 0;
  let historyHasMore = true;
  let latestEvents: EngineEvent[] = [];
  let eventIntervalId: ReturnType<typeof setInterval> | null = null;
  let unlistenEvents: UnlistenFn | null = null;

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

  let isDesktopRail = true;
  let railMediaQuery: MediaQueryList | null = null;

  function updateIsDesktopRail() {
    isDesktopRail = railMediaQuery ? railMediaQuery.matches : true;
  }

  async function pollEvents() {
    try {
      const events = await drainEngineEvents();
      latestEvents = events;
      void maybePromptUpNext(events);
    } catch {
      // Keep polling alive; individual errors are surfaced by consumers if needed.
    }
  }

  let upNextPrompt: UpNext | null = null;
  let lastPromptKey: PromptKey | null = null;

  // The prompt fires when playback of a library episode ends — not when progress
  // advances, so marking episodes watched by hand never prompts.
  async function maybePromptUpNext(events: EngineEvent[]) {
    try {
      const ended = latestPlaybackEnded(events);
      if (!ended) return;
      const toastOn = (await getSetting<boolean>('up_next_toast_enabled')) ?? true;
      const notifyOn = (await getSetting<boolean>('up_next_notification_enabled')) ?? true;
      if (!toastOn && !notifyOn) return;
      const next = await getUpNext(ended.anime_id, ended.episode);
      if (!next) return;
      const key: PromptKey = { anime_id: next.anime_id, episode: next.episode };
      if (samePrompt(key, lastPromptKey)) return; // already surfaced this one
      lastPromptKey = key;
      if (toastOn) upNextPrompt = next;
      if (notifyOn) void notifyUpNext(next.title, next.episode);
    } catch {
      // Best-effort; a failed lookup just means no prompt this cycle.
    }
  }

  // Clearing the key alongside the toast keeps the guard doing its one job —
  // not raising a second toast over one already on screen — while letting the
  // same episode prompt again later, e.g. after rewatching it.
  function playUpNext() {
    if (upNextPrompt) openEpisodeFile(upNextPrompt.file_path);
    upNextPrompt = null;
    lastPromptKey = null;
  }
  function dismissUpNext() {
    upNextPrompt = null;
    lastPromptKey = null;
  }

  function handleLibrarySelect(event: CustomEvent<{ anime_id: number }>) {
    previousView = currentView;
    detailAnimeId = event.detail.anime_id;
    currentView = 'detail';
  }

  function handleCollectionSelect(event: CustomEvent<{ anime_id: number }>) {
    previousView = currentView;
    detailAnimeId = event.detail.anime_id;
    currentView = 'detail';
  }

  function handleSeasonSelect(event: CustomEvent<{ anime_id: number }>) {
    previousView = currentView;
    detailAnimeId = event.detail.anime_id;
    currentView = 'detail';
  }

  function handleSearchSelect(event: CustomEvent<{ anime_id: number }>) {
    previousView = currentView;
    detailAnimeId = event.detail.anime_id;
    currentView = 'detail';
  }

  function handleCalendarSelect(event: CustomEvent<{ anime_id: number }>) {
    previousView = currentView;
    detailAnimeId = event.detail.anime_id;
    currentView = 'detail';
  }

  function handleDetailSelect(event: CustomEvent<{ anime_id: number }>) {
    detailAnimeId = event.detail.anime_id;
    currentView = 'detail';
  }

  function handleDetailBack() {
    currentView = previousView;
  }

  function isNavActive(itemId: View): boolean {
    if (itemId === currentView) return true;
    if (itemId === 'library' && currentView === 'detail') return true;
    return false;
  }

  function setView(view: View) {
    currentView = view;
    if (view !== 'detail') {
      detailAnimeId = null;
    }
  }

  let update: UpdateInfo | null = null;
  let updateDismissed = loadDismissedUpdate();

  function openUpdate() {
    if (update) openEpisodeFile(update.url);
  }
  function hideUpdate() {
    if (!update) return;
    dismissUpdate(update.latest);
    updateDismissed = update.latest;
  }

  onMount(() => {
    eventIntervalId = setInterval(pollEvents, 3000);
    // The timer is the floor, not the latency budget: the backend pushes a
    // signal the moment playback ends so the Up Next prompt does not sit
    // waiting for the next tick. Failing to attach costs responsiveness, not
    // correctness — the timer still catches everything.
    onEngineEventsReady(() => { void pollEvents(); })
      .then((un) => { unlistenEvents = un; })
      .catch(() => {});
    railMediaQuery = window.matchMedia('(min-width: 769px)');
    updateIsDesktopRail();
    railMediaQuery.addEventListener('change', updateIsDesktopRail);
    // Best-effort; offline or rate-limited just means no banner.
    checkForUpdate().then((u) => { update = u; }).catch(() => {});
  });

  onDestroy(() => {
    if (eventIntervalId) clearInterval(eventIntervalId);
    unlistenEvents?.();
    railMediaQuery?.removeEventListener('change', updateIsDesktopRail);
  });
</script>

<main class="shell">
  <aside class="rail" class:collapsed aria-label="Main navigation">
    <div class="rail-top">
      <div class="brand-block">
        <img class="brand-banner" src={collapsed ? iconUrl : bannerUrl} alt="AniVault" />
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
    <nav
      class="nav-list"
      on:contextmenu={openNavContextMenu}
      on:dragover={handleNavListDragOver}
      on:drop|preventDefault={commitNavDrop}
    >
      {#each navItems as item, i (item.id)}
        <button
          type="button"
          class="nav-item"
          class:active={isNavActive(item.id)}
          class:subtle-active={currentView === 'detail' && item.id === 'library'}
          class:dragging={dragIndex === i}
          class:drop-above={dragIndex !== null && dropIndex === i}
          class:drop-below={dragIndex !== null && dropIndex === navItems.length && i === navItems.length - 1}
          bind:this={navButtons[i]}
          draggable={!collapsed && isDesktopRail}
          title={item.label}
          aria-label={item.label}
          on:pointerdown={() => (justDragged = false)}
          on:dragstart={(e) => handleNavDragStart(e, i)}
          on:dragover={(e) => handleNavDragOver(e, i)}
          on:drop|preventDefault={commitNavDrop}
          on:dragend={clearNavDragState}
          on:keydown={(e) => handleNavKeydown(e, i)}
          on:click={(e) => handleNavClick(item.id, e)}
        >
          <svelte:component this={navIcons[item.id]} class="nav-icon" size={18} />
          <span class="nav-label">{item.label}</span>
        </button>
      {/each}
    </nav>
    <div class="sr-only" aria-live="polite">{navAnnouncement}</div>
    <div class="now-playing-sidebar">
      <NowPlaying events={latestEvents} collapsed={collapsed && isDesktopRail} />
    </div>
  </aside>

  <section class="content">
    {#if update && shouldShowUpdate(update, updateDismissed)}
      <div class="update-banner" role="status">
        <span>AniVault {update.latest} is available</span>
        <button class="update-link" on:click={openUpdate}>View release</button>
        <button class="update-dismiss" aria-label="Dismiss update notice" on:click={hideUpdate}>×</button>
      </div>
    {/if}
    {#if currentView === 'dashboard'}
      <DashboardView
        events={latestEvents}
        on:select={handleLibrarySelect}
        on:navigate={(e) => { previousView = currentView; currentView = e.detail.view as View; }}
      />
    {:else if currentView === 'library'}
      <LibraryView events={latestEvents} on:select={handleLibrarySelect} />
    {:else if currentView === 'collection'}
      <CollectionView events={latestEvents} on:select={handleCollectionSelect} />
    {:else if currentView === 'season'}
      <SeasonView on:select={handleSeasonSelect} />
    {:else if currentView === 'search'}
      <SearchView bind:query={searchQuery} bind:entries={searchEntries} bind:hasSearched={searchHasSearched} on:select={handleSearchSelect} />
    {:else if currentView === 'calendar'}
      <CalendarView on:select={handleCalendarSelect} />
    {:else if currentView === 'history'}
      <HistoryView bind:entries={historyEntries} bind:query={historyQuery} bind:offset={historyOffset} bind:hasMore={historyHasMore} />
    {:else if currentView === 'stats'}
      <StatsView />
    {:else if currentView === 'detail' && detailAnimeId !== null}
      <DetailView animeId={detailAnimeId} events={latestEvents} on:back={handleDetailBack} on:select={handleDetailSelect} />
    {:else if currentView === 'settings'}
      <SettingsView events={latestEvents} />
    {/if}
  </section>

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

  {#if upNextPrompt}
    <div class="up-next-toast" role="dialog" aria-label="Up next">
      {#if upNextPrompt.image_url}
        <img class="un-thumb" src={upNextPrompt.image_url} alt="" />
      {/if}
      <div class="un-body">
        <span class="un-eyebrow">Up Next</span>
        <span class="un-title">{upNextPrompt.title}</span>
        <span class="un-ep">Episode {upNextPrompt.episode}</span>
      </div>
      <div class="un-actions">
        <button class="un-play" on:click={playUpNext}>▶ Play</button>
        <button class="un-dismiss" aria-label="Dismiss" on:click={dismissUpNext}>×</button>
      </div>
    </div>
  {/if}
</main>

<svelte:window on:keydown={(e) => { if (navCtxMenu && e.key === 'Escape') navCtxMenu = null; }} />

<style>
  .shell {
    display: grid;
    grid-template-columns: auto 1fr;
    grid-template-rows: 1fr;
    height: 100vh;
    height: 100dvh;
  }

  .rail {
    border-right: 1px solid rgb(255 255 255 / 8%);
    background: rgb(10 13 20 / 72%);
    padding: 1.5rem;
    backdrop-filter: blur(24px);
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    width: 16rem;
    min-height: 0;
    overflow-y: auto;
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
    outline: 2px solid rgba(var(--color-accent-rgb), 0.5);
    outline-offset: 2px;
  }

  .brand-block {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.6rem;
  }

  .brand-banner {
    display: block;
    width: 100%;
    max-width: 10rem;
    height: auto;
    border-radius: 12px;
  }

  .brand-label {
    font-weight: 800;
    letter-spacing: -0.04em;
    font-size: 1.1rem;
    color: var(--color-text);
  }

  .nav-list {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

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

  .nav-item:hover {
    color: var(--color-text);
    background: rgb(255 255 255 / 8%);
  }

  .nav-item.active {
    color: var(--color-text);
    background: rgb(255 255 255 / 10%);
  }

  .nav-item.subtle-active {
    color: var(--color-accent);
  }

  .nav-item:focus-visible {
    outline: 2px solid rgba(var(--color-accent-rgb), 0.5);
    outline-offset: 2px;
  }

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

  .now-playing-sidebar {
    margin-top: auto;
    padding-top: 1rem;
    border-top: 1px solid rgb(255 255 255 / 8%);
  }

  .content {
    padding: 1.5rem;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    max-width: 100%;
  }

  .update-banner {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 1rem;
    padding: 0.5rem 0.9rem;
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    border-radius: 10px;
    background: rgba(var(--color-accent-rgb), 0.1);
    font-size: 0.85rem;
    color: var(--color-text);
  }

  .update-link {
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    border-radius: 999px;
    padding: 0.25rem 0.75rem;
    background: rgba(var(--color-accent-rgb), 0.18);
    color: var(--color-text);
    font-size: 0.78rem;
    cursor: pointer;
  }
  .update-link:hover { background: rgba(var(--color-accent-rgb), 0.3); }

  .update-dismiss {
    margin-left: auto;
    border: none;
    background: transparent;
    color: var(--color-muted);
    font-size: 1rem;
    cursor: pointer;
    padding: 0 0.3rem;
  }
  .update-dismiss:hover { color: var(--color-text); }

  .up-next-toast {
    position: fixed; right: 1.25rem; bottom: 1.25rem; z-index: 50;
    display: flex; align-items: center; gap: 0.75rem; max-width: 22rem;
    padding: 0.7rem 0.9rem; border-radius: 12px;
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    background: rgba(16, 21, 32, 0.98); box-shadow: 0 12px 30px rgba(0,0,0,0.5);
  }
  .un-thumb { width: 40px; height: 56px; object-fit: cover; border-radius: 6px; flex: 0 0 auto; }
  .un-body { display: flex; flex-direction: column; min-width: 0; }
  .un-eyebrow { font-size: 0.66rem; font-weight: 800; letter-spacing: 0.12em; text-transform: uppercase; color: var(--color-accent); }
  .un-title { font-size: 0.85rem; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .un-ep { font-size: 0.72rem; color: var(--color-muted); }
  .un-actions { display: flex; align-items: center; gap: 0.4rem; margin-left: auto; }
  .un-play { border: 1px solid rgba(var(--color-accent-rgb), 0.35); border-radius: 999px; padding: 0.35rem 0.75rem; background: rgba(var(--color-accent-rgb), 0.18); color: var(--color-text); font-size: 0.78rem; cursor: pointer; white-space: nowrap; }
  .un-play:hover { background: rgba(var(--color-accent-rgb), 0.3); }
  .un-dismiss { border: none; background: transparent; color: var(--color-muted); font-size: 1.1rem; cursor: pointer; padding: 0 0.25rem; }
  .un-dismiss:hover { color: var(--color-text); }

  @media (max-width: 768px) {
    .shell {
      grid-template-columns: 1fr;
      grid-template-rows: auto 1fr;
    }

    .rail {
      border-right: none;
      border-bottom: 1px solid rgb(255 255 255 / 8%);
      flex-direction: row;
      align-items: center;
      padding: 1rem;
      width: auto;
    }

    .nav-list {
      flex-direction: row;
      gap: 0.5rem;
    }

    .nav-item {
      width: auto;
      padding: 0.6rem 0.9rem;
    }

    .collapse-toggle {
      display: none;
    }

    .rail.collapsed {
      width: auto;
      padding: 1rem;
    }

    .rail.collapsed .rail-top {
      flex-direction: row;
      gap: 0.5rem;
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

  .now-playing-sidebar {
    margin-top: auto;
    padding-top: 1rem;
    border-top: 1px solid rgb(255 255 255 / 8%);
  }

  .content {
      padding: 1rem;
      overflow-x: hidden;
      max-width: 100%;
    }
  }
</style>
