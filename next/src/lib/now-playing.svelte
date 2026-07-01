<script lang="ts">
  import { onMount } from 'svelte';
  import { getTrackingStatus } from './api';

  let currentAnime: string | null = $state(null);
  let isRunning: boolean = $state(false);

  onMount(() => {
    const interval = setInterval(async () => {
      try {
        const status = await getTrackingStatus();
        isRunning = status.is_running;
        currentAnime = status.current_anime;
      } catch {
        isRunning = false;
        currentAnime = null;
      }
    }, 2000);

    return () => clearInterval(interval);
  });
</script>

{#if isRunning || currentAnime}
  <div class="now-playing">
    {#if currentAnime}
      Tracking <strong>{currentAnime}</strong>
    {:else if isRunning}
      Tracking ready
    {/if}
  </div>
{/if}

<style>
  .now-playing {
    display: inline-block;
    border: 1px solid rgb(255 255 255 / 10%);
    border-radius: 999px;
    padding: 0.4rem 1rem;
    margin-top: 1.5rem;
    font-size: 0.82rem;
    color: var(--color-muted);
    background: rgb(255 255 255 / 4%);
  }

  .now-playing strong {
    color: var(--color-text);
    font-weight: 600;
  }
</style>
