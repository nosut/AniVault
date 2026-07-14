<script lang="ts">
  import { onMount } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { searchAnime, getLibraryIds, updateListEntry, importAnilistAnime, type SeasonAnimeEntry } from './api';

  const dispatch = createEventDispatcher<{ select: { anime_id: number } }>();

  export let query = '';
  export let entries: SeasonAnimeEntry[] = [];
  export let hasSearched = false;
  let loading = false;
  let error: string | null = null;
  let libraryIds = new Set<number>();

  async function load() {
    if (!query.trim()) return;
    loading = true; error = null; hasSearched = true;
    try { entries = await searchAnime(query.trim()); }
    catch(e) { error = e instanceof Error ? e.message : String(e); }
    finally { loading = false; }
  }

  async function loadLibraryIds() {
    try {
      libraryIds = new Set(await getLibraryIds());
    } catch {}
  }

  async function handleAddToList(animeId: number) {
    try {
      // Import the anime locally first — list_entry has a foreign key to the
      // anime table, so adding a show straight from AniList would otherwise fail.
      await importAnilistAnime(animeId);
      // updateListEntry also queues the AniList push with this status.
      await updateListEntry(animeId, { status: 'plan_to_watch' });
      libraryIds.add(animeId);
      libraryIds = new Set(libraryIds);
    } catch(e) { error = e instanceof Error ? e.message : String(e); }
  }

  function scoreColor(score: number | null): string {
    if (!score) return 'var(--color-muted)';
    if (score >= 80) return 'var(--color-success)';
    if (score >= 60) return 'var(--color-warning)';
    return 'var(--color-error)';
  }

  onMount(loadLibraryIds);
</script>

<div class="search-view">
  <h2>Search AniList</h2>
  <div class="controls">
    <input class="search-input" type="text" bind:value={query} placeholder="Search anime by title…" on:keydown={(e) => e.key === 'Enter' && load()} />
    <button class="action-btn" on:click={load} disabled={loading || !query.trim()}>{loading ? 'Searching…' : 'Search'}</button>
  </div>

  {#if loading}
    <div class="poster-grid">{#each Array(8) as _}<div class="skeleton-poster" />{/each}</div>
  {:else if error}
    <div class="message error"><p>{error}</p></div>
  {:else if hasSearched && entries.length === 0}
    <p class="empty">No results found for "{query}".</p>
  {:else if entries.length > 0}
    <div class="poster-grid">
      {#each entries as entry (entry.id)}
        <div class="poster-card"
          tabindex="0"
          role="button"
          aria-label={entry.title}
          on:click={() => dispatch('select', { anime_id: entry.id })}
          on:keydown={(e) => e.key === 'Enter' && dispatch('select', { anime_id: entry.id })}
        >
          {#if entry.image_url}
            <img class="poster-img" src={entry.image_url} alt={entry.title} loading="lazy" />
          {:else}
            <div class="poster-img placeholder" />
          {/if}
          {#if libraryIds.has(entry.id)}
            <span class="in-library-badge">In Library</span>
          {:else}
            <button class="add-btn" on:click|stopPropagation={() => handleAddToList(entry.id)} aria-label="Add {entry.title} to list">+</button>
          {/if}
          <div class="poster-info">
            <p class="poster-title">{entry.title}</p>
            <div class="poster-meta">
              <span class="poster-format">{entry.format ?? 'TV'}</span>
              {#if entry.average_score}
                <span class="poster-score" style="color: {scoreColor(entry.average_score)}">{entry.average_score}%</span>
              {/if}
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .search-view { display: flex; flex-direction: column; gap: 1.25rem; }
  h2 { font-size: 1.3rem; font-weight: 700; }
  .controls { display: flex; gap: 0.5rem; }
  .search-input { flex: 1; border: 1px solid rgba(var(--color-accent-rgb),0.18); border-radius: 999px; padding: 0.5rem 1rem; background: rgba(255,255,255,0.04); color: var(--color-text); font-size: 0.9rem; outline: none; }
  .search-input:focus { border-color: var(--color-accent); }
  .action-btn { border: 1px solid rgba(var(--color-accent-rgb),0.35); border-radius: 999px; padding: 0.45rem 0.9rem; background: rgba(var(--color-accent-rgb),0.12); color: #e9eefc; cursor: pointer; font-size: 0.85rem; white-space: nowrap; }
  .poster-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr)); gap: 1rem; }
  .poster-card { position: relative; border: 1px solid rgba(var(--color-accent-rgb),0.1); border-radius: 10px; overflow: hidden; background: rgba(255,255,255,0.03); cursor: pointer; transition: border-color 0.15s, transform 0.15s; }
  .poster-card:hover { border-color: rgba(var(--color-accent-rgb),0.3); transform: translateY(-2px); }
  .in-library-badge { position: absolute; top: 0.3rem; left: 0.3rem; font-size: 0.65rem; padding: 0.2rem 0.5rem; border-radius: 999px; background: rgba(var(--color-success-rgb),0.25); color: var(--color-success); font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em; z-index: 1; }
  .add-btn { position: absolute; top: 0.3rem; right: 0.3rem; border: 1px solid rgba(var(--color-accent-rgb),0.3); border-radius: 4px; padding: 0.1rem 0.4rem; background: rgba(var(--color-accent-rgb),0.15); color: var(--color-accent); cursor: pointer; font-size: 0.85rem; line-height: 1.2; z-index: 1; }
  .add-btn:hover { background: rgba(var(--color-accent-rgb),0.3); }
  .poster-img { width: 100%; aspect-ratio: 3/4; object-fit: cover; display: block; }
  .poster-img.placeholder { background: rgba(var(--color-accent-rgb),0.08); }
  .poster-info { padding: 0.5rem 0.6rem; display: flex; flex-direction: column; gap: 0.25rem; }
  .poster-title { font-size: 0.82rem; font-weight: 600; line-height: 1.3; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .poster-meta { display: flex; gap: 0.5rem; font-size: 0.75rem; align-items: center; }
  .poster-format { color: var(--color-muted); }
  .poster-score { font-weight: 600; }
  .skeleton-poster { aspect-ratio: 3/4; border-radius: 10px; background: rgba(255,255,255,0.04); animation: pulse 2s infinite; }
  @keyframes pulse { 0%,100%{opacity:0.4} 50%{opacity:0.7} }
  .empty { color: var(--color-muted); text-align: center; padding: 2rem; }
  .message.error { color: var(--color-error); padding: 1rem; border: 1px solid rgba(var(--color-error-rgb),0.2); border-radius: 10px; background: rgba(var(--color-error-rgb),0.06); }
</style>
