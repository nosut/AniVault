<script lang="ts">
  import { onMount, createEventDispatcher } from 'svelte';
  import { searchLibrary, type LibraryEntry } from './api';

  const dispatch = createEventDispatcher<{ onSelect: { anime_id: number } }>();

  let query = '';
  let statusFilter: string | null = null;
  let entries: LibraryEntry[] = [];
  let loading = false;
  let error: string | null = null;

  let sortKey: 'title' | 'status' | 'watched_episodes' | 'score' = 'title';
  let sortDir: 'asc' | 'desc' = 'asc';

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  const statusOptions = [
    { value: null, label: 'All' },
    { value: 'watching', label: 'Watching' },
    { value: 'completed', label: 'Completed' },
    { value: 'on_hold', label: 'On Hold' },
    { value: 'dropped', label: 'Dropped' },
    { value: 'plan_to_watch', label: 'Plan to Watch' },
  ];

  const statusColors: Record<string, string> = {
    watching: '#8fb7ff',
    completed: '#7dd3a8',
    on_hold: '#ffc164',
    dropped: '#ff9d9d',
    plan_to_watch: '#c9a8ff',
  };

  function statusLabel(status: string): string {
    const map: Record<string, string> = {
      watching: 'Watching',
      completed: 'Completed',
      on_hold: 'On Hold',
      dropped: 'Dropped',
      plan_to_watch: 'Plan to Watch',
    };
    return map[status] ?? status;
  }

  function handleSort(key: typeof sortKey) {
    if (sortKey === key) {
      sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    } else {
      sortKey = key;
      sortDir = 'asc';
    }
  }

  function sortEntries(list: LibraryEntry[]): LibraryEntry[] {
    return [...list].sort((a, b) => {
      let av: string | number | null;
      let bv: string | number | null;

      switch (sortKey) {
        case 'title':
          av = a.title.toLowerCase();
          bv = b.title.toLowerCase();
          break;
        case 'status':
          av = a.status;
          bv = b.status;
          break;
        case 'watched_episodes':
          av = a.watched_episodes ?? 0;
          bv = b.watched_episodes ?? 0;
          break;
        case 'score':
          av = a.score ?? 0;
          bv = b.score ?? 0;
          break;
        default:
          return 0;
      }

      if (av === bv) return 0;
      if (av == null) return sortDir === 'asc' ? -1 : 1;
      if (bv == null) return sortDir === 'asc' ? 1 : -1;
      if (typeof av === 'string' && typeof bv === 'string') {
        return sortDir === 'asc' ? av.localeCompare(bv) : bv.localeCompare(av);
      }
      return sortDir === 'asc' ? (av as number) - (bv as number) : (bv as number) - (av as number);
    });
  }

  async function load() {
    loading = true;
    error = null;
    try {
      entries = await searchLibrary(query, statusFilter, 200, 0);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function scheduleLoad() {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      void load();
    }, 300);
  }

  $: {
    // reactive dependency on query and statusFilter
    void query;
    void statusFilter;
    scheduleLoad();
  }

  onMount(() => {
    void load();
    return () => {
      if (debounceTimer) clearTimeout(debounceTimer);
    };
  });

  $: sortedEntries = sortEntries(entries);
</script>

<section class="library-view" aria-label="Anime library">
  <div class="toolbar">
    <input
      type="text"
      class="search-input"
      placeholder="Search anime…"
      bind:value={query}
      aria-label="Search"
    />
    <select class="status-select" bind:value={statusFilter} aria-label="Filter by status">
      {#each statusOptions as opt}
        <option value={opt.value}>{opt.label}</option>
      {/each}
    </select>
  </div>

  {#if error}
    <div class="error-box">
      <p class="error">{error}</p>
      <button type="button" class="retry-btn" on:click={load}>Retry</button>
    </div>
  {:else if loading}
    <div class="table-wrap">
      <table class="library-table" aria-label="Anime library loading">
        <thead>
          <tr>
            <th class="col-thumb" scope="col"></th>
            <th class="col-title" scope="col">Title</th>
            <th class="col-status" scope="col">Status</th>
            <th class="col-progress" scope="col">Progress</th>
            <th class="col-score" scope="col">Score</th>
          </tr>
        </thead>
        <tbody>
          {#each Array(6) as _, i}
            <tr class="skeleton-row" aria-hidden="true">
              <td><div class="skeleton-thumb"></div></td>
              <td><div class="skeleton-line" style="width: 60%"></div></td>
              <td><div class="skeleton-pill"></div></td>
              <td><div class="skeleton-line" style="width: 40%"></div></td>
              <td><div class="skeleton-line" style="width: 2rem"></div></td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else if sortedEntries.length === 0}
    <div class="empty-state">
      <p class="empty-title">No anime found</p>
      <p class="empty-hint">Connect AniList to import your library.</p>
    </div>
  {:else}
    <div class="table-wrap">
      <table class="library-table" aria-label="Anime library">
        <thead>
          <tr>
            <th class="col-thumb" scope="col"></th>
            <th
              class="col-title sortable"
              scope="col"
              class:active={sortKey === 'title'}
              on:click={() => handleSort('title')}
            >
              Title {sortKey === 'title' ? (sortDir === 'asc' ? '▲' : '▼') : ''}
            </th>
            <th
              class="col-status sortable"
              scope="col"
              class:active={sortKey === 'status'}
              on:click={() => handleSort('status')}
            >
              Status {sortKey === 'status' ? (sortDir === 'asc' ? '▲' : '▼') : ''}
            </th>
            <th
              class="col-progress sortable"
              scope="col"
              class:active={sortKey === 'watched_episodes'}
              on:click={() => handleSort('watched_episodes')}
            >
              Progress {sortKey === 'watched_episodes' ? (sortDir === 'asc' ? '▲' : '▼') : ''}
            </th>
            <th
              class="col-score sortable"
              scope="col"
              class:active={sortKey === 'score'}
              on:click={() => handleSort('score')}
            >
              Score {sortKey === 'score' ? (sortDir === 'asc' ? '▲' : '▼') : ''}
            </th>
          </tr>
        </thead>
        <tbody>
          {#each sortedEntries as entry}
            <tr
              class="data-row"
              on:click={() => dispatch('onSelect', { anime_id: entry.anime_id })}
              role="button"
              tabindex="0"
              on:keydown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  dispatch('onSelect', { anime_id: entry.anime_id });
                }
              }}
            >
              <td class="cell-thumb">
                {#if entry.image_url}
                  <img src={entry.image_url} alt="" class="thumb" width="24" height="24" loading="lazy" />
                {:else}
                  <div class="thumb-placeholder" aria-hidden="true"></div>
                {/if}
              </td>
              <td class="cell-title">{entry.title}</td>
              <td class="cell-status">
                <span
                  class="status-pill"
                  style="background: {statusColors[entry.status] ?? 'rgba(255,255,255,0.1)'}; color: #080a0f;"
                >
                  {statusLabel(entry.status)}
                </span>
              </td>
              <td class="cell-progress">
                {entry.watched_episodes ?? 0} / {entry.episode_count ?? '?'}
              </td>
              <td class="cell-score">
                {entry.score ?? '—'}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

<style>
  .library-view {
    display: grid;
    gap: 1rem;
  }

  .toolbar {
    display: flex;
    gap: 0.75rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .search-input {
    flex: 1 1 12rem;
    min-width: 8rem;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 999px;
    padding: 0.55rem 1rem;
    color: var(--color-text);
    font-size: 0.85rem;
  }

  .search-input::placeholder {
    color: var(--color-muted);
  }

  .search-input:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .status-select {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 999px;
    padding: 0.55rem 1rem;
    color: var(--color-text);
    font-size: 0.85rem;
    cursor: pointer;
  }

  .status-select:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .table-wrap {
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: var(--radius-card);
    background: rgba(255, 255, 255, 0.04);
    overflow: hidden;
  }

  .library-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }

  .library-table th,
  .library-table td {
    padding: 0.65rem 0.9rem;
    text-align: left;
    vertical-align: middle;
  }

  .library-table thead th {
    color: var(--color-muted);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-size: 0.72rem;
    border-bottom: 1px solid rgba(143, 183, 255, 0.18);
    white-space: nowrap;
  }

  .library-table th.sortable {
    cursor: pointer;
    user-select: none;
    transition: color 0.15s ease;
  }

  .library-table th.sortable:hover {
    color: var(--color-text);
  }

  .library-table th.active {
    color: var(--color-accent);
  }

  .col-thumb {
    width: 2.5rem;
    padding-left: 0.9rem;
  }

  .col-title {
    width: auto;
  }

  .col-status {
    width: 7rem;
  }

  .col-progress {
    width: 6rem;
  }

  .col-score {
    width: 4rem;
  }

  .data-row {
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .data-row:hover {
    background: rgba(143, 183, 255, 0.08);
  }

  .data-row:focus-visible {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: -2px;
  }

  .cell-thumb {
    padding: 0.5rem 0.9rem;
  }

  .thumb {
    width: 24px;
    height: 24px;
    border-radius: 4px;
    object-fit: cover;
    display: block;
  }

  .thumb-placeholder {
    width: 24px;
    height: 24px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.1);
  }

  .cell-title {
    color: var(--color-text);
    font-weight: 600;
  }

  .cell-status {
    font-size: 0.78rem;
  }

  .status-pill {
    display: inline-block;
    padding: 0.2rem 0.5rem;
    border-radius: 999px;
    font-weight: 700;
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    white-space: nowrap;
  }

  .cell-progress {
    color: var(--color-muted);
    font-variant-numeric: tabular-nums;
  }

  .cell-score {
    color: var(--color-muted);
    font-variant-numeric: tabular-nums;
  }

  .skeleton-row td {
    padding: 0.65rem 0.9rem;
    vertical-align: middle;
  }

  .skeleton-thumb {
    width: 24px;
    height: 24px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.08);
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }

  .skeleton-line {
    height: 0.85rem;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.08);
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }

  .skeleton-pill {
    width: 3.5rem;
    height: 1.1rem;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  .error-box {
    border: 1px solid rgba(255, 157, 157, 0.25);
    border-radius: var(--radius-card);
    background: rgba(255, 157, 157, 0.06);
    padding: 1.25rem;
    display: grid;
    gap: 0.75rem;
  }

  .error {
    color: var(--color-error, #ff9d9d);
    font-size: 0.85rem;
    margin: 0;
  }

  .retry-btn {
    justify-self: start;
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 999px;
    padding: 0.5rem 0.85rem;
    font-size: 0.78rem;
    cursor: pointer;
    background: rgba(143, 183, 255, 0.18);
    color: #e9eefc;
  }

  .retry-btn:hover {
    background: rgba(143, 183, 255, 0.28);
  }

  .retry-btn:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .empty-state {
    border: 1px dashed rgba(143, 183, 255, 0.25);
    border-radius: var(--radius-card);
    background: rgba(255, 255, 255, 0.03);
    padding: 2rem 1.25rem;
    text-align: center;
    display: grid;
    gap: 0.5rem;
  }

  .empty-title {
    color: var(--color-text);
    font-size: 1rem;
    font-weight: 700;
    margin: 0;
  }

  .empty-hint {
    color: var(--color-muted);
    font-size: 0.85rem;
    margin: 0;
  }
</style>
