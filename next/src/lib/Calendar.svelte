<script lang="ts">
  import { getSeasonalAnime } from './api';
  import type { SeasonalAnime } from './api';

  const seasons = ['WINTER', 'SPRING', 'SUMMER', 'FALL'] as const;
  type Season = (typeof seasons)[number];

  function getCurrentSeason(): { season: Season; year: number } {
    const now = new Date();
    const month = now.getMonth();
    const year = now.getFullYear();
    if (month <= 2) return { season: 'WINTER', year };
    if (month <= 5) return { season: 'SPRING', year };
    if (month <= 8) return { season: 'SUMMER', year };
    return { season: 'FALL', year };
  }

  function seasonLabel(s: Season): string {
    return s.charAt(0) + s.slice(1).toLowerCase();
  }

  let currentSeason = $state(getCurrentSeason());
  let anime: SeasonalAnime[] = $state([]);
  let loading = $state(false);
  let error = $state(false);

  async function load() {
    loading = true;
    error = false;
    try {
      anime = await getSeasonalAnime(currentSeason.season, currentSeason.year);
    } catch {
      error = true;
      anime = [];
    } finally {
      loading = false;
    }
  }

  const prevSeasonMap: Record<Season, Season> = {
    WINTER: 'FALL',
    SPRING: 'WINTER',
    SUMMER: 'SPRING',
    FALL: 'SUMMER',
  };

  const nextSeasonMap: Record<Season, Season> = {
    WINTER: 'SPRING',
    SPRING: 'SUMMER',
    SUMMER: 'FALL',
    FALL: 'WINTER',
  };

  function prevSeason() {
    const s = prevSeasonMap[currentSeason.season];
    const y = currentSeason.season === 'WINTER' ? currentSeason.year - 1 : currentSeason.year;
    currentSeason = { season: s, year: y };
  }

  function nextSeason() {
    const s = nextSeasonMap[currentSeason.season];
    const y = currentSeason.season === 'FALL' ? currentSeason.year + 1 : currentSeason.year;
    currentSeason = { season: s, year: y };
  }

  $effect(() => {
    load();
  });
</script>

<div class="calendar-root">
  <div class="season-bar">
    <button class="arrow" onclick={prevSeason} aria-label="Previous season">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6" /></svg>
    </button>
    <span class="season-label">{seasonLabel(currentSeason.season)} {currentSeason.year}</span>
    <button class="arrow" onclick={nextSeason} aria-label="Next season">
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6" /></svg>
    </button>
  </div>

  {#if loading}
    <div class="status-msg">Loading...</div>
  {:else if error}
    <div class="status-msg">Failed to load seasonal anime</div>
  {:else if anime.length === 0}
    <div class="status-msg">No seasonal anime found.</div>
  {:else}
    <div class="grid">
      {#each anime as entry (entry.anilist_id)}
        <button
          class="card"
          onclick={() => window.open(`https://anilist.co/anime/${entry.anilist_id}`, '_blank')}
        >
          <div class="poster-wrap">
            {#if entry.image_url}
              <img class="poster" src={entry.image_url} alt={entry.english_title ?? entry.title} loading="lazy" />
            {:else}
              <div class="poster placeholder"></div>
            {/if}
            <div class="poster-glow"></div>
            {#if entry.format}
              <span class="format-badge">{entry.format}</span>
            {/if}
          </div>
          <div class="meta">
            <span class="title">{entry.english_title ?? entry.title}</span>
          </div>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .calendar-root {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .season-bar {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .arrow {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 2rem;
    height: 2rem;
    border: 1px solid rgb(255 255 255 / 10%);
    border-radius: 50%;
    background: transparent;
    color: var(--color-muted);
    cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }

  .arrow:hover {
    background: rgb(255 255 255 / 8%);
    color: var(--color-text);
    border-color: rgb(255 255 255 / 18%);
  }

  .season-label {
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--color-text);
    letter-spacing: -0.01em;
    min-width: 8rem;
    text-align: center;
  }

  .status-msg {
    color: var(--color-muted);
    font-size: 0.9rem;
    padding: 2rem 0;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 1.25rem;
  }

  @media (max-width: 1100px) {
    .grid {
      grid-template-columns: repeat(3, 1fr);
    }
  }

  @media (max-width: 780px) {
    .grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    cursor: pointer;
    outline: none;
    border: none;
    background: none;
    font: inherit;
    text-align: left;
    color: inherit;
    width: 100%;
    min-width: 0;
    border-radius: var(--radius-card);
    padding: 0.4rem;
    transition: transform 0.2s ease;
  }

  .card:hover,
  .card:focus-visible {
    transform: scale(1.03);
  }

  .poster-wrap {
    position: relative;
    border-radius: calc(var(--radius-card) - 4px);
    overflow: hidden;
    aspect-ratio: 2 / 3;
    background: #0d1018;
  }

  .poster {
    width: 100%;
    height: 100%;
    max-width: 100%;
    object-fit: cover;
    display: block;
  }

  .placeholder {
    width: 100%;
    height: 100%;
    background: linear-gradient(145deg, rgb(255 255 255 / 6%), rgb(255 255 255 / 2%));
  }

  .poster-glow {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0%);
    transition: box-shadow 0.25s ease;
    pointer-events: none;
  }

  .card:hover .poster-glow,
  .card:focus-visible .poster-glow {
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 14%), 0 0 24px rgb(143 183 255 / 12%);
  }

  .format-badge {
    position: absolute;
    bottom: 0.5rem;
    right: 0.5rem;
    padding: 0.2rem 0.5rem;
    border-radius: 999px;
    background: rgb(0 0 0 / 60%);
    backdrop-filter: blur(6px);
    color: var(--color-text);
    font-size: 0.65rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    pointer-events: none;
  }

  .meta {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0 0.2rem;
  }

  .title {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--color-text);
    line-height: 1.2;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
