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

  let currentView: View = 'dashboard';
  let previousView: View = 'dashboard';
  let detailAnimeId: number | null = null;
  let latestEvents: EngineEvent[] = [];
  let eventIntervalId: ReturnType<typeof setInterval> | null = null;

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
  <aside class="rail" aria-label="Main navigation">
    <div class="brand-block">
      <img class="brand-banner" src={bannerUrl} alt="AniVault" />
      <div class="brand-label">AniVault</div>
    </div>
    <nav class="nav-list">
      {#each navItems as item}
        <button
          type="button"
          class="nav-item"
          class:active={isNavActive(item.id)}
          class:subtle-active={currentView === 'detail' && item.id === 'library'}
          on:click={() => setView(item.id)}
        >
          {item.label}
        </button>
      {/each}
    </nav>
    <div class="now-playing-sidebar">
      <NowPlaying events={latestEvents} />
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
      <SettingsView />
    {/if}
  </section>
</main>

<style>
  .shell {
    display: grid;
    grid-template-columns: 16rem 1fr;
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
