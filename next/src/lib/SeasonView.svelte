<script lang="ts">
  import { onMount } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { getSeasonAnime, searchLibrary, updateListEntry, queueAniListSync, type SeasonAnimeEntry } from './api';

  const dispatch = createEventDispatcher<{ select: { anime_id: number } }>();

  const seasons = ['WINTER', 'SPRING', 'SUMMER', 'FALL'];
  const seasonLabels: Record<string, string> = { WINTER: 'Winter', SPRING: 'Spring', SUMMER: 'Summer', FALL: 'Fall' };
  const genres = ['', 'Action', 'Adventure', 'Comedy', 'Drama', 'Fantasy', 'Horror', 'Mecha', 'Mystery', 'Romance', 'Sci-Fi', 'Slice of Life', 'Sports', 'Supernatural', 'Thriller'];

  function getCurrentSeason(): { season: string; year: number } {
    const now = new Date();
    const m = now.getMonth();
    const s = m < 3 ? 'WINTER' : m < 6 ? 'SPRING' : m < 9 ? 'SUMMER' : 'FALL';
    return { season: s, year: now.getFullYear() };
  }

  function loadSeasonState(): { season: string; year: number } {
    try {
      const saved = localStorage.getItem('anivault-season-state');
      if (saved) return JSON.parse(saved);
    } catch {}
    return getCurrentSeason();
  }

  function saveSeasonState(s: string, y: number) {
    try { localStorage.setItem('anivault-season-state', JSON.stringify({ season: s, year: y })); }
    catch {}
  }

  let initial = loadSeasonState();
  let season = initial.season;
  let year = initial.year;
  let genre: string = '';
  let entries: SeasonAnimeEntry[] = [];
  let loading = true;
  let error: string | null = null;
  let libraryIds = new Set<number>();

  async function load() {
    loading = true; error = null;
    try { entries = await getSeasonAnime(season, year, genre || undefined); }
    catch(e) { error = e instanceof Error ? e.message : String(e); }
    finally { loading = false; }
  }

  async function loadLibraryIds() {
    try {
      const all = await searchLibrary('', null, 500, 0);
      libraryIds = new Set(all.map(e => e.anime_id));
      libraryIds = libraryIds; // force reactivity
    } catch {
      libraryIds = new Set();
    }
  }

  async function handleAddToList(animeId: number, title: string) {
    try {
      await updateListEntry(animeId, { status: 'plan_to_watch' });
      await queueAniListSync(animeId, 0);
      libraryIds.add(animeId);
      libraryIds = new Set(libraryIds);
    } catch(e) { /* show error? */ }
  }

  async function handleQuickAdd(animeId: number) {
    await handleAddToList(animeId, '');
  }

  function prevSeason() {
    const idx = seasons.indexOf(season);
    if (idx === 0) { season = 'FALL'; year--; }
    else { season = seasons[idx - 1]; }
    load();
  }

  function nextSeason() {
    const idx = seasons.indexOf(season);
    if (idx === 3) { season = 'WINTER'; year++; }
    else { season = seasons[idx + 1]; }
    load();
  }

  function scoreColor(score: number | null): string {
    if (!score) return 'var(--color-muted)';
    if (score >= 80) return '#7ee87e';
    if (score >= 60) return '#f0c040';
    return '#ff9d9d';
  }

  $: saveSeasonState(season, year);

  async function loadAll() {
    await Promise.all([load(), loadLibraryIds()]);
  }
  onMount(() => { loadAll(); });
</script>

<div class="season-view">
  <div class="season-header">
    <div class="season-nav">
      <button class="nav-arrow" on:click={prevSeason} aria-label="Previous season">◀</button>
      <h2>{seasonLabels[season]} {year}</h2>
      <button class="nav-arrow" on:click={nextSeason} aria-label="Next season">▶</button>
    </div>
    <div class="season-controls">
      <select class="genre-select" bind:value={genre} on:change={load}>
        <option value="">All Genres</option>
        {#each genres.filter(g => g) as g}
          <option value={g}>{g}</option>
        {/each}
      </select>
    </div>
  </div>

  {#if loading}
    <div class="poster-grid">{#each Array(12) as _}<div class="skeleton-poster" />{/each}</div>
  {:else if error}
    <div class="message error"><p>{error}</p><button class="action-btn" on:click={load}>Retry</button></div>
  {:else if entries.length === 0}
    <p class="empty">No anime found for this season.</p>
  {:else}
    <div class="poster-grid">
      {#each entries as entry (entry.id)}
        <div class="poster-card"
          tabindex="0"
          role="button"
          aria-label={entry.title}
          on:click={() => dispatch('select', { anime_id: entry.id })}
          on:contextmenu|preventDefault={() => handleQuickAdd(entry.id)}
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
            <button class="add-btn" on:click|stopPropagation={() => handleAddToList(entry.id, entry.title)} aria-label="Add {entry.title} to list">+</button>
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
  .season-view { display: flex; flex-direction: column; gap: 1.25rem; }
  .season-header { display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.75rem; }
  .season-nav { display: flex; align-items: center; gap: 0.75rem; }
  .season-nav h2 { font-size: 1.3rem; font-weight: 700; min-width: 10rem; }
  .nav-arrow { border: 1px solid rgba(143,183,255,0.2); border-radius: 999px; padding: 0.4rem 0.7rem; background: transparent; color: var(--color-muted); cursor: pointer; font-size: 0.85rem; }
  .nav-arrow:hover { background: rgba(143,183,255,0.1); color: var(--color-text); }
  .genre-select { border: 1px solid rgba(143,183,255,0.2); border-radius: 999px; padding: 0.4rem 0.8rem; background: rgba(255,255,255,0.06); color: var(--color-text); font-size: 0.85rem; }
  .genre-select option { background: #141820; }
  .poster-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr)); gap: 1rem; }
  .poster-card { position: relative; border: 1px solid rgba(143,183,255,0.1); border-radius: 10px; overflow: hidden; background: rgba(255,255,255,0.03); cursor: pointer; transition: border-color 0.15s, transform 0.15s; }
  .poster-card:hover { border-color: rgba(143,183,255,0.3); transform: translateY(-2px); }
  .in-library-badge { position: absolute; top: 0.3rem; left: 0.3rem; font-size: 0.65rem; padding: 0.2rem 0.5rem; border-radius: 999px; background: rgba(126,232,126,0.25); color: #7ee87e; font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em; z-index: 1; }
  .add-btn { position: absolute; top: 0.3rem; right: 0.3rem; border: 1px solid rgba(143,183,255,0.3); border-radius: 4px; padding: 0.1rem 0.4rem; background: rgba(143,183,255,0.15); color: var(--color-accent); cursor: pointer; font-size: 0.85rem; line-height: 1.2; z-index: 1; }
  .add-btn:hover { background: rgba(143,183,255,0.3); }
  .poster-img { width: 100%; aspect-ratio: 3/4; object-fit: cover; display: block; }
  .poster-img.placeholder { background: rgba(143,183,255,0.08); }
  .poster-info { padding: 0.5rem 0.6rem; display: flex; flex-direction: column; gap: 0.25rem; }
  .poster-title { font-size: 0.82rem; font-weight: 600; line-height: 1.3; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .poster-meta { display: flex; gap: 0.5rem; font-size: 0.75rem; align-items: center; }
  .poster-format { color: var(--color-muted); }
  .poster-score { font-weight: 600; }
  .skeleton-poster { aspect-ratio: 3/4; border-radius: 10px; background: rgba(255,255,255,0.04); animation: pulse 2s infinite; }
  @keyframes pulse { 0%,100%{opacity:0.4} 50%{opacity:0.7} }
  .empty { color: var(--color-muted); text-align: center; padding: 2rem; }
  .message.error { color: #ff9d9d; padding: 1rem; border: 1px solid rgba(255,157,157,0.2); border-radius: 10px; background: rgba(255,157,157,0.06); }
  .action-btn { border: 1px solid rgba(143,183,255,0.3); border-radius: 999px; padding: 0.4rem 0.9rem; background: rgba(143,183,255,0.1); color: var(--color-text); cursor: pointer; margin-top: 0.5rem; }
</style>
