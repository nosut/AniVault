<script lang="ts">
  import { onMount } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { searchLibrary, type LibraryEntry } from './api';

  const dispatch = createEventDispatcher<{ select: { anime_id: number } }>();

  let query = '';
  let statusFilter: string | null = null;
  let entries: LibraryEntry[] = [];
  let loading = false;
  let error = '';

  let sortKey: 'title' | 'status' | 'progress' | 'score' = 'title';
  let sortDir: 'asc' | 'desc' = 'asc';

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
    <select
      class="filter"
      bind:value={statusFilter}
      on:change={load}
      aria-label="Filter by status"
    >
      {#each statusOptions as opt}
        <option value={opt.value}>{opt.label}</option>
      {/each}
    </select>
  </div>

  {#if error}
    <div class="message error" role="alert">
      <p>{error}</p>
      <button type="button" class="retry" on:click={load}>Retry</button>
    </div>
  {/if}

  <div class="table-wrap">
    <table>
      <thead>
        <tr>
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
              <td><div class="skeleton-thumb"></div></td>
              <td><div class="skeleton-line"></div></td>
              <td><div class="skeleton-badge"></div></td>
              <td><div class="skeleton-line short"></div></td>
              <td><div class="skeleton-line short"></div></td>
            </tr>
          {/each}
        {:else if sortedEntries.length === 0}
          <tr class="empty-row">
            <td colspan="5">
              <p class="empty">No anime found.</p>
            </td>
          </tr>
        {:else}
          {#each sortedEntries as entry (entry.anime_id)}
            <tr
              class="data-row"
              tabindex="0"
              on:click={() => handleRowActivate(entry)}
              on:keydown={(e) => onRowKeydown(e, entry)}
            >
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
              <td class="num-cell">
                {entry.watched_episodes ?? 0} / {entry.episode_count ?? '?'}
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

  .search,
  .filter {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: var(--radius-card);
    color: var(--color-text);
    padding: 0.6rem 0.9rem;
    font-family: var(--font-ui);
    font-size: 0.9rem;
    outline: none;
  }

  .search {
    min-width: 16rem;
    flex: 1 1 16rem;
  }

  .filter {
    min-width: 10rem;
  }

  .search:focus,
  .filter:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 3px rgba(143, 183, 255, 0.25);
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
</style>
