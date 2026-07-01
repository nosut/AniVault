<script lang="ts">
  import { onMount } from 'svelte';
  import { getSyncStatus } from './api';
  import type { SyncStatus } from './api';

  let status: SyncStatus | null = $state(null);

  async function refresh() {
    try { status = await getSyncStatus(); } catch { status = { pending: -1, failed: 0 }; }
  }

  onMount(() => {
    refresh();
    const interval = setInterval(refresh, 10_000);
    return () => clearInterval(interval);
  });
</script>

<div class="card">
  <span>Sync Queue</span>
  {#if status == null}
    <p style="color:var(--color-muted);font-size:0.82rem;">Loading...</p>
  {:else if status.pending === -1}
    <p style="color:#ef4444;font-size:0.82rem;">Failed to load sync status.</p>
  {:else if status.pending === 0}
    <p style="color:#22c55e;font-size:0.82rem;">All items synced to AniList.</p>
  {:else}
    <p style="color:#f59e0b;font-size:0.82rem;">{status.pending} item{status.pending !== 1 ? 's' : ''} pending sync.</p>
    <p class="oauth-msg">Sync runs automatically with backoff. Check back shortly.</p>
  {/if}
</div>

<style>
  .card { max-width: 34rem; border: 1px solid rgb(255 255 255 / 10%); border-radius: var(--radius-card); background: linear-gradient(145deg, rgb(255 255 255 / 12%), rgb(255 255 255 / 4%)); box-shadow: var(--shadow-card); padding: 1.5rem; }
  .card span { color: var(--color-muted); }
  .oauth-msg { color: var(--color-accent); font-size: 0.78rem; margin-top: 0.5rem; }
</style>
