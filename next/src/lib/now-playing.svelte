<script lang="ts">
  import { onMount } from 'svelte';
  import { getTrackingStatus, setWatchedEpisodes } from './api';

  let currentAnime: string | null = $state(null);
  let isRunning: boolean = $state(false);
  let animeId: number | null = $state(null);
  let currentEpisode: number | null = $state(null);
  let editing: boolean = $state(false);

  onMount(() => {
    const interval = setInterval(async () => {
      try {
        const status = await getTrackingStatus();
        isRunning = status.is_running;
        currentAnime = status.current_anime;
        animeId = status.current_anime_id;
        currentEpisode = status.current_episode;
      } catch {
        isRunning = false;
        currentAnime = null;
      }
    }, 2000);

    return () => clearInterval(interval);
  });

  async function saveEpisode() {
    if (animeId != null && currentEpisode != null) {
      await setWatchedEpisodes(animeId, currentEpisode);
    }
    editing = false;
  }
</script>

{#if isRunning || currentAnime}
  <div class="now-playing">
    {#if currentAnime}
      Tracking <strong>{currentAnime}</strong>
      {#if currentEpisode != null && animeId != null}
        {#if editing}
          <input type="number" class="ep-input" bind:value={currentEpisode} onkeydown={(e) => e.key === 'Enter' && saveEpisode()} onblur={saveEpisode} />
        {:else}
          <button class="ep-chip" onclick={() => editing = true} title="Click to edit episode">
            Ep {currentEpisode}
          </button>
        {/if}
      {/if}
    {:else if isRunning}
      Tracking ready
    {/if}
  </div>
{/if}

<style>
  .now-playing {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
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

  .ep-chip {
    border: 1px solid var(--color-accent);
    border-radius: 999px;
    padding: 0.1rem 0.5rem;
    font-size: 0.72rem;
    color: var(--color-accent);
    background: transparent;
    cursor: pointer;
  }

  .ep-chip:hover {
    background: rgb(255 255 255 / 6%);
  }

  .ep-input {
    width: 3.5rem;
    border: 1px solid var(--color-accent);
    border-radius: 999px;
    padding: 0.1rem 0.4rem;
    font-size: 0.72rem;
    color: var(--color-text);
    background: transparent;
    text-align: center;
  }
</style>
