<script lang="ts">
  import { onMount } from 'svelte';
  import { getWatchingAnime } from './api';
  import type { LibraryEntry } from './api';

  let entries: LibraryEntry[] = $state([]);

  onMount(async () => {
    try { entries = await getWatchingAnime(); } catch { /* empty */ }
  });
</script>

{#if entries.length === 0}
  <div class="card"><span>Not watching anything yet. Play a file to start tracking.</span></div>
{:else}
  <div class="library-list">
    {#each entries as entry}
      <div class="lib-row">
        <span class="lib-title">{entry.title}</span>
        <span class="lib-ep">Ep {entry.watched_episodes}{#if entry.episode_count} / {entry.episode_count}{/if}</span>
        <div class="progress-bar">
          <div class="progress-fill" style="width: {entry.episode_count ? Math.min(100, entry.watched_episodes / entry.episode_count * 100) : 0}%"></div>
        </div>
      </div>
    {/each}
  </div>
{/if}

<style>
  .card { max-width: 34rem; border: 1px solid rgb(255 255 255 / 10%); border-radius: var(--radius-card); background: linear-gradient(145deg, rgb(255 255 255 / 12%), rgb(255 255 255 / 4%)); box-shadow: var(--shadow-card); padding: 1.5rem; }
  .card span { color: var(--color-muted); }
  .library-list { max-width: 42rem; }
  .lib-row { display: flex; align-items: center; gap: 1rem; padding: 0.6rem 0; border-bottom: 1px solid rgb(255 255 255 / 6%); font-size: 0.9rem; }
  .lib-title { flex: 1; color: var(--color-text); }
  .lib-ep { color: var(--color-muted); font-size: 0.78rem; min-width: 5rem; text-align: right; }
  .progress-bar { width: 6rem; height: 4px; background: rgb(255 255 255 / 8%); border-radius: 999px; overflow: hidden; }
  .progress-fill { height: 100%; background: var(--color-accent); border-radius: 999px; transition: width 0.3s; }
</style>
