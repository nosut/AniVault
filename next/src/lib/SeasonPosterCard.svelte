<script lang="ts">
  import type { SeasonAnimeEntry, FutureAnimeEntry } from './api';
  import { createEventDispatcher } from 'svelte';

  export let entry: SeasonAnimeEntry | FutureAnimeEntry;
  export let inLibrary = false;
  export let isNew = false;
  export let future = false;
  export let label = '';

  const dispatch = createEventDispatcher<{
    select: { anime_id: number };
    add: { anime_id: number; title: string };
    quickAdd: { anime_id: number };
  }>();

  function scoreColor(score: number | null | undefined): string {
    if (!score) return 'var(--color-muted)';
    if (score >= 80) return 'var(--color-success)';
    if (score >= 60) return 'var(--color-warning)';
    return 'var(--color-error)';
  }
</script>

<div class="poster-card"
  class:in-library={inLibrary}
  class:is-new={isNew}
  tabindex="0"
  role="button"
  aria-label={entry.title}
  on:click={() => dispatch('select', { anime_id: entry.id })}
  on:contextmenu|preventDefault={() => dispatch('quickAdd', { anime_id: entry.id })}
  on:keydown={(e) => e.key === 'Enter' && dispatch('select', { anime_id: entry.id })}
>
  {#if entry.image_url}
    <img class="poster-img" src={entry.image_url} alt={entry.title} loading="lazy" />
  {:else}
    <div class="poster-img placeholder"></div>
  {/if}
  <div class="badge-row">
    {#if isNew}
      <span class="new-badge">New</span>
    {/if}
    {#if inLibrary}
      <span class="in-library-badge">In Library</span>
    {/if}
  </div>
  {#if !inLibrary}
    <button class="add-btn" on:click|stopPropagation={() => dispatch('add', { anime_id: entry.id, title: entry.title })} aria-label="Add {entry.title} to list">+</button>
  {/if}
  <div class="poster-info">
    <p class="poster-title">{entry.title}</p>
    <div class="poster-meta">
      <span class="poster-format">{entry.format ?? 'TV'}</span>
      {#if future}
        <span class="poster-future">{label}</span>
      {:else if entry.average_score}
        <span class="poster-score" style="color: {scoreColor(entry.average_score)}">{entry.average_score}%</span>
      {/if}
    </div>
  </div>
</div>

<style>
  .poster-card { position: relative; border: 1px solid rgba(var(--color-accent-rgb),0.1); border-radius: 10px; overflow: hidden; background: rgba(255,255,255,0.03); cursor: pointer; transition: border-color 0.15s, transform 0.15s; }
  .poster-card:hover { border-color: rgba(var(--color-accent-rgb),0.3); transform: translateY(-2px); }
  .poster-card.in-library { border-color: rgba(var(--color-success-rgb), 0.55); }
  .poster-card.in-library:hover { border-color: var(--color-success); }
  /* Amber, not the success green: green already means "In Library" on this
     exact card and two unrelated states must not look alike. A card that is
     both new and in-library must stay amber, including on hover. */
  .poster-card.is-new { border-color: rgba(var(--color-warning-rgb), 0.5); }
  .poster-card.is-new:hover { border-color: var(--color-warning); }
  .badge-row { position: absolute; top: 0.3rem; left: 0.3rem; display: flex; align-items: center; gap: 0.3rem; z-index: 1; }
  .in-library-badge { font-size: 0.65rem; padding: 0.2rem 0.5rem; border-radius: 999px; background: rgba(var(--color-success-rgb),0.25); color: var(--color-success); font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em; }
  .new-badge { font-size: 0.65rem; padding: 0.2rem 0.5rem; border-radius: 999px; background: rgba(var(--color-warning-rgb),0.28); color: var(--color-warning); font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em; }
  .add-btn { position: absolute; top: 0.3rem; right: 0.3rem; border: 1px solid rgba(var(--color-accent-rgb),0.3); border-radius: 4px; padding: 0.1rem 0.4rem; background: rgba(var(--color-accent-rgb),0.15); color: var(--color-accent); cursor: pointer; font-size: 0.85rem; line-height: 1.2; z-index: 1; }
  .add-btn:hover { background: rgba(var(--color-accent-rgb),0.3); }
  .poster-img { width: 100%; aspect-ratio: 3/4; object-fit: cover; display: block; }
  .poster-img.placeholder { background: rgba(var(--color-accent-rgb),0.08); }
  .poster-info { padding: 0.5rem 0.6rem; display: flex; flex-direction: column; gap: 0.25rem; }
  .poster-title { font-size: 0.82rem; font-weight: 600; line-height: 1.3; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .poster-meta { display: flex; gap: 0.5rem; font-size: 0.75rem; align-items: center; }
  .poster-format { color: var(--color-muted); }
  .poster-score { font-weight: 600; }
  .poster-future { font-weight: 600; color: var(--color-accent); }
</style>
