<script lang="ts">
  import { onMount } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { searchLibrary, updateListEntry, type LibraryEntry } from './api';

  const dispatch = createEventDispatcher<{ select: { anime_id: number } }>();

  let query = '';
  let statusFilter: string | null = null;
  let entries: LibraryEntry[] = [];
  let loading = false;
  let error = '';

  let sortKey: 'title' | 'status' | 'progress' | 'score' = 'title';
  let sortDir: 'asc' | 'desc' = 'asc';
  let viewMode: 'table' | 'grid' = 'table';

  let debounceTimer: ReturnType<typeof setTimeout>;

  const statusOptions = [
    { value: null, label: 'All' },
    { value: 'watching', label: 'Watching' },
    { value: 'completed', label: 'Completed' },
    { value: 'on_hold', label: 'On Hold' },
    { value: 'dropped', label: 'Dropped' },
    { value: 'plan_to_watch', label: 'Plan to Watch' },
    { value: 'unlisted', label: 'Unlisted' },
  ];

  function formatStatus(status: string) {
    return status.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
  }

  async function load() {
    loading = true;
    error = '';
    try {
      const results = await searchLibrary(query, statusFilter, 200, 0);
      entries = results;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function debouncedReload() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      void load();
    }, 300);
  }

  function setSort(key: 'title' | 'status' | 'progress' | 'score') {
    if (sortKey === key) {
      sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    } else {
      sortKey = key;
      sortDir = 'asc';
    }
  }

  let progressUpdating = new Set<number>();

  let dragEntry: LibraryEntry | null = null;

  function handleDragStart(e: DragEvent, entry: LibraryEntry) {
    dragEntry = entry;
    e.dataTransfer!.effectAllowed = 'move';
  }

  async function handleDrop(newStatus: string | null) {
    if (!dragEntry || !newStatus || dragEntry.status === newStatus) return;
    const entry = dragEntry;
    dragEntry = null;
    try {
      await updateListEntry(entry.anime_id, { status: newStatus });
      entry.status = newStatus;
    } catch (e) {
      // revert on next reload
    }
  }

  async function handleIncrement(entry: LibraryEntry) {
    if (progressUpdating.has(entry.anime_id)) return;
    const newEp = entry.watched_episodes + 1;
    if (entry.episode_count && newEp > entry.episode_count) return;
    progressUpdating.add(entry.anime_id);
    try {
      await updateListEntry(entry.anime_id, { watched_episodes: newEp });
      entry.watched_episodes = newEp;
    } catch (e) {
      // revert on error handled by refresh
    } finally {
      progressUpdating.delete(entry.anime_id);
    }
  }

  async function handleDecrement(entry: LibraryEntry) {
    if (progressUpdating.has(entry.anime_id)) return;
    const newEp = Math.max(0, entry.watched_episodes - 1);
    progressUpdating.add(entry.anime_id);
    try {
      await updateListEntry(entry.anime_id, { watched_episodes: newEp });
      entry.watched_episodes = newEp;
    } catch (e) {
      // revert on error
    } finally {
      progressUpdating.delete(entry.anime_id);
    }
  }

  let selectedIds = new Set<number>();
  let allSelected = false;
  let batchUpdating = false;

  function toggleSelectAll() {
    if (allSelected) {
      selectedIds.clear();
    } else {
      sortedEntries.forEach(e => selectedIds.add(e.anime_id));
    }
    allSelected = !allSelected;
    selectedIds = new Set(selectedIds);
  }

  function toggleSelect(animeId: number) {
    if (selectedIds.has(animeId)) { selectedIds.delete(animeId); }
    else { selectedIds.add(animeId); }
    allSelected = sortedEntries.length > 0 && selectedIds.size === sortedEntries.length;
    selectedIds = new Set(selectedIds);
  }

  function batchSetStatus(status: string) {
    return async () => {
      if (batchUpdating) return;
      batchUpdating = true;
      for (const id of selectedIds) {
        try {
          await updateListEntry(id, { status });
          const entry = entries.find(e => e.anime_id === id);
          if (entry) entry.status = status;
        } catch { /* continue */ }
      }
      selectedIds.clear(); allSelected = false;
      selectedIds = new Set(selectedIds);
      batchUpdating = false;
    };
  }

  async function batchIncrementProgress() {
    if (batchUpdating) return;
    batchUpdating = true;
    for (const id of selectedIds) {
      const entry = entries.find(e => e.anime_id === id);
      if (!entry) continue;
      const newEp = entry.watched_episodes + 1;
      if (entry.episode_count && newEp > entry.episode_count) continue;
      try {
        await updateListEntry(id, { watched_episodes: newEp });
        entry.watched_episodes = newEp;
      } catch { /* continue */ }
    }
    selectedIds.clear(); allSelected = false;
    selectedIds = new Set(selectedIds);
    batchUpdating = false;
  }

  function handleRowActivate(entry: LibraryEntry) {
    dispatch('select', { anime_id: entry.anime_id });
  }

  function onRowKeydown(e: KeyboardEvent, entry: LibraryEntry) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      handleRowActivate(entry);
    }
  }

  $: sortedEntries = (() => {
    const list = [...entries];
    const dir = sortDir === 'asc' ? 1 : -1;
    list.sort((a, b) => {
      let cmp = 0;
      switch (sortKey) {
        case 'title':
          cmp = a.title.localeCompare(b.title);
          break;
        case 'status':
          cmp = a.status.localeCompare(b.status);
          break;
        case 'progress': {
          const pa = a.watched_episodes ?? 0;
          const pb = b.watched_episodes ?? 0;
          cmp = pa - pb;
          break;
        }
        case 'score': {
          const sa = a.score ?? -1;
          const sb = b.score ?? -1;
          cmp = sa - sb;
          break;
        }
      }
      return cmp * dir;
    });
    return list;
  })();

  onMount(() => {
    void load();
  });
</script>

<div class="library-view">
  <div class="controls">
    <input
      type="text"
      class="search"
      placeholder="Search library…"
      bind:value={query}
      on:input={debouncedReload}
      aria-label="Search library"
    />
    <button class="view-toggle" on:click={() => viewMode = viewMode === 'table' ? 'grid' : 'table'} aria-label="Toggle view">
      {viewMode === 'table' ? '▦ Grid' : '☰ Table'}
    </button>
  </div>
  <div class="status-tabs">
    {#each statusOptions as opt}
      <button
        type="button"
        class="status-tab"
        class:active={statusFilter === opt.value}
        class:dragover={dragEntry !== null && dragEntry.status !== opt.value}
        on:dragover={(e) => { e.preventDefault(); }}
        on:drop={() => handleDrop(opt.value)}
        on:click={() => { statusFilter = opt.value; load(); }}
      >
        {opt.label}
      </button>
    {/each}
  </div>

  {#if error}
    <div class="message error" role="alert">
      <p>{error}</p>
      <button type="button" class="retry" on:click={load}>Retry</button>
    </div>
  {/if}

  {#if viewMode === 'grid'}
    <div class="poster-grid">
      {#each sortedEntries as entry (entry.anime_id)}
        <div class="poster-card"
          tabindex="0"
          role="button"
          aria-label={`${entry.title}, ${entry.status}`}
          on:click={() => handleRowActivate(entry)}
          on:keydown={(e) => e.key === 'Enter' && handleRowActivate(entry)}
        >
          <div class="poster-check">
            <input type="checkbox" checked={selectedIds.has(entry.anime_id)} on:change={() => toggleSelect(entry.anime_id)} on:click|stopPropagation aria-label={`Select ${entry.title}`} />
          </div>
          {#if entry.image_url}
            <img class="poster-thumb" src={entry.image_url} alt={entry.title} loading="lazy" />
          {:else}
            <div class="poster-thumb placeholder" />
          {/if}
          <div class="poster-info">
            <p class="poster-title">{entry.title}</p>
            <span class="badge">{formatStatus(entry.status)}</span>
            <div class="progress-wrap poster-progress">
              <div class="progress-bar" style="width: {entry.episode_count ? (entry.watched_episodes / entry.episode_count * 100) : 0}%" />
              <div class="progress-inner">
                <span class="progress-text">{entry.watched_episodes} / {entry.episode_count ?? '?'}</span>
              </div>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th class="col-check" scope="col">
              <input type="checkbox" on:change={toggleSelectAll} checked={allSelected} aria-label="Select all" />
            </th>
            <th class="col-thumb" scope="col">
              <span class="sr-only">Thumbnail</span>
            </th>
            <th class="col-title" scope="col">
              <button
                type="button"
                class="sort-btn"
                aria-label="Sort by title"
                aria-sort={sortKey === 'title'
                  ? sortDir === 'asc'
                    ? 'ascending'
                    : 'descending'
                  : 'none'}
                on:click={() => setSort('title')}
              >
                Title
                {#if sortKey === 'title'}
                  <span aria-hidden="true">{sortDir === 'asc' ? '▲' : '▼'}</span>
                {/if}
              </button>
            </th>
            <th class="col-status" scope="col">
              <button
                type="button"
                class="sort-btn"
                aria-label="Sort by status"
                aria-sort={sortKey === 'status'
                  ? sortDir === 'asc'
                    ? 'ascending'
                    : 'descending'
                  : 'none'}
                on:click={() => setSort('status')}
              >
                Status
                {#if sortKey === 'status'}
                  <span aria-hidden="true">{sortDir === 'asc' ? '▲' : '▼'}</span>
                {/if}
              </button>
            </th>
            <th class="col-progress" scope="col">
              <button
                type="button"
                class="sort-btn"
                aria-label="Sort by progress"
                aria-sort={sortKey === 'progress'
                  ? sortDir === 'asc'
                    ? 'ascending'
                    : 'descending'
                  : 'none'}
                on:click={() => setSort('progress')}
              >
                Progress
                {#if sortKey === 'progress'}
                  <span aria-hidden="true">{sortDir === 'asc' ? '▲' : '▼'}</span>
                {/if}
              </button>
            </th>
            <th class="col-score" scope="col">
              <button
                type="button"
                class="sort-btn"
                aria-label="Sort by score"
                aria-sort={sortKey === 'score'
                  ? sortDir === 'asc'
                    ? 'ascending'
                    : 'descending'
                  : 'none'}
                on:click={() => setSort('score')}
              >
                Score
                {#if sortKey === 'score'}
                  <span aria-hidden="true">{sortDir === 'asc' ? '▲' : '▼'}</span>
                {/if}
              </button>
            </th>
          </tr>
        </thead>
        <tbody>
          {#if loading}
            {#each Array.from({ length: 5 }) as _, i (i)}
              <tr class="skeleton-row">
                <td></td>
                <td><div class="skeleton-thumb"></div></td>
                <td><div class="skeleton-line"></div></td>
                <td><div class="skeleton-badge"></div></td>
                <td><div class="skeleton-line short"></div></td>
                <td><div class="skeleton-line short"></div></td>
              </tr>
            {/each}
          {:else if sortedEntries.length === 0}
            <tr class="empty-row">
              <td colspan="6">
                <p class="empty">No anime found.</p>
              </td>
            </tr>
          {:else}
            {#each sortedEntries as entry (entry.anime_id)}
              <tr
                class="data-row"
                draggable="true"
                tabindex="0"
                on:click={() => handleRowActivate(entry)}
                on:keydown={(e) => onRowKeydown(e, entry)}
                on:dragstart={(e) => handleDragStart(e, entry)}
                on:dragend={() => dragEntry = null}
              >
                <td class="col-check">
                  <input type="checkbox" checked={selectedIds.has(entry.anime_id)} on:change={() => toggleSelect(entry.anime_id)} on:click|stopPropagation />
                </td>
                <td>
                  {#if entry.image_url}
                    <img
                      class="thumb"
                      src={entry.image_url}
                      alt=""
                      width="24"
                      height="24"
                      loading="lazy"
                    />
                  {:else}
                    <div class="thumb fallback" aria-hidden="true"></div>
                  {/if}
                </td>
                <td class="title-cell">{entry.title}</td>
                <td>
                  <span class="badge">{formatStatus(entry.status)}</span>
                </td>
                <td class="num-cell progress-cell" class:completed={entry.watched_episodes > 0 && entry.episode_count != null && entry.watched_episodes >= entry.episode_count}>
                  <div class="progress-wrap">
                    <div class="progress-bar" style="width: {entry.episode_count ? (entry.watched_episodes / entry.episode_count * 100) : 0}%" />
                    <div class="progress-inner">
                      <button class="progress-btn" on:click|stopPropagation={() => handleDecrement(entry)} aria-label="Decrease">&minus;</button>
                      <span class="progress-text">{entry.watched_episodes} / {entry.episode_count ?? '?'}</span>
                      <button class="progress-btn" on:click|stopPropagation={() => handleIncrement(entry)} aria-label="Increase">+</button>
                    </div>
                  </div>
                </td>
                <td class="num-cell">
                  {entry.score ?? '-'}
                </td>
              </tr>
            {/each}
          {/if}
        </tbody>
      </table>
    </div>
    {#if selectedIds.size > 0}
      <div class="batch-bar">
        <span class="batch-count">{selectedIds.size} selected</span>
        <button class="action-btn" on:click={batchSetStatus('watching')}>Watching</button>
        <button class="action-btn" on:click={batchSetStatus('completed')}>Completed</button>
        <button class="action-btn" on:click={batchSetStatus('on_hold')}>On Hold</button>
        <button class="action-btn" on:click={batchSetStatus('dropped')}>Dropped</button>
        <button class="action-btn" on:click={batchSetStatus('plan_to_watch')}>Plan to Watch</button>
        <button class="action-btn" on:click={batchIncrementProgress}>+1 Ep</button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .library-view {
    display: grid;
    gap: 1rem;
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
    border: 1px solid rgba(143, 183, 255, 0.18);
    min-width: 16rem;
    flex: 1 1 16rem;
  }

  .search:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 3px rgba(143, 183, 255, 0.25);
  }

  .status-tabs {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
    padding: 0.25rem 0;
  }

  .status-tab {
    border: 1px solid rgba(143, 183, 255, 0.18);
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
    background: rgba(143, 183, 255, 0.08);
    color: var(--color-text);
  }

  .status-tab.active {
    background: rgba(143, 183, 255, 0.18);
    color: var(--color-accent);
    border-color: rgba(143, 183, 255, 0.35);
  }

  .status-tab.dragover {
    background: rgba(143, 183, 255, 0.25);
    border-color: var(--color-accent);
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
    background: rgba(143, 183, 255, 0.25);
    border-radius: 4px;
    transition: width 0.3s ease;
  }

  .progress-cell.completed .progress-bar {
    background: rgba(126, 232, 126, 0.25);
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

  .progress-btn {
    border: 1px solid rgba(143, 183, 255, 0.2);
    border-radius: 4px;
    background: rgba(143, 183, 255, 0.06);
    color: var(--color-muted);
    width: 1.4rem;
    height: 1.4rem;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    font-size: 0.85rem;
    line-height: 1;
    padding: 0;
    transition: background 0.12s, color 0.12s;
  }

  .progress-btn:hover {
    background: rgba(143, 183, 255, 0.2);
    color: var(--color-text);
  }

  .table-wrap {
    border: 1px solid rgba(143, 183, 255, 0.18);
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
    border-bottom: 1px solid rgba(143, 183, 255, 0.18);
    white-space: nowrap;
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

  tbody td {
    padding: 0.6rem 1rem;
    vertical-align: middle;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .data-row {
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .data-row:hover,
  .data-row:focus {
    background: rgba(143, 183, 255, 0.08);
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

  .badge {
    display: inline-block;
    background: rgba(143, 183, 255, 0.12);
    color: var(--color-accent);
    border-radius: 999px;
    padding: 0.15rem 0.55rem;
    font-size: 0.75rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .num-cell {
    font-variant-numeric: tabular-nums;
    color: var(--color-muted);
  }

  .empty-row td {
    text-align: center;
    padding: 2.5rem 1rem;
  }

  .empty {
    color: var(--color-muted);
    margin: 0;
  }

  .message {
    border: 1px solid rgba(255, 157, 157, 0.35);
    border-radius: var(--radius-card);
    background: rgba(255, 157, 157, 0.08);
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
    border: 1px solid rgba(255, 157, 157, 0.45);
    border-radius: 999px;
    background: rgba(255, 157, 157, 0.14);
    color: var(--color-error);
    padding: 0.5rem 1rem;
    cursor: pointer;
    font-family: var(--font-ui);
    font-weight: 600;
    font-size: 0.85rem;
  }

  .retry:hover {
    background: rgba(255, 157, 157, 0.22);
  }

  .skeleton-row td {
    padding: 0.7rem 1rem;
  }

  .skeleton-thumb,
  .skeleton-line,
  .skeleton-badge {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }

  .skeleton-thumb {
    width: 24px;
    height: 24px;
  }

  .skeleton-line {
    height: 0.9rem;
    width: 60%;
  }

  .skeleton-line.short {
    width: 30%;
  }

  .skeleton-badge {
    height: 1.1rem;
    width: 4rem;
    border-radius: 999px;
  }

  .view-toggle {
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: 999px;
    padding: 0.45rem 0.8rem;
    background: rgba(143, 183, 255, 0.06);
    color: var(--color-muted);
    cursor: pointer;
    font-size: 0.82rem;
    white-space: nowrap;
  }

  .view-toggle:hover {
    background: rgba(143, 183, 255, 0.15);
    color: var(--color-text);
  }

  .poster-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));
    gap: 1rem;
  }

  .poster-card {
    position: relative;
    border: 1px solid rgba(143, 183, 255, 0.1);
    border-radius: 10px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.03);
    cursor: pointer;
    transition: border-color 0.15s, transform 0.15s;
  }

  .poster-card:hover {
    border-color: rgba(143, 183, 255, 0.3);
    transform: translateY(-2px);
  }

  .poster-thumb {
    width: 100%;
    aspect-ratio: 3/4;
    object-fit: cover;
    display: block;
  }

  .poster-thumb.placeholder {
    background: rgba(143, 183, 255, 0.08);
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

  .poster-progress {
    height: 1.2rem;
  }

  .poster-progress .progress-text {
    font-size: 0.72rem;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.4;
    }
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

  .col-check {
    width: 2rem;
    text-align: center;
  }

  .col-check input {
    accent-color: var(--color-accent);
  }

  .batch-bar {
    position: sticky;
    bottom: 0;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 1rem;
    background: rgba(10,13,20,0.95);
    border: 1px solid rgba(143,183,255,0.2);
    border-radius: 10px;
    margin-top: 0.5rem;
    backdrop-filter: blur(8px);
    z-index: 10;
  }

  .batch-count {
    font-size: 0.82rem;
    color: var(--color-muted);
    margin-right: 0.5rem;
  }

  .action-btn {
    border: 1px solid rgba(143, 183, 255, 0.2);
    border-radius: 999px;
    padding: 0.35rem 0.7rem;
    background: rgba(143, 183, 255, 0.08);
    color: var(--color-text);
    font-family: var(--font-ui);
    font-size: 0.78rem;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.12s;
  }

  .action-btn:hover {
    background: rgba(143, 183, 255, 0.2);
  }

  .poster-check {
    position: absolute;
    top: 0.35rem;
    left: 0.35rem;
    z-index: 2;
  }

  .poster-check input {
    accent-color: var(--color-accent);
  }
</style>
