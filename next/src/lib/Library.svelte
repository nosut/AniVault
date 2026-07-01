<script lang="ts">
  import type { LibraryEntry } from './api';

  interface Props {
    library: LibraryEntry[];
  }

  let { library }: Props = $props();

  const filters = [
    { key: 'all', label: 'All' },
    { key: 'watching', label: 'Watching' },
    { key: 'completed', label: 'Completed' },
    { key: 'dropped', label: 'Dropped' },
    { key: 'on_hold', label: 'On Hold' },
    { key: 'plan_to_watch', label: 'Plan to Watch' },
  ];

  let activeFilter = $state('all');
  let searchQuery = $state('');

  const statusColor: Record<string, string> = {
    watching: '#22c55e',
    completed: '#60a5fa',
    on_hold: '#f59e0b',
    dropped: '#ef4444',
    plan_to_watch: '#9aa6b8',
  };

  const filtered = $derived(
    library.filter((entry) => {
      const matchesFilter = activeFilter === 'all' || entry.status === activeFilter;
      const q = searchQuery.trim().toLowerCase();
      const matchesSearch = !q || entry.title.toLowerCase().includes(q);
      return matchesFilter && matchesSearch;
    })
  );
</script>

<div class="library-root">
  <div class="controls">
    <input
      type="text"
      class="search-input"
      placeholder="Search library..."
      bind:value={searchQuery}
    />
    <div class="filter-bar">
      {#each filters as f}
        <button
          class="filter-pill"
          class:active={activeFilter === f.key}
          onclick={() => activeFilter = f.key}
        >
          {f.label}
        </button>
      {/each}
    </div>
  </div>

  {#if filtered.length === 0}
    <div class="empty">No matches found.</div>
  {:else}
    <div class="grid">
      {#each filtered as entry (entry.id)}
        {@const imageUrl = (entry as any).image_url}
        {@const color = statusColor[entry.status] ?? '#9aa6b8'}
        <div class="card" role="button" tabindex="0">
          <div class="poster-wrap">
            {#if imageUrl}
              <img class="poster" src={imageUrl} alt={entry.title} loading="lazy" />
            {:else}
              <div class="poster placeholder"></div>
            {/if}
            <div class="poster-glow"></div>
          </div>
          <div class="meta">
            <div class="title-row">
              <span class="status-dot" style="background: {color}"></span>
              <span class="title">{entry.title}</span>
            </div>
            <span class="episodes">
              Ep {entry.watched_episodes}{#if entry.episode_count} / {entry.episode_count}{/if}
            </span>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .library-root {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .controls {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .search-input {
    width: 100%;
    max-width: 28rem;
    padding: 0.55rem 1rem;
    border: 1px solid rgb(255 255 255 / 10%);
    border-radius: 999px;
    background: rgb(255 255 255 / 5%);
    color: var(--color-text);
    font-size: 0.85rem;
    outline: none;
    transition: border-color 0.2s, background 0.2s;
  }

  .search-input::placeholder {
    color: var(--color-muted);
  }

  .search-input:focus {
    border-color: rgb(255 255 255 / 22%);
    background: rgb(255 255 255 / 8%);
  }

  .filter-bar {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .filter-pill {
    padding: 0.35rem 0.9rem;
    border: 1px solid rgb(255 255 255 / 8%);
    border-radius: 999px;
    background: transparent;
    color: var(--color-muted);
    font-size: 0.78rem;
    cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }

  .filter-pill:hover {
    background: rgb(255 255 255 / 6%);
    color: var(--color-text);
  }

  .filter-pill.active {
    background: rgb(255 255 255 / 10%);
    color: var(--color-text);
    border-color: rgb(255 255 255 / 18%);
  }

  .empty {
    color: var(--color-muted);
    font-size: 0.9rem;
    padding: 2rem 0;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 1.25rem;
  }

  @media (max-width: 1100px) {
    .grid {
      grid-template-columns: repeat(3, 1fr);
    }
  }

  @media (max-width: 780px) {
    .grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    cursor: pointer;
    outline: none;
    border-radius: var(--radius-card);
    padding: 0.4rem;
    transition: transform 0.2s ease;
  }

  .card:hover,
  .card:focus-visible {
    transform: scale(1.03);
  }

  .poster-wrap {
    position: relative;
    border-radius: calc(var(--radius-card) - 4px);
    overflow: hidden;
    aspect-ratio: 2 / 3;
    background: #0d1018;
  }

  .poster {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .placeholder {
    width: 100%;
    height: 100%;
    background: linear-gradient(145deg, rgb(255 255 255 / 6%), rgb(255 255 255 / 2%));
  }

  .poster-glow {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0%);
    transition: box-shadow 0.25s ease;
    pointer-events: none;
  }

  .card:hover .poster-glow,
  .card:focus-visible .poster-glow {
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 14%), 0 0 24px rgb(143 183 255 / 12%);
  }

  .meta {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0 0.2rem;
  }

  .title-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .status-dot {
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }

  .title {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--color-text);
    line-height: 1.2;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .episodes {
    font-size: 0.72rem;
    color: var(--color-muted);
    padding-left: 1.1rem;
  }
</style>
