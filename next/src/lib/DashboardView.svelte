<script lang="ts">
  import { onMount } from 'svelte';
  import { getLibraryStats, type LibraryStats, type EngineEvent } from './api';
  import NowPlaying from './NowPlaying.svelte';
  import MarkWatched from './MarkWatched.svelte';
  import RecognitionCard from './RecognitionCard.svelte';
  import AniListConnect from './AniListConnect.svelte';
  import SyncStatus from './SyncStatus.svelte';
  import KnownFiles from './KnownFiles.svelte';

  export let events: EngineEvent[] = [];
  export let onConfirmed: () => void = () => {};

  let stats: LibraryStats | null = null;
  let error: string | null = null;
  let loading = false;
  let knownFilesRef: { load: () => Promise<void> } | undefined;

  const statKeys: { key: keyof LibraryStats; label: string; accent: string }[] = [
    { key: 'total', label: 'Total', accent: '#e9eefc' },
    { key: 'watching', label: 'Watching', accent: '#8fb7ff' },
    { key: 'completed', label: 'Completed', accent: '#7dd3a8' },
    { key: 'on_hold', label: 'On Hold', accent: '#ffc164' },
    { key: 'dropped', label: 'Dropped', accent: '#ff9d9d' },
    { key: 'plan_to_watch', label: 'Plan to Watch', accent: '#c9a8ff' },
  ];

  async function load() {
    loading = true;
    error = null;
    try {
      stats = await getLibraryStats();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);
</script>

<div class="dashboard">
  <section class="stats-section" aria-label="Library stats">
    {#if loading}
      <div class="stats-grid">
        {#each Array(6) as _, i}
          <div class="stat-card skeleton" aria-hidden="true">
            <div class="skeleton-number"></div>
            <div class="skeleton-label"></div>
          </div>
        {/each}
      </div>
    {:else if error}
      <div class="stats-error">
        <p class="error">{error}</p>
        <button type="button" class="retry-btn" on:click={load}>Retry</button>
      </div>
    {:else if stats}
      {#if stats.total === 0}
        <div class="empty-state">
          <p class="empty-title">No anime in your library yet</p>
          <p class="empty-hint">Connect AniList to import your list and get started.</p>
        </div>
      {:else}
        <div class="stats-grid">
          {#each statKeys as { key, label, accent }}
            <div class="stat-card" style="--card-accent: {accent}">
              <span class="stat-number" style="color: {accent}">{stats[key]}</span>
              <span class="stat-label">{label}</span>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </section>

  <div class="widget-stack">
    <NowPlaying {events} />
    <MarkWatched />
    <RecognitionCard {events} {onConfirmed} />
    <AniListConnect />
    <SyncStatus />
    <KnownFiles bind:this={knownFilesRef} />
  </div>
</div>

<style>
  .dashboard {
    display: grid;
    gap: 1.5rem;
  }

  .stats-section {
    display: grid;
    gap: 0.75rem;
  }

  .stats-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 0.75rem;
  }

  .stat-card {
    flex: 1 1 8rem;
    min-width: 7rem;
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: var(--radius-card);
    background: rgba(255, 255, 255, 0.04);
    padding: 1.25rem;
    display: grid;
    gap: 0.35rem;
    transition: border-color 0.2s ease;
  }

  .stat-card:hover {
    border-color: rgba(143, 183, 255, 0.35);
  }

  .stat-number {
    font-size: 2rem;
    font-weight: 800;
    line-height: 1;
    font-variant-numeric: tabular-nums;
  }

  .stat-label {
    color: var(--color-muted);
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    font-weight: 700;
  }

  .skeleton {
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }

  .skeleton-number {
    width: 3rem;
    height: 2rem;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 6px;
  }

  .skeleton-label {
    width: 4rem;
    height: 0.7rem;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 4px;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  .stats-error {
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

  .widget-stack {
    display: grid;
    gap: 1rem;
  }
</style>
