<script lang="ts">
  import { onMount } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { getSeasonAnime, getFutureAnime, getLibraryIds, updateListEntry, importAnilistAnime, type SeasonAnimeEntry, type FutureAnimeEntry } from './api';
  import { addSeasons, futureLabel, seasonOffset } from './seasonUi';
  import { ChevronLeft, ChevronRight } from 'lucide-svelte';

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

  // Seasons reachable with the arrows: current through +4 ahead. Beyond that
  // sits the single "Future Seasons" page (far-out and TBA announcements).
  const BROWSABLE_SEASONS_AHEAD = 4;

  function loadSeasonState(): { season: string; year: number; genre: string; future: boolean } {
    try {
      const saved = localStorage.getItem('anivault-season-state');
      if (saved) {
        const parsed = JSON.parse(saved);
        const current = getCurrentSeason();
        // Saved states deeper than the browsable window (including ones from
        // before the future page existed) land on the Future Seasons page,
        // anchored at the last browsable season so prev exits sensibly.
        if (parsed.future || seasonOffset(parsed.season, parsed.year, current.season, current.year) > BROWSABLE_SEASONS_AHEAD) {
          const last = addSeasons(current.season, current.year, BROWSABLE_SEASONS_AHEAD);
          return { ...last, genre: parsed.genre ?? '', future: true };
        }
        return { season: parsed.season, year: parsed.year, genre: parsed.genre ?? '', future: false };
      }
    } catch {}
    return { ...getCurrentSeason(), genre: '', future: false };
  }

  function saveSeasonState(s: string, y: number, g: string, f: boolean) {
    try { localStorage.setItem('anivault-season-state', JSON.stringify({ season: s, year: y, genre: g, future: f })); }
    catch {}
  }

  let initial = loadSeasonState();
  let season = initial.season;
  let year = initial.year;
  let genre: string = initial.genre;
  let future = initial.future;
  let entries: (SeasonAnimeEntry | FutureAnimeEntry)[] = [];
  let loading = true;
  let error: string | null = null;
  let libraryIds = new Set<number>();

  async function load() {
    loading = true; error = null;
    try {
      entries = future
        ? await getFutureAnime(genre || undefined)
        : await getSeasonAnime(season, year, genre || undefined);
    }
    catch(e) { error = e instanceof Error ? e.message : String(e); }
    finally { loading = false; }
  }

  async function loadLibraryIds() {
    try {
      libraryIds = new Set(await getLibraryIds());
    } catch {
      libraryIds = new Set();
    }
  }

  async function handleAddToList(animeId: number, title: string) {
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

  async function handleQuickAdd(animeId: number) {
    await handleAddToList(animeId, '');
  }

  function prevSeason() {
    if (future) {
      // Exit the future page back to the last browsable season (season/year
      // still hold it — they don't advance while on the future page).
      future = false;
      load();
      return;
    }
    const idx = seasons.indexOf(season);
    if (idx === 0) { season = 'FALL'; year--; }
    else { season = seasons[idx - 1] ?? 'WINTER'; }
    load();
  }

  function nextSeason() {
    const current = getCurrentSeason();
    if (seasonOffset(season, year, current.season, current.year) >= BROWSABLE_SEASONS_AHEAD) {
      future = true;
      load();
      return;
    }
    const idx = seasons.indexOf(season);
    if (idx === 3) { season = 'WINTER'; year++; }
    else { season = seasons[idx + 1] ?? 'WINTER'; }
    load();
  }

  function goCurrentSeason() {
    const current = getCurrentSeason();
    season = current.season;
    year = current.year;
    future = false;
    load();
  }

  $: viewingCurrentSeason = !future && seasonOffset(season, year, getCurrentSeason().season, getCurrentSeason().year) === 0;

  // The grid holds a union of season/future entries; only future entries carry
  // season fields, which futureLabel treats as optional.
  function labelFor(entry: SeasonAnimeEntry | FutureAnimeEntry): string {
    return futureLabel(entry as FutureAnimeEntry);
  }

  function scoreColor(score: number | null): string {
    if (!score) return 'var(--color-muted)';
    if (score >= 80) return 'var(--color-success)';
    if (score >= 60) return 'var(--color-warning)';
    return 'var(--color-error)';
  }

  $: saveSeasonState(season, year, genre, future);

  async function loadAll() {
    await Promise.all([load(), loadLibraryIds()]);
  }
  onMount(() => { loadAll(); });
</script>

<div class="season-view">
  <div class="season-header">
    <div class="season-nav">
      <button class="nav-arrow" on:click={prevSeason} aria-label="Previous season"><ChevronLeft size={15} /></button>
      <h2>{future ? 'Future Seasons' : `${seasonLabels[season]} ${year}`}</h2>
      {#if !future}
        <button class="nav-arrow" on:click={nextSeason} aria-label="Next season"><ChevronRight size={15} /></button>
      {/if}
      {#if !viewingCurrentSeason}
        <button class="nav-arrow current-btn" on:click={goCurrentSeason} aria-label="Go to current season">Current</button>
      {/if}
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
    <div class="poster-grid">{#each Array(12) as _}<div class="skeleton-poster"></div>{/each}</div>
  {:else if error}
    <div class="message error"><p>{error}</p><button class="action-btn" on:click={load}>Retry</button></div>
  {:else if entries.length === 0}
    <p class="empty">{future ? 'No far-future or TBA announcements found.' : 'No anime found for this season.'}</p>
  {:else}
    <div class="poster-grid">
      {#each entries as entry (entry.id)}
        <div class="poster-card"
          class:in-library={libraryIds.has(entry.id)}
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
            <div class="poster-img placeholder"></div>
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
              {#if future}
                <span class="poster-future">{labelFor(entry)}</span>
              {:else if entry.average_score}
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
  .nav-arrow { display: inline-flex; align-items: center; justify-content: center; border: 1px solid rgba(var(--color-accent-rgb),0.2); border-radius: 999px; padding: 0.4rem 0.7rem; background: transparent; color: var(--color-muted); cursor: pointer; font-size: 0.85rem; }
  .nav-arrow:hover { background: rgba(var(--color-accent-rgb),0.1); color: var(--color-text); }
  .current-btn { font-size: 0.78rem; padding: 0.35rem 0.8rem; }
  .genre-select { border: 1px solid rgba(var(--color-accent-rgb),0.2); border-radius: 999px; padding: 0.4rem 0.8rem; background: rgba(255,255,255,0.06); color: var(--color-text); font-size: 0.85rem; }
  .genre-select option { background: #141820; }
  .poster-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr)); gap: 1rem; }
  .poster-card { position: relative; border: 1px solid rgba(var(--color-accent-rgb),0.1); border-radius: 10px; overflow: hidden; background: rgba(255,255,255,0.03); cursor: pointer; transition: border-color 0.15s, transform 0.15s; }
  .poster-card:hover { border-color: rgba(var(--color-accent-rgb),0.3); transform: translateY(-2px); }
  .poster-card.in-library { border-color: rgba(var(--color-success-rgb), 0.55); }
  .poster-card.in-library:hover { border-color: var(--color-success); }
  .in-library-badge { position: absolute; top: 0.3rem; left: 0.3rem; font-size: 0.65rem; padding: 0.2rem 0.5rem; border-radius: 999px; background: rgba(var(--color-success-rgb),0.25); color: var(--color-success); font-weight: 700; text-transform: uppercase; letter-spacing: 0.05em; z-index: 1; }
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
  .skeleton-poster { aspect-ratio: 3/4; border-radius: 10px; background: rgba(255,255,255,0.04); animation: pulse 2s infinite; }
  @keyframes pulse { 0%,100%{opacity:0.4} 50%{opacity:0.7} }
  .empty { color: var(--color-muted); text-align: center; padding: 2rem; }
  .message.error { color: var(--color-error); padding: 1rem; border: 1px solid rgba(var(--color-error-rgb),0.2); border-radius: 10px; background: rgba(var(--color-error-rgb),0.06); }
  .action-btn { border: 1px solid rgba(var(--color-accent-rgb),0.3); border-radius: 999px; padding: 0.4rem 0.9rem; background: rgba(var(--color-accent-rgb),0.1); color: var(--color-text); cursor: pointer; margin-top: 0.5rem; }
</style>
