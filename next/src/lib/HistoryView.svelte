<script lang="ts">
  import { onMount } from 'svelte';
  import { getWatchHistory, type WatchHistoryEntry } from './api';

  let entries: WatchHistoryEntry[] = [];
  let query = '';
  let loading = true;
  let error: string | null = null;
  let offset = 0;
  let hasMore = true;
  const pageSize = 50;

  async function load(reset = false) {
    if (reset) { offset = 0; entries = []; hasMore = true; }
    loading = true;
    error = null;
    try {
      const newEntries = await getWatchHistory(query || undefined, pageSize, offset);
      if (reset) entries = newEntries;
      else entries = [...entries, ...newEntries];
      hasMore = newEntries.length === pageSize;
      offset += newEntries.length;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function formatDate(unix: number): string {
    const d = new Date(unix * 1000);
    return d.toLocaleDateString() + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }

  function handleSearch() {
    load(true);
  }

  onMount(() => load(true));
</script>

<div class="history-view">
  <h2>Watch History</h2>

  <div class="controls">
    <input class="search" type="text" bind:value={query} placeholder="Filter by anime title…" on:keydown={(e) => e.key === 'Enter' && handleSearch()} />
    <button class="action-btn" on:click={handleSearch}>Search</button>
  </div>

  {#if loading && entries.length === 0}
    <div class="skeleton-list">
      {#each Array(8) as _}<div class="skeleton-row" />{/each}
    </div>
  {:else if error}
    <div class="message error" role="alert"><p>{error}</p><button class="action-btn" on:click={() => load(true)}>Retry</button></div>
  {:else if entries.length === 0}
    <p class="empty">No watch history yet. Start tracking anime to build history.</p>
  {:else}
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Anime</th>
            <th>Episode</th>
            <th>Player</th>
            <th>Date</th>
            <th>Source</th>
          </tr>
        </thead>
        <tbody>
          {#each entries as entry (entry.id)}
            <tr>
              <td class="title-cell">{entry.anime_title}</td>
              <td class="num-cell">{entry.episode}</td>
              <td class="muted-cell">{entry.player ?? '—'}</td>
              <td class="muted-cell">{formatDate(entry.watched_at)}</td>
              <td><span class="source-badge">{entry.source}</span></td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if hasMore}
      <button class="load-more" on:click={() => load(false)} disabled={loading}>
        {loading ? 'Loading…' : 'Load more'}
      </button>
    {/if}
  {/if}
</div>

<style>
  .history-view { display: flex; flex-direction: column; gap: 1rem; }
  h2 { font-size: 1.3rem; font-weight: 700; }
  .controls { display: flex; gap: 0.5rem; align-items: center; }
  .search { flex: 1; border: 1px solid rgba(143,183,255,0.18); border-radius: 999px; padding: 0.5rem 1rem; background: rgba(255,255,255,0.04); color: var(--color-text); font-size: 0.9rem; outline: none; }
  .search:focus { border-color: var(--color-accent); }
  .action-btn { border: 1px solid rgba(143,183,255,0.35); border-radius: 999px; padding: 0.45rem 0.9rem; background: rgba(143,183,255,0.12); color: #e9eefc; cursor: pointer; font-size: 0.85rem; }
  .table-wrap { overflow-x: auto; }
  table { width: 100%; border-collapse: collapse; font-size: 0.9rem; }
  th { text-align: left; padding: 0.6rem 0.75rem; color: var(--color-muted); font-weight: 600; font-size: 0.8rem; border-bottom: 1px solid rgba(143,183,255,0.1); }
  td { padding: 0.5rem 0.75rem; border-bottom: 1px solid rgba(143,183,255,0.06); }
  tr:hover td { background: rgba(143,183,255,0.04); }
  .title-cell { font-weight: 500; max-width: 20rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .num-cell { font-variant-numeric: tabular-nums; text-align: right; }
  .muted-cell { color: var(--color-muted); font-size: 0.82rem; }
  .source-badge { font-size: 0.72rem; padding: 0.15rem 0.5rem; border-radius: 999px; background: rgba(143,183,255,0.12); color: var(--color-accent); }
  .load-more { display: block; margin: 0.5rem auto; border: 1px solid rgba(143,183,255,0.2); border-radius: 999px; padding: 0.5rem 1.5rem; background: transparent; color: var(--color-muted); cursor: pointer; font-size: 0.85rem; }
  .load-more:hover { background: rgba(143,183,255,0.08); color: var(--color-text); }
  .empty { color: var(--color-muted); text-align: center; padding: 2rem; }
  .message.error { color: #ff9d9d; padding: 1rem; border: 1px solid rgba(255,157,157,0.2); border-radius: 10px; background: rgba(255,157,157,0.06); }
  .skeleton-list { display: grid; gap: 0.5rem; }
  .skeleton-row { height: 2rem; border-radius: 6px; background: rgba(255,255,255,0.04); animation: pulse 2s infinite; }
  @keyframes pulse { 0%,100%{opacity:0.4} 50%{opacity:0.7} }
</style>
