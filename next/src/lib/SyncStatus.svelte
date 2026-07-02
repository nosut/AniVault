<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getSyncStatus, triggerSync, type AniListSyncStatus } from './api';

  let status: AniListSyncStatus | null = null;
  let error: string | null = null;
  let loading = false;
  let syncing = false;
  let intervalId: ReturnType<typeof setInterval> | null = null;

  async function refresh() {
    if (loading) return;
    loading = true;
    try {
      status = await getSyncStatus();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function handleSyncNow() {
    syncing = true;
    try {
      await triggerSync();
      await refresh();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      syncing = false;
    }
  }

  function startPolling() {
    refresh();
    intervalId = setInterval(refresh, 5000);
  }

  function stopPolling() {
    if (intervalId) {
      clearInterval(intervalId);
      intervalId = null;
    }
  }

  onMount(startPolling);
  onDestroy(stopPolling);
</script>

<section class="sync-card" aria-label="Sync status">
  <div class="sync-header">
    <p class="eyebrow">Sync</p>
    <button type="button" class="btn-refresh" on:click={handleSyncNow} disabled={syncing || loading}>
      {syncing ? 'Syncing…' : 'Sync Now'}
    </button>
  </div>

  {#if error}
    <p class="error" aria-live="polite">{error}</p>
  {/if}

  {#if status}
    <dl class="sync-list">
      <div>
        <dt>Pending</dt>
        <dd>{status.pending}</dd>
      </div>
      <div>
        <dt>Failed</dt>
        <dd>{status.failed}</dd>
      </div>
      <div>
        <dt>Blocked</dt>
        <dd>{status.blocked}</dd>
      </div>
    </dl>

    {#if status.last_sync_at}
      <p class="last-sync">
        Last sync: {new Date(status.last_sync_at * 1000).toLocaleString()}
      </p>
    {:else}
      <p class="last-sync">No sync yet</p>
    {/if}
  {:else if !error}
    <p class="idle">Checking sync status…</p>
  {/if}
</section>

<style>
  .sync-card {
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: var(--radius-card);
    padding: 1.25rem;
    background: rgba(255, 255, 255, 0.04);
    display: grid;
    gap: 0.75rem;
  }

  .sync-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .eyebrow {
    color: var(--color-accent);
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.78rem;
    font-weight: 800;
  }

  .btn-refresh {
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 999px;
    padding: 0.5rem 0.85rem;
    font-size: 0.78rem;
    cursor: pointer;
    background: rgba(143, 183, 255, 0.18);
    color: #e9eefc;
  }

  .btn-refresh:hover:not(:disabled) {
    background: rgba(143, 183, 255, 0.28);
  }

  .btn-refresh:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .sync-list {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 1rem;
    margin: 0;
  }

  .sync-list div {
    display: grid;
    gap: 0.25rem;
  }

  .sync-list dt {
    color: var(--color-muted);
    font-size: 0.78rem;
  }

  .sync-list dd {
    margin: 0;
    font-weight: 700;
  }

  .last-sync,
  .idle {
    color: var(--color-muted);
    font-size: 0.82rem;
    margin: 0;
  }

  .error {
    color: var(--color-error, #ff9d9d);
    font-size: 0.82rem;
  }
</style>
