<script lang="ts">
  import { onMount } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import {
    getCollection,
    getEpisodeFiles,
    openEpisodeFile,
    openContainingFolder,
    deleteAnime,
    updateListEntry,
    type CollectionEntry,
    type FileIndexEntry,
    type EngineEvent,
  } from './api';
  import {
    filterCollection,
    isComplete,
    type CollectionFilter,
  } from './collectionUi';
  import { Play, FolderOpen, Info, Trash2, ChevronRight, ChevronLeft, ChevronUp, ChevronDown, LayoutGrid, List } from 'lucide-svelte';

  export let events: EngineEvent[] = [];
  const dispatch = createEventDispatcher<{ select: { anime_id: number } }>();

  function loadPref(k: string, f: string) { try { return localStorage.getItem(k) ?? f; } catch { return f; } }
  function persistPref(k: string, v: string) { try { localStorage.setItem(k, v); } catch {} }

  let entries: CollectionEntry[] = [];
  let loading = false;
  let error = '';
  let query = loadPref('anivault-collection-query', '');
  let filter = loadPref('anivault-collection-filter', 'all') as CollectionFilter;
  let viewMode = loadPref('anivault-collection-viewmode', 'grid') as 'grid' | 'table';
  let compact = loadPref('anivault-collection-compact', 'false') === 'true';
  let sortKey = loadPref('anivault-collection-sortkey', 'recent') as 'recent' | 'title' | 'status' | 'progress';
  let sortDir = loadPref('anivault-collection-sortdir', 'desc') as 'asc' | 'desc';
  let episodeFilesMap = new Map<number, FileIndexEntry[]>();

  $: persistPref('anivault-collection-query', query);
  $: persistPref('anivault-collection-filter', filter);
  $: persistPref('anivault-collection-sortkey', sortKey);
  $: persistPref('anivault-collection-sortdir', sortDir);
  $: persistPref('anivault-collection-viewmode', viewMode);
  $: persistPref('anivault-collection-compact', compact ? 'true' : 'false');
  $: visible = (() => {
    const list = filterCollection(entries, filter, query);
    const dir = sortDir === 'desc' ? -1 : 1;
    list.sort((a, b) => {
      let cmp = 0;
      switch (sortKey) {
        case 'title': cmp = a.title.localeCompare(b.title); break;
        case 'status': cmp = a.status.localeCompare(b.status); break;
        case 'progress': cmp = a.watched_episodes - b.watched_episodes; break;
        case 'recent':
        default: cmp = a.last_indexed_at - b.last_indexed_at; break;
      }
      return cmp * dir;
    });
    return list;
  })();

  const FILTERS: { value: CollectionFilter; label: string }[] = [
    { value: 'all', label: 'All' },
    { value: 'new', label: 'New' },
    { value: 'complete', label: 'Complete' },
    { value: 'incomplete', label: 'Incomplete' },
  ];

  const STATUS_OPTIONS: { value: string; label: string }[] = [
    { value: 'watching', label: 'Watching' },
    { value: 'completed', label: 'Completed' },
    { value: 'on_hold', label: 'On Hold' },
    { value: 'dropped', label: 'Dropped' },
    { value: 'plan_to_watch', label: 'Plan to Watch' },
  ];

  async function load() {
    loading = true; error = '';
    episodeFilesMap = new Map();
    try {
      entries = await getCollection();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally { loading = false; }
  }

  function completeness(e: CollectionEntry): number {
    if (e.episode_count && e.episode_count > 0) return Math.min(100, (e.max_downloaded_episode / e.episode_count) * 100);
    return 100;
  }

  // Table-view helpers (mirror LibraryView's table columns).
  function formatStatus(status: string): string {
    return status.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
  }
  function totalLabel(e: CollectionEntry): string | number {
    return e.episode_count && e.episode_count > 0 ? e.episode_count : '?';
  }
  function watchPct(e: CollectionEntry): number {
    const total = Math.max(e.episode_count ?? 0, e.watched_episodes);
    return total > 0 ? Math.min(100, (e.watched_episodes / total) * 100) : 0;
  }
  function fullyWatched(e: CollectionEntry): boolean {
    return e.watched_episodes > 0 && e.episode_count != null && e.watched_episodes >= e.episode_count;
  }

  function setSort(key: 'recent' | 'title' | 'status' | 'progress') {
    if (sortKey === key) {
      sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    } else {
      sortKey = key;
      sortDir = 'asc';
    }
  }

  function playNext(e: CollectionEntry) {
    if (e.next_episode_path) openEpisodeFile(e.next_episode_path);
  }

  function open(e: CollectionEntry) { dispatch('select', { anime_id: e.anime_id }); }

  // Reload when the file index or progress changes (mirrors LibraryView).
  $: if (events && events.some((ev) => 'LibraryUpdated' in ev || 'ProgressAdvanced' in ev)) void load();

  // Right-click context menu.
  let ctxMenu: { x: number; y: number; entry: CollectionEntry } | null = null;

  async function openContextMenu(e: MouseEvent, entry: CollectionEntry) {
    e.preventDefault();
    ctxMenu = { x: Math.min(e.clientX, window.innerWidth - 240), y: Math.min(e.clientY, window.innerHeight - 320), entry };
    // Lazy-load this anime's full file list on demand — only the series being
    // right-clicked, not the whole collection (see load()).
    if (!episodeFilesMap.has(entry.anime_id)) {
      try {
        const files = await getEpisodeFiles(entry.anime_id);
        episodeFilesMap.set(entry.anime_id, files);
        episodeFilesMap = new Map(episodeFilesMap);
      } catch { /* ignore */ }
    }
  }
  function closeContextMenu() { ctxMenu = null; }

  function ctxFiles(): { ep: number; path: string }[] {
    if (!ctxMenu) return [];
    return (episodeFilesMap.get(ctxMenu.entry.anime_id) ?? [])
      .map((f) => ({ ep: f.episode ?? 0, path: f.file_path }))
      .filter((f) => f.ep > 0)
      .sort((a, b) => a.ep - b.ep);
  }
  function playCtxPath(path: string) { openEpisodeFile(path); closeContextMenu(); }
  function ctxNextEp(): number | null {
    const menu = ctxMenu;
    if (!menu) return null;
    const files = ctxFiles();
    const f = files.find((x) => x.ep === menu.entry.watched_episodes + 1)
      ?? files.find((x) => x.ep > menu.entry.watched_episodes);
    return f ? f.ep : null;
  }
  function ctxPrevEp(): number | null {
    const menu = ctxMenu;
    if (!menu) return null;
    const want = menu.entry.watched_episodes;
    const f = ctxFiles().filter((x) => x.ep <= want).sort((a, b) => b.ep - a.ep)[0];
    return f ? f.ep : null;
  }
  function playCtxEp(ep: number | null) {
    if (ep == null) return;
    const f = ctxFiles().find((x) => x.ep === ep);
    if (f) playCtxPath(f.path);
  }
  function ctxOpenFolder() {
    const first = ctxFiles()[0];
    if (first) openContainingFolder(first.path);
    closeContextMenu();
  }
  function ctxOpenDetails() {
    if (!ctxMenu) return;
    const entry = ctxMenu.entry;
    closeContextMenu();
    open(entry);
  }
  async function ctxSetStatus(status: string) {
    if (!ctxMenu) return;
    const id = ctxMenu.entry.anime_id;
    closeContextMenu();
    try {
      await updateListEntry(id, { status });
      await load();
    } catch { /* ignore */ }
  }
  async function ctxRemove() {
    if (!ctxMenu) return;
    const id = ctxMenu.entry.anime_id;
    closeContextMenu();
    try {
      await deleteAnime(id);
      entries = entries.filter((e) => e.anime_id !== id);
    } catch { /* ignore */ }
  }

  onMount(load);
</script>

<div class="collection-view">
  <div class="lib-header">
    <div class="controls">
      <input
        type="text"
        class="search"
        placeholder="Search collection…"
        bind:value={query}
        aria-label="Search collection"
      />
      <button class="view-toggle" on:click={() => (viewMode = viewMode === 'table' ? 'grid' : 'table')} aria-label="Toggle view">
        {#if viewMode === 'table'}
          <LayoutGrid size={14} /> Grid
        {:else}
          <List size={14} /> Table
        {/if}
      </button>
      {#if viewMode === 'table'}
        <button class="view-toggle" on:click={() => (compact = !compact)} aria-pressed={compact} title="Toggle compact list density">
          {compact ? '≣ Comfortable' : '≡ Compact'}
        </button>
      {/if}
    </div>
    <div class="status-tabs">
      {#each FILTERS as f}
        <button
          type="button"
          class="status-tab"
          class:active={filter === f.value}
          on:click={() => (filter = f.value)}
        >
          {f.label}
        </button>
      {/each}
    </div>
  </div>

  {#if error}
    <div class="message error" role="alert">
      <p>{error}</p>
      <button type="button" class="retry" on:click={load}>Retry</button>
    </div>
  {/if}

  {#if loading}
    <div class="poster-grid">
      {#each Array.from({ length: 10 }) as _, i (i)}
        <div class="poster-card skeleton-card">
          <div class="poster-thumb placeholder"></div>
          <div class="poster-info">
            <div class="skeleton-line"></div>
            <div class="skeleton-line short"></div>
          </div>
        </div>
      {/each}
    </div>
  {:else if visible.length === 0}
    <p class="empty">No downloaded series yet.</p>
  {:else if viewMode === 'grid'}
    <div class="poster-grid">
      {#each visible as entry (entry.anime_id)}
        <div
          class="poster-card"
          tabindex="0"
          role="button"
          aria-label={`${entry.title}, ${entry.max_downloaded_episode} downloaded`}
          on:click={() => open(entry)}
          on:keydown={(e) => e.key === 'Enter' && open(entry)}
          on:contextmenu={(e) => openContextMenu(e, entry)}
        >
          {#if entry.image_url}
            <img class="poster-thumb" src={entry.image_url} alt={entry.title} loading="lazy" />
          {:else}
            <div class="poster-thumb placeholder"></div>
          {/if}
          {#if entry.new_count > 0}
            <span class="badge new-badge">{entry.new_count} new</span>
          {/if}
          <button
            type="button"
            class="play-next-btn"
            on:click|stopPropagation={() => playNext(entry)}
            aria-label={`Play next episode of ${entry.title}`}
          >
            <Play size={22} />
          </button>
          <div class="poster-info">
            <p class="poster-title">{entry.title}</p>
            <div class="ep-download-bar completeness-bar">
              <div
                class="completeness-fill"
                class:complete={isComplete(entry)}
                style="width: {completeness(entry)}%"
              ></div>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {:else if viewMode === 'table'}
    <div class="table-wrap">
      <table class:compact>
        <thead>
          <tr>
            <th class="col-thumb" scope="col"><span class="sr-only">Thumbnail</span></th>
            <th
              class="col-title"
              scope="col"
              aria-sort={sortKey === 'title'
                ? sortDir === 'asc'
                  ? 'ascending'
                  : 'descending'
                : 'none'}
            >
              <button
                type="button"
                class="sort-btn"
                aria-label="Sort by title"
                on:click={() => setSort('title')}
              >
                Title
                {#if sortKey === 'title'}
                  <span aria-hidden="true" class="sort-arrow">
                    {#if sortDir === 'asc'}<ChevronUp size={13} />{:else}<ChevronDown size={13} />{/if}
                  </span>
                {/if}
              </button>
            </th>
            <th
              class="col-status"
              scope="col"
              aria-sort={sortKey === 'status'
                ? sortDir === 'asc'
                  ? 'ascending'
                  : 'descending'
                : 'none'}
            >
              <button
                type="button"
                class="sort-btn"
                aria-label="Sort by status"
                on:click={() => setSort('status')}
              >
                Status
                {#if sortKey === 'status'}
                  <span aria-hidden="true" class="sort-arrow">
                    {#if sortDir === 'asc'}<ChevronUp size={13} />{:else}<ChevronDown size={13} />{/if}
                  </span>
                {/if}
              </button>
            </th>
            <th
              class="col-progress"
              scope="col"
              aria-sort={sortKey === 'progress'
                ? sortDir === 'asc'
                  ? 'ascending'
                  : 'descending'
                : 'none'}
            >
              <button
                type="button"
                class="sort-btn"
                aria-label="Sort by progress"
                on:click={() => setSort('progress')}
              >
                Progress
                {#if sortKey === 'progress'}
                  <span aria-hidden="true" class="sort-arrow">
                    {#if sortDir === 'asc'}<ChevronUp size={13} />{:else}<ChevronDown size={13} />{/if}
                  </span>
                {/if}
              </button>
            </th>
            <th class="col-files" scope="col"><span class="sr-only">Files</span></th>
          </tr>
        </thead>
        <tbody>
          {#if visible.length === 0}
            <tr class="empty-row">
              <td colspan="5"><p class="empty">No anime found.</p></td>
            </tr>
          {:else}
            {#each visible as entry (entry.anime_id)}
              <tr
                class="data-row"
                tabindex="0"
                on:click={() => open(entry)}
                on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && open(entry)}
                on:contextmenu={(e) => openContextMenu(e, entry)}
              >
                <td>
                  {#if entry.image_url}
                    <img class="thumb" src={entry.image_url} alt="" width="24" height="24" loading="lazy" />
                  {:else}
                    <div class="thumb fallback" aria-hidden="true"></div>
                  {/if}
                </td>
                <td class="title-cell" class:has-new={entry.new_count > 0}>{entry.title}</td>
                <td><span class="badge">{formatStatus(entry.status)}</span></td>
                <td class="num-cell progress-cell" class:completed={fullyWatched(entry)}>
                  <div class="progress-wrap">
                    <div class="progress-bar" style="width: {watchPct(entry)}%"></div>
                    <div class="progress-inner">
                      <span class="progress-text">{entry.watched_episodes} / {totalLabel(entry)}</span>
                    </div>
                  </div>
                </td>
                <td class="col-files">
                  {#if entry.next_episode_path}
                    <button class="play-inline-btn" on:click|stopPropagation={() => playNext(entry)} title="Play next episode">&#9654;</button>
                  {/if}
                </td>
              </tr>
            {/each}
          {/if}
        </tbody>
      </table>
    </div>
  {/if}

  {#if ctxMenu}
    <div class="ctx-backdrop" on:click={closeContextMenu} on:contextmenu|preventDefault={closeContextMenu} role="presentation"></div>
    <div class="ctx-menu" style="left: {ctxMenu.x}px; top: {ctxMenu.y}px;" role="menu">
      <button class="ctx-item" role="menuitem" disabled={ctxNextEp() === null} on:click={() => playCtxEp(ctxNextEp())}>
        <Play size={13} /> Play next{#if ctxNextEp() !== null} <span class="ctx-dim">Ep {ctxNextEp()}</span>{/if}
      </button>
      <button class="ctx-item" role="menuitem" disabled={ctxPrevEp() === null} on:click={() => playCtxEp(ctxPrevEp())}>
        <ChevronLeft size={13} /> Play previous{#if ctxPrevEp() !== null} <span class="ctx-dim">Ep {ctxPrevEp()}</span>{/if}
      </button>

      <div class="ctx-sep"></div>
      <button class="ctx-item" role="menuitem" disabled={ctxFiles().length === 0} on:click={ctxOpenFolder}>
        <FolderOpen size={13} /> Open folder
      </button>
      <button class="ctx-item" role="menuitem" on:click={ctxOpenDetails}>
        <Info size={13} /> Full details
      </button>

      <div class="ctx-sub">
        <button class="ctx-item has-sub" role="menuitem">Set status <ChevronRight size={13} class="ctx-arrow" /></button>
        <div class="ctx-submenu">
          {#each STATUS_OPTIONS as opt (opt.value)}
            <button class="ctx-item" role="menuitem" on:click={() => ctxSetStatus(opt.value)}>{opt.label}</button>
          {/each}
        </div>
      </div>

      <div class="ctx-sep"></div>
      <button class="ctx-item danger" role="menuitem" on:click={ctxRemove}>
        <Trash2 size={13} /> Remove from library
      </button>
    </div>
  {/if}
</div>

<svelte:window on:keydown={(e) => e.key === 'Escape' && closeContextMenu()} />

<style>
  .collection-view {
    display: grid;
    gap: 1rem;
  }

  .lib-header {
    position: sticky;
    top: -1.5rem;
    z-index: 6;
    display: grid;
    gap: 0.75rem;
    padding: 1.5rem 0 0.5rem;
    margin: -1.5rem 0 -0.25rem;
    background: var(--color-bg, #0a0d14);
  }

  .controls {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
    align-items: center;
  }

  .search {
    border-radius: var(--radius-card);
    color: var(--color-text);
    padding: 0.6rem 0.9rem;
    font-family: var(--font-ui);
    font-size: 0.9rem;
    outline: none;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
    min-width: 16rem;
    flex: 1 1 16rem;
  }

  .search:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 3px rgba(var(--color-accent-rgb), 0.25);
  }

  .sort-btn {
    all: unset;
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    cursor: pointer;
    color: inherit;
    font: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
    border-radius: 6px;
    padding: 0.15rem 0.3rem;
    margin: -0.15rem -0.3rem;
  }
  .sort-btn:focus-visible {
    outline: 2px solid var(--color-accent);
    outline-offset: 2px;
  }
  .sort-arrow {
    display: inline-flex;
    align-items: center;
  }

  .status-tabs {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
    padding: 0.25rem 0;
  }

  .status-tab {
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
    border-radius: 999px;
    padding: 0.35rem 0.8rem;
    background: transparent;
    color: var(--color-muted);
    font-family: var(--font-ui);
    font-size: 0.82rem;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
    white-space: nowrap;
  }

  .status-tab:hover {
    background: rgba(var(--color-accent-rgb), 0.08);
    color: var(--color-text);
  }

  .status-tab.active {
    background: rgba(var(--color-accent-rgb), 0.18);
    color: var(--color-accent);
    border-color: rgba(var(--color-accent-rgb), 0.35);
  }

  .message {
    border: 1px solid rgba(var(--color-error-rgb), 0.35);
    border-radius: var(--radius-card);
    background: rgba(var(--color-error-rgb), 0.08);
    padding: 1rem 1.25rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .message p {
    margin: 0;
    color: var(--color-error);
  }

  .retry {
    border: 1px solid rgba(var(--color-error-rgb), 0.45);
    border-radius: 999px;
    background: rgba(var(--color-error-rgb), 0.14);
    color: var(--color-error);
    padding: 0.5rem 1rem;
    cursor: pointer;
    font-family: var(--font-ui);
    font-weight: 600;
    font-size: 0.85rem;
  }

  .retry:hover {
    background: rgba(var(--color-error-rgb), 0.22);
  }

  .empty {
    color: var(--color-muted);
    margin: 2.5rem 0;
    text-align: center;
  }

  .poster-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));
    gap: 1rem;
  }

  .poster-card {
    position: relative;
    border: 1px solid rgba(var(--color-accent-rgb), 0.1);
    border-radius: 10px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.03);
    cursor: pointer;
    transition: border-color 0.15s, transform 0.15s;
  }

  .poster-card:hover {
    border-color: rgba(var(--color-accent-rgb), 0.3);
    transform: translateY(-2px);
  }

  .poster-thumb {
    width: 100%;
    aspect-ratio: 3/4;
    object-fit: cover;
    display: block;
  }

  .poster-thumb.placeholder {
    background: rgba(var(--color-accent-rgb), 0.08);
  }

  .poster-info {
    padding: 0.6rem;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .poster-title {
    font-size: 0.85rem;
    font-weight: 600;
    line-height: 1.3;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .badge {
    display: inline-block;
    background: rgba(var(--color-accent-rgb), 0.12);
    color: var(--color-accent);
    border-radius: 999px;
    padding: 0.15rem 0.55rem;
    font-size: 0.75rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .new-badge {
    position: absolute;
    top: 0.4rem;
    right: 0.4rem;
    z-index: 2;
    background: rgba(var(--color-accent-rgb), 0.85);
    color: var(--color-bg, #0a0d14);
  }

  .play-next-btn {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: calc(0.6rem * 2 + 1.6rem);
    margin: auto;
    width: 2.75rem;
    height: 2.75rem;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.65);
    color: #fff;
    opacity: 0;
    pointer-events: none;
    cursor: pointer;
    transition: opacity 0.15s, background-color 0.15s;
    padding: 0;
  }

  .poster-card:hover .play-next-btn,
  .poster-card:focus-within .play-next-btn,
  .play-next-btn:focus-visible {
    opacity: 1;
    pointer-events: auto;
  }

  .play-next-btn:hover {
    background: rgba(var(--color-accent-rgb), 0.9);
  }

  .play-next-btn:focus-visible {
    outline: 2px solid var(--color-accent);
    outline-offset: 2px;
  }

  .ep-download-bar {
    display: flex;
    gap: 1px;
    height: 0.45rem;
    margin-top: 0.2rem;
    align-items: center;
  }

  .completeness-bar {
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.08);
    overflow: hidden;
  }

  .completeness-fill {
    height: 100%;
    border-radius: 3px;
    background: rgba(var(--color-warning-rgb), 0.55);
    transition: width 0.3s ease;
  }

  .completeness-fill.complete {
    background: rgba(var(--color-success-rgb), 0.6);
  }

  .skeleton-card {
    cursor: default;
  }

  .skeleton-card .poster-thumb.placeholder {
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }

  .skeleton-line {
    height: 0.9rem;
    width: 80%;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }

  .skeleton-line.short {
    width: 40%;
    height: 0.6rem;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .ctx-backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
  }

  .ctx-menu {
    position: fixed;
    z-index: 41;
    min-width: 11rem;
    max-width: 16rem;
    background: rgba(16, 21, 32, 0.98);
    border: 1px solid rgba(var(--color-accent-rgb), 0.25);
    border-radius: 10px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    padding: 0.35rem;
    display: grid;
    gap: 0.15rem;
  }

  .ctx-item {
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

  .ctx-item:hover:not(:disabled) {
    background: rgba(var(--color-accent-rgb), 0.15);
  }

  .ctx-item:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .ctx-item.danger { color: var(--color-danger-text); }
  .ctx-item.danger:hover:not(:disabled) { background: rgba(var(--color-danger-rgb), 0.15); }

  .ctx-dim { color: var(--color-muted); font-size: 0.75rem; }

  .ctx-sep {
    height: 1px;
    background: rgba(var(--color-accent-rgb), 0.15);
    margin: 0.25rem 0.3rem;
  }

  .ctx-sub { position: relative; }

  .ctx-submenu {
    display: none;
    position: absolute;
    left: 100%;
    top: 0;
    margin-left: 2px;
    min-width: 8rem;
    max-height: 18rem;
    overflow-y: auto;
    background: rgba(16, 21, 32, 0.99);
    border: 1px solid rgba(var(--color-accent-rgb), 0.25);
    border-radius: 10px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    padding: 0.35rem;
    gap: 0.1rem;
  }

  .ctx-sub:hover .ctx-submenu,
  .ctx-submenu:hover {
    display: grid;
  }

  /* .lib-header's sticky offset cancels out .content's padding (App.svelte) so the
     header sits flush with the viewport edge instead of leaving a gap above it. */
  @media (max-width: 768px) {
    .lib-header {
      top: -1rem;
      padding: 1rem 0 0.5rem;
      margin: -1rem 0 -0.25rem;
    }
  }

  /* ===== Table-view styles (ported from LibraryView) ===== */
  .view-toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
    border-radius: 999px;
    padding: 0.45rem 0.8rem;
    background: rgba(var(--color-accent-rgb), 0.06);
    color: var(--color-muted);
    cursor: pointer;
    font-size: 0.82rem;
    white-space: nowrap;
  }
  .view-toggle:hover {
    background: rgba(var(--color-accent-rgb), 0.15);
    color: var(--color-text);
  }
  .table-wrap {
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
    border-radius: var(--radius-card);
    background: rgba(255, 255, 255, 0.04);
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    min-width: 640px;
    font-size: 0.9rem;
  }
  thead th {
    text-align: left;
    padding: 0.75rem 1rem;
    color: var(--color-muted);
    font-weight: 600;
    font-size: 0.78rem;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    border-bottom: 1px solid rgba(var(--color-accent-rgb), 0.18);
    white-space: nowrap;
  }
  tbody td {
    padding: 0.6rem 1rem;
    vertical-align: middle;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }
  table.compact { font-size: 0.82rem; }
  table.compact thead th { padding: 0.4rem 0.7rem; }
  table.compact tbody td { padding: 0.12rem 0.7rem; }
  table.compact .thumb,
  table.compact .thumb.fallback { width: 18px; height: 18px; }
  table.compact .badge { padding: 0.04rem 0.4rem; font-size: 0.66rem; }
  table.compact .progress-wrap { height: 1.15rem; }
  table.compact .progress-text { font-size: 0.72rem; min-width: 2.4rem; }
  table.compact .play-inline-btn { padding: 0.05rem 0.35rem; }
  .data-row {
    cursor: pointer;
    transition: background 0.15s ease;
  }
  .data-row:hover,
  .data-row:focus {
    background: rgba(var(--color-accent-rgb), 0.08);
  }
  .data-row:focus-visible {
    outline: 2px solid var(--color-accent);
    outline-offset: -2px;
  }
  .thumb {
    width: 24px;
    height: 24px;
    border-radius: 6px;
    object-fit: cover;
    display: block;
    background: rgba(255, 255, 255, 0.08);
  }
  .thumb.fallback {
    background: rgba(255, 255, 255, 0.12);
  }
  .title-cell {
    font-weight: 500;
    color: var(--color-text);
    max-width: 20rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title-cell.has-new {
    color: var(--color-accent);
    font-weight: 600;
  }
  .num-cell {
    font-variant-numeric: tabular-nums;
    color: var(--color-muted);
  }
  .progress-cell {
    white-space: nowrap;
  }
  .progress-wrap {
    position: relative;
    width: 100%;
    height: 1.55rem;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.06);
    overflow: hidden;
  }
  .progress-bar {
    position: absolute;
    left: 0;
    top: 0;
    height: 100%;
    background: rgba(var(--color-accent-rgb), 0.25);
    border-radius: 4px;
    transition: width 0.3s ease;
  }
  .progress-cell.completed .progress-bar {
    background: rgba(var(--color-success-rgb), 0.25);
  }
  .progress-inner {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    height: 100%;
    padding: 0 0.3rem;
  }
  .progress-text {
    font-size: 0.8rem;
    font-weight: 500;
    min-width: 3rem;
    text-align: center;
  }
  .col-thumb {
    width: 2.2rem;
  }
  .col-files {
    width: 2rem;
    text-align: center;
  }
  .play-inline-btn {
    border: none;
    background: rgba(var(--color-accent-rgb), 0.15);
    color: var(--color-accent);
    cursor: pointer;
    border-radius: 4px;
    padding: 0.1rem 0.4rem;
    font-size: 0.75rem;
    line-height: 1.4;
  }
  .play-inline-btn:hover {
    background: rgba(var(--color-accent-rgb), 0.3);
  }
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
  .empty-row td {
    text-align: center;
    padding: 2.5rem 1rem;
  }
  table .empty {
    margin: 0;
  }
</style>
