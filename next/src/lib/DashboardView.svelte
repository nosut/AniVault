<script lang="ts">
  import { onMount } from 'svelte';
  import { getLibraryStats, type LibraryStats, type EngineEvent } from './api';
  import RecognitionCard from './RecognitionCard.svelte';
  import AniListConnect from './AniListConnect.svelte';
  import SyncStatus from './SyncStatus.svelte';
  import KnownFiles from './KnownFiles.svelte';

  export let events: EngineEvent[] = [];

  let stats: LibraryStats | null = null;
  let loading = true;
  let error: string | null = null;

  async function loadStats() {
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

  onMount(loadStats);

  let knownFilesRef: KnownFiles | null = null;

  function handleConfirmed() {
    knownFilesRef?.load();
  }

  const statDefs = [
    { key: 'total' as const, label: 'Total' },
    { key: 'watching' as const, label: 'Watching' },
    { key: 'completed' as const, label: 'Completed' },
    { key: 'on_hold' as const, label: 'On Hold' },
    { key: 'dropped' as const, label: 'Dropped' },
    { key: 'plan_to_watch' as const, label: 'Plan to Watch' },
  ];
</script>

<div class="dashboard">
  <header class="dash-header">
    <h1 class="dash-title">Dashboard</h1>
  </header>

  <section class="stats-section" aria-label="Library stats">
    {#if loading}
      <div class="stats-grid">
        {#each Array(6) as _, i (i)}
          <div class="stat-card skeleton">
            <div class="skeleton-eyebrow"></div>
            <div class="skeleton-value"></div>
          </div>
        {/each}
      </div>
    {:else if error}
      <div class="stats-error">
        <p class="error-msg">{error}</p>
        <button type="button" class="btn-retry" on:click={loadStats}>Retry</button>
      </div>
    {:else if stats && stats.total === 0}
      <div class="stats-empty">
        <p>No anime yet. Connect AniList to import.</p>
      </div>
    {:else if stats}
      <div class="stats-grid">
        {#each statDefs as def (def.key)}
          <div class="stat-card">
            <p class="eyebrow">{def.label}</p>
            <p class="stat-value">{stats[def.key]}</p>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <section class="widgets-grid" aria-label="Widgets">
    <RecognitionCard {events} onConfirmed={handleConfirmed} />
    <AniListConnect />
    <SyncStatus />
    <KnownFiles bind:this={knownFilesRef} />
  </section>
</div>

<style>
  .dashboard {
    display: grid;
    gap: 1.5rem;
    padding: 1.25rem;
  }

  .dash-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .dash-title {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--color-text);
    letter-spacing: -0.01em;
  }

  .stats-section {
    display: grid;
    gap: 0.75rem;
  }

  .stats-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(7.5rem, 1fr));
    gap: 0.75rem;
  }

  .stat-card {
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: var(--radius-card);
    background: rgba(255, 255, 255, 0.04);
    padding: 1.25rem;
    display: grid;
    gap: 0.5rem;
    min-width: 0;
  }

  .eyebrow {
    color: var(--color-accent);
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.78rem;
    font-weight: 800;
    margin: 0;
  }

  .stat-value {
    margin: 0;
    font-size: 1.6rem;
    font-weight: 700;
    color: var(--color-text);
    font-variant-numeric: tabular-nums;
  }

  .skeleton {
    opacity: 0.6;
  }

  .skeleton-eyebrow {
    height: 0.78rem;
    width: 40%;
    background: rgba(143, 183, 255, 0.15);
    border-radius: 999px;
    animation: pulse 2s infinite;
  }

  .skeleton-value {
    height: 1.6rem;
    width: 60%;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 999px;
    animation: pulse 2s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .stats-error {
    border: 1px solid rgba(255, 157, 157, 0.25);
    border-radius: var(--radius-card);
    background: rgba(255, 157, 157, 0.06);
    padding: 1.25rem;
    display: grid;
    gap: 0.75rem;
    justify-items: start;
  }

  .error-msg {
    color: var(--color-error, #ff9d9d);
    font-size: 0.85rem;
    margin: 0;
  }

  .btn-retry {
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 999px;
    padding: 0.5rem 0.85rem;
    font-size: 0.78rem;
    cursor: pointer;
    background: rgba(143, 183, 255, 0.18);
    color: #e9eefc;
  }

  .btn-retry:hover {
    background: rgba(143, 183, 255, 0.28);
  }

  .btn-retry:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .stats-empty {
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: var(--radius-card);
    background: rgba(255, 255, 255, 0.04);
    padding: 1.25rem;
    color: var(--color-muted);
    font-size: 0.85rem;
  }

  .widgets-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(18rem, 1fr));
    gap: 0.75rem;
    align-items: start;
  }

  @media (max-width: 480px) {
    .dashboard {
      padding: 0.75rem;
      gap: 1rem;
    }
    .stats-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .widgets-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
