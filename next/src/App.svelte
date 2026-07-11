<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { drainEngineEvents, type EngineEvent } from './lib/api';
  import NowPlaying from './lib/NowPlaying.svelte';
  import DashboardView from './lib/DashboardView.svelte';
  import LibraryView from './lib/LibraryView.svelte';
  import DetailView from './lib/DetailView.svelte';
  import SettingsView from './lib/SettingsView.svelte';
  import CalendarView from './lib/CalendarView.svelte';
  import StatsView from './lib/StatsView.svelte';
  import HistoryView from './lib/HistoryView.svelte';
  import SeasonView from './lib/SeasonView.svelte';
  import SearchView from './lib/SearchView.svelte';
  import bannerUrl from './assets/banner.png';
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

  type View = 'dashboard' | 'library' | 'season' | 'search' | 'calendar' | 'history' | 'detail' | 'stats' | 'settings';

  const navItems = [
    { id: 'dashboard' as View, label: 'Dashboard' },
    { id: 'library' as View, label: 'Library' },
    { id: 'season' as View, label: 'Season' },
    { id: 'search' as View, label: 'Search' },
    { id: 'calendar' as View, label: 'Calendar' },
    { id: 'history' as View, label: 'History' },
    { id: 'stats' as View, label: 'Stats' },
    { id: 'settings' as View, label: 'Settings' },
  ];

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

  let currentView: View = 'dashboard';
  let previousView: View = 'dashboard';
  let detailAnimeId: number | null = null;
  let latestEvents: EngineEvent[] = [];
  let eventIntervalId: ReturnType<typeof setInterval> | null = null;

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

  async function pollEvents() {
    try {
      const events = await drainEngineEvents();
      latestEvents = events;
    } catch {
      // Keep polling alive; individual errors are surfaced by consumers if needed.
    }
  }

  function handleLibrarySelect(event: CustomEvent<{ anime_id: number }>) {
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

  onMount(() => {
    eventIntervalId = setInterval(pollEvents, 3000);
  });

  onDestroy(() => {
    if (eventIntervalId) clearInterval(eventIntervalId);
  });
</script>

<main class="shell">
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

  <section class="content">
    {#if currentView === 'dashboard'}
      <DashboardView events={latestEvents} />
    {:else if currentView === 'library'}
      <LibraryView events={latestEvents} on:select={handleLibrarySelect} />
    {:else if currentView === 'season'}
      <SeasonView on:select={handleSeasonSelect} />
    {:else if currentView === 'search'}
      <SearchView on:select={handleSearchSelect} />
    {:else if currentView === 'calendar'}
      <CalendarView on:select={handleCalendarSelect} />
    {:else if currentView === 'history'}
      <HistoryView />
    {:else if currentView === 'stats'}
      <StatsView />
    {:else if currentView === 'detail' && detailAnimeId !== null}
      <DetailView animeId={detailAnimeId} events={latestEvents} on:back={handleDetailBack} on:select={handleDetailSelect} />
    {:else if currentView === 'settings'}
      <SettingsView events={latestEvents} />
    {/if}
  </section>
</main>

<style>
  .shell {
    display: grid;
    grid-template-columns: auto 1fr;
    min-height: 100vh;
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

  .brand {
    font-weight: 800;
    letter-spacing: -0.04em;
    font-size: 1.1rem;
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
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .now-playing-sidebar {
    margin-top: auto;
    padding-top: 1rem;
    border-top: 1px solid rgb(255 255 255 / 8%);
  }

  .content {
    padding: 1.5rem;
    overflow-y: auto;
    overflow-x: hidden;
    max-width: 100%;
  }

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
