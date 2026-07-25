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
    sortCollection,
    isComplete,
    type CollectionFilter,
    type CollectionSort,
  } from './collectionUi';
  import { Play, FolderOpen, Info, Trash2, ChevronRight, ChevronLeft } from 'lucide-svelte';

  export let events: EngineEvent[] = [];
  const dispatch = createEventDispatcher<{ select: { anime_id: number } }>();

  function loadPref(k: string, f: string) { try { return localStorage.getItem(k) ?? f; } catch { return f; } }
  function persistPref(k: string, v: string) { try { localStorage.setItem(k, v); } catch {} }

  let entries: CollectionEntry[] = [];
  let loading = false;
  let error = '';
  let query = loadPref('anivault-collection-query', '');
  let filter = loadPref('anivault-collection-filter', 'all') as CollectionFilter;
  let sort = loadPref('anivault-collection-sort', 'recent') as CollectionSort;
  let episodeFilesMap = new Map<number, FileIndexEntry[]>();

  $: persistPref('anivault-collection-query', query);
  $: persistPref('anivault-collection-filter', filter);
  $: persistPref('anivault-collection-sort', sort);
  $: visible = sortCollection(filterCollection(entries, filter, query), sort);

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
    try {
      entries = await getCollection();
      for (const e of entries.slice(0, 100)) {
        try {
          const files = await getEpisodeFiles(e.anime_id);
          if (files.length > 0) episodeFilesMap.set(e.anime_id, files);
        } catch {}
      }
      episodeFilesMap = new Map(episodeFilesMap);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally { loading = false; }
  }

  function completeness(e: CollectionEntry): number {
    if (e.episode_count && e.episode_count > 0) return Math.min(100, (e.max_downloaded_episode / e.episode_count) * 100);
    return 100;
  }

  function playNext(e: CollectionEntry) {
    const files = episodeFilesMap.get(e.anime_id);
    if (!files) return;
    const ep = e.next_unwatched_episode ?? e.max_downloaded_episode;
    const f = files.find((x) => (x.episode ?? 0) === ep) ?? files[0];
    if (f) openEpisodeFile(f.file_path);
  }

  function open(e: CollectionEntry) { dispatch('select', { anime_id: e.anime_id }); }

  // Reload when the file index or progress changes (mirrors LibraryView).
  $: if (events && events.some((ev) => 'LibraryUpdated' in ev || 'ProgressAdvanced' in ev)) void load();

  // Right-click context menu.
  let ctxMenu: { x: number; y: number; entry: CollectionEntry } | null = null;

  function openContextMenu(e: MouseEvent, entry: CollectionEntry) {
    e.preventDefault();
    ctxMenu = { x: Math.min(e.clientX, window.innerWidth - 240), y: Math.min(e.clientY, window.innerHeight - 320), entry };
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
    if (!ctxMenu) return null;
    const files = ctxFiles();
    const f = files.find((x) => x.ep === ctxMenu!.entry.watched_episodes + 1)
      ?? files.find((x) => x.ep > ctxMenu!.entry.watched_episodes);
    return f ? f.ep : null;
  }
  function ctxPrevEp(): number | null {
    if (!ctxMenu) return null;
    const want = ctxMenu.entry.watched_episodes;
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
      <select class="sort-select" bind:value={sort} aria-label="Sort collection">
        <option value="recent">Recently added</option>
        <option value="title">Title</option>
        <option value="progress">Progress</option>
      </select>
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
          <div class="poster-thumb placeholder" />
          <div class="poster-info">
            <div class="skeleton-line"></div>
            <div class="skeleton-line short"></div>
          </div>
        </div>
      {/each}
    </div>
  {:else if visible.length === 0}
    <p class="empty">No downloaded series yet.</p>
  {:else}
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
            <div class="poster-thumb placeholder" />
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
              />
            </div>
          </div>
        </div>
      {/each}
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

  .sort-select {
    border-radius: var(--radius-card);
    color: var(--color-text);
    padding: 0.55rem 0.8rem;
    font-family: var(--font-ui);
    font-size: 0.85rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
    cursor: pointer;
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

  .poster-card:hover .play-next-btn {
    opacity: 1;
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
    inset: 0;
    bottom: calc(0.6rem * 2 + 1.6rem);
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: linear-gradient(to bottom, rgba(0, 0, 0, 0.05), rgba(0, 0, 0, 0.55));
    color: #fff;
    opacity: 0;
    cursor: pointer;
    transition: opacity 0.15s;
    padding: 0;
  }

  .play-next-btn:focus-visible {
    opacity: 1;
    outline: 2px solid var(--color-accent);
    outline-offset: -2px;
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
</style>
