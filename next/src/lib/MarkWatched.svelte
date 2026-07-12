<script lang="ts">
  import { onMount } from 'svelte';
  import { markEpisodeWatched, listRecentHistory, type RecentHistoryEntry } from './api';

  let animeId = 0;
  let episode = 0;
  let message: string | null = null;
  let error: string | null = null;
  let recent: RecentHistoryEntry[] = [];
  let loading = false;

  async function handleMark() {
    loading = true;
    error = null;
    message = null;
    try {
      await markEpisodeWatched(animeId, episode);
      message = `Marked anime ${animeId} episode ${episode} watched.`;
      recent = await listRecentHistory(5);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    listRecentHistory(5).then(r => { recent = r; }).catch(e => {
      error = e instanceof Error ? e.message : String(e);
    });
  });
</script>

<section class="mark-watched-card">
  <p class="eyebrow">Manual marking</p>

  <form on:submit|preventDefault={handleMark}>
    <div class="mw-form">
      <label>
        Anime ID
        <input type="number" bind:value={animeId} min="0" />
      </label>
      <label>
        Episode
        <input type="number" bind:value={episode} min="1" />
      </label>
      <button type="submit" class="mw-btn" disabled={loading}>Mark watched</button>
    </div>
  </form>

  {#if error}
    <p class="error" aria-live="polite">{error}</p>
  {/if}
  {#if message}
    <p class="mw-message" aria-live="polite">{message}</p>
  {/if}

  {#if recent.length > 0}
    <div class="mw-recent">
      <p class="mw-recent-label">Recent history</p>
      <ul role="list">
        {#each recent as entry}
          <li role="listitem" class="mw-entry">
            <span>#{entry.anime_id}</span>
            <span>ep {entry.episode}</span>
            <span class="mw-time">{new Date(entry.watched_at * 1000).toLocaleTimeString()}</span>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</section>

<style>
  .mark-watched-card {
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
    border-radius: var(--radius-card);
    padding: 1.25rem;
    background: rgba(255, 255, 255, 0.04);
    display: grid;
    gap: 0.75rem;
  }

  .eyebrow {
    color: var(--color-accent);
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.78rem;
    font-weight: 800;
  }

  .mw-form {
    display: flex;
    gap: 0.75rem;
    align-items: end;
  }

  .mw-form label {
    display: grid;
    gap: 0.25rem;
    font-size: 0.78rem;
    color: var(--color-muted);
  }

  .mw-form input {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(var(--color-accent-rgb), 0.25);
    border-radius: 8px;
    padding: 0.5rem 0.65rem;
    color: var(--color-text);
    width: 6rem;
  }

  .mw-form input:focus {
    outline: 2px solid rgba(var(--color-accent-rgb), 0.5);
    outline-offset: 2px;
  }

  .mw-btn {
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    border-radius: 999px;
    padding: 0.5rem 0.85rem;
    background: rgba(var(--color-accent-rgb), 0.18);
    color: #e9eefc;
    cursor: pointer;
    font-size: 0.82rem;
  }

  .mw-btn:hover {
    background: rgba(var(--color-accent-rgb), 0.28);
  }

  .mw-btn:focus {
    outline: 2px solid rgba(var(--color-accent-rgb), 0.5);
    outline-offset: 2px;
  }

  .mw-message {
    color: var(--color-accent);
    font-size: 0.82rem;
  }

  .error {
    color: var(--color-error);
    font-size: 0.82rem;
  }

  .mw-recent-label {
    color: var(--color-muted);
    font-size: 0.75rem;
    margin-bottom: 0.25rem;
  }

  .mw-recent ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .mw-entry {
    display: flex;
    gap: 0.75rem;
    font-size: 0.78rem;
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  }

  .mw-time {
    color: var(--color-muted);
  }
</style>
