<script lang="ts">
  import { onMount } from 'svelte';
  import { getCalendar, getLibraryStats, type CalendarEntry, type LibraryStats } from './api';

  let entries: CalendarEntry[] = [];
  let stats: LibraryStats | null = null;
  let loading = true;
  let error: string | null = null;

  async function load() {
    loading = true;
    error = null;
    try {
      [entries, stats] = await Promise.all([getCalendar(), getLibraryStats()]);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function formatCountdown(seconds: number): string {
    if (seconds <= 0) return 'Airing now';
    const d = Math.floor(seconds / 86400);
    const h = Math.floor((seconds % 86400) / 3600);
    if (d > 0) return `${d}d ${h}h`;
    const m = Math.floor((seconds % 3600) / 60);
    if (h > 0) return `${h}h ${m}m`;
    return `${m}m`;
  }

  function formatDate(unix: number): string {
    return new Date(unix * 1000).toLocaleDateString();
  }

  $: sorted = [...entries].sort((a, b) => {
    const tA = a.time_until_airing ?? Infinity;
    const tB = b.time_until_airing ?? Infinity;
    return tA - tB;
  });

  onMount(load);
</script>

<div class="calendar-view">
  <div class="cal-header">
    <h2>Calendar</h2>
    {#if stats}
      <span class="cal-subtitle">{stats.watching} watching</span>
    {/if}
  </div>

  {#if loading}
    <div class="cal-skeleton">
      {#each Array(5) as _}
        <div class="skeleton-card" />
      {/each}
    </div>
  {:else if error}
    <div class="message error" role="alert">
      <p>{error}</p>
      <button class="action-btn" on:click={load}>Retry</button>
    </div>
  {:else if sorted.length === 0}
    <div class="cal-empty">
      <p>No upcoming episodes. Add anime to your Watching list on AniList.</p>
    </div>
  {:else}
    <div class="cal-grid">
      {#each sorted as entry (entry.animeId)}
        <div class="cal-card">
          {#if entry.image_url}
            <img class="cal-thumb" src={entry.image_url} alt={entry.title} loading="lazy" />
          {/if}
          <div class="cal-info">
            <p class="cal-title">{entry.title}</p>
            <div class="cal-meta">
              <span class="cal-progress">Ep {entry.progress ?? 0}{entry.episode_count ? ` / ${entry.episode_count}` : ''}</span>
              {#if entry.next_episode && entry.time_until_airing != null}
                <span class="cal-next">→ Ep {entry.next_episode} in {formatCountdown(entry.time_until_airing)}</span>
              {:else if entry.next_episode && entry.airing_at}
                <span class="cal-next aired">Ep {entry.next_episode} — {formatDate(entry.airing_at)}</span>
              {:else}
                <span class="cal-next none">No upcoming episode</span>
              {/if}
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .calendar-view {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .cal-header {
    display: flex;
    align-items: baseline;
    gap: 1rem;
  }

  .cal-header h2 {
    font-size: 1.3rem;
    font-weight: 700;
  }

  .cal-subtitle {
    color: var(--color-muted);
    font-size: 0.85rem;
  }

  .cal-grid {
    display: grid;
    gap: 0.6rem;
  }

  .cal-card {
    display: flex;
    gap: 0.9rem;
    align-items: center;
    padding: 0.75rem 1rem;
    border: 1px solid rgba(143, 183, 255, 0.12);
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.03);
    transition: background 0.15s;
  }

  .cal-card:hover {
    background: rgba(143, 183, 255, 0.06);
  }

  .cal-thumb {
    width: 3rem;
    height: 4.2rem;
    border-radius: 6px;
    object-fit: cover;
    flex-shrink: 0;
    background: rgba(143, 183, 255, 0.08);
  }

  .cal-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 0;
  }

  .cal-title {
    font-weight: 600;
    font-size: 0.95rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .cal-meta {
    display: flex;
    gap: 0.75rem;
    font-size: 0.82rem;
    flex-wrap: wrap;
  }

  .cal-progress {
    color: var(--color-muted);
  }

  .cal-next {
    color: var(--color-accent);
  }

  .cal-next.aired {
    color: var(--color-muted);
  }

  .cal-next.none {
    color: var(--color-muted);
    font-style: italic;
  }

  .cal-skeleton {
    display: grid;
    gap: 0.6rem;
  }

  .skeleton-card {
    height: 4rem;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.04);
    animation: pulse 2s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 0.4; }
    50% { opacity: 0.7; }
  }

  .cal-empty {
    padding: 2rem;
    text-align: center;
    color: var(--color-muted);
  }

  .message.error {
    color: var(--color-error, #ff9d9d);
    padding: 1rem;
    border: 1px solid rgba(255, 157, 157, 0.2);
    border-radius: 10px;
    background: rgba(255, 157, 157, 0.06);
  }

  .action-btn {
    border: 1px solid rgba(143, 183, 255, 0.3);
    border-radius: 999px;
    padding: 0.4rem 0.9rem;
    background: rgba(143, 183, 255, 0.1);
    color: var(--color-text);
    cursor: pointer;
    margin-top: 0.5rem;
  }
</style>
