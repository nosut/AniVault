<script lang="ts">
  import { onMount, onDestroy, createEventDispatcher } from 'svelte';
  import {
    getLibraryStats, getContinueWatching, getReadyToWatch, getCalendar,
    getAniListConnectionStatus, getSyncStatus,
    type LibraryStats, type ContinueWatchingEntry, type ReadyToWatchEntry,
    type CalendarEntry, type AniListSyncStatus, type EngineEvent,
  } from './api';
  import { episodeMarker } from './calendarUi';
  import { airedAgoShort, isSameLocalDay, syncPill, todayRowLabel } from './homeUi';
  import RecognitionCard from './RecognitionCard.svelte';

  const dispatch = createEventDispatcher<{ select: { anime_id: number }; navigate: { view: string } }>();

  export let events: EngineEvent[] = [];

  let stats: LibraryStats | null = null;
  let continueEntries: ContinueWatchingEntry[] = [];
  let readyEntries: ReadyToWatchEntry[] = [];
  let calendarEntries: CalendarEntry[] = [];
  let connected = false;
  let syncStatus: AniListSyncStatus | null = null;
  let loading = true;

  let now = Math.floor(Date.now() / 1000);
  let ticker: ReturnType<typeof setInterval>;

  async function load() {
    loading = true;
    // Each source is independent; a failure in one leaves the others alive.
    const [statsR, contR, readyR, calR, connR, syncR] = await Promise.allSettled([
      getLibraryStats(), getContinueWatching(), getReadyToWatch(), getCalendar(),
      getAniListConnectionStatus(), getSyncStatus(),
    ]);
    if (statsR.status === 'fulfilled') stats = statsR.value;
    if (contR.status === 'fulfilled') continueEntries = contR.value;
    if (readyR.status === 'fulfilled') readyEntries = readyR.value;
    if (calR.status === 'fulfilled') calendarEntries = calR.value;
    if (connR.status === 'fulfilled') connected = connR.value;
    if (syncR.status === 'fulfilled') syncStatus = syncR.value;
    loading = false;
  }

  onMount(() => {
    load();
    ticker = setInterval(() => { now = Math.floor(Date.now() / 1000); }, 1000);
  });
  onDestroy(() => clearInterval(ticker));

  $: pill = syncPill(connected, syncStatus);

  $: todayEntries = calendarEntries
    .filter(e => e.airing_at != null && isSameLocalDay(e.airing_at, now))
    .sort((a, b) => (a.airing_at ?? 0) - (b.airing_at ?? 0));

  // Next upcoming episode after today, for the "nothing airs today" hint.
  $: nextUpcoming = calendarEntries
    .filter(e => e.airing_at != null && e.airing_at > now && !isSameLocalDay(e.airing_at, now))
    .sort((a, b) => (a.airing_at ?? 0) - (b.airing_at ?? 0))[0] ?? null;

  $: missingEntries = calendarEntries
    .filter(e => e.airing_at != null && e.airing_at <= now && !e.has_file)
    .sort((a, b) => (a.airing_at ?? 0) - (b.airing_at ?? 0))
    .slice(0, 8);

  // anime_id → ready info, for the "Ep N ready" label on continue cards.
  $: readyById = new Map(readyEntries.map(r => [r.anime_id, r]));
  // (anime_id, episode) aired without a file, for the "not downloaded" label.
  $: missingSet = new Set(
    calendarEntries
      .filter(e => e.airing_at != null && e.airing_at <= now && !e.has_file && e.next_episode != null)
      .map(e => `${e.anime_id}:${e.next_episode}`),
  );

  function nextEpLabel(entry: ContinueWatchingEntry): { text: string; ready: boolean } | null {
    const ready = readyById.get(entry.anime_id);
    if (ready) return { text: `Ep ${ready.next_episode} ready ▸`, ready: true };
    const next = entry.watched_episodes + 1;
    if (missingSet.has(`${entry.anime_id}:${next}`)) return { text: `Ep ${next} not downloaded`, ready: false };
    return null;
  }

  function select(animeId: number) {
    if (animeId > 0) dispatch('select', { anime_id: animeId });
  }

  const dayFmt = new Intl.DateTimeFormat(undefined, { weekday: 'long', month: 'long', day: 'numeric' });
  const shortDayFmt = new Intl.DateTimeFormat(undefined, { weekday: 'short' });
</script>

<div class="home">
  <header class="home-head">
    <h1>Home</h1>
    <div class="head-right">
      <button class="pill" on:click={() => dispatch('navigate', { view: 'settings' })}>
        AniList <span class="pill-state" class:ok={pill.ok} class:bad={!pill.ok}>● {pill.text}</span>
      </button>
      {#if stats}
        <span class="pill">{stats.total} in library</span>
      {/if}
    </div>
  </header>

  <RecognitionCard {events} />

  <section data-testid="jump-back-in">
    <h3>Jump back in</h3>
    {#if loading}
      <div class="skeleton-row"></div>
    {:else if continueEntries.length === 0}
      <p class="muted">No anime in progress. Start watching to see them here.</p>
    {:else}
      <div class="cw-grid">
        {#each continueEntries as entry (entry.anime_id)}
          {@const label = nextEpLabel(entry)}
          <div class="cw-card" tabindex="0" role="button" on:click={() => select(entry.anime_id)} on:keydown={(e) => e.key === 'Enter' && select(entry.anime_id)}>
            {#if entry.image_url}
              <img class="thumb" src={entry.image_url} alt={entry.anime_title} loading="lazy" />
            {:else}
              <div class="thumb placeholder"></div>
            {/if}
            <div class="cw-info">
              <p class="cw-title">{entry.anime_title}</p>
              <div class="bar"><i style="width: {entry.episode_count ? Math.min(100, entry.watched_episodes / entry.episode_count * 100) : 0}%"></i></div>
              <div class="cw-meta">
                <span>{entry.watched_episodes} / {entry.episode_count ?? '?'}</span>
                {#if label}
                  <span class:next-up={label.ready} class:muted={!label.ready}>{label.text}</span>
                {/if}
              </div>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <section data-testid="airing-today">
    <h3>Airing today <span class="count">{dayFmt.format(new Date(now * 1000))}</span></h3>
    {#if loading}
      <div class="skeleton-row"></div>
    {:else if todayEntries.length === 0}
      <p class="muted">
        Nothing airs today{#if nextUpcoming} — next: {nextUpcoming.title}, {shortDayFmt.format(new Date((nextUpcoming.airing_at ?? 0) * 1000))}{/if}
      </p>
    {:else}
      <div class="today-list">
        {#each todayEntries as entry (`${entry.anime_id}:${entry.next_episode}`)}
          {@const marker = episodeMarker(entry, now)}
          <button class="today-row" on:click={() => select(entry.anime_id)}>
            <span class="dot {marker}"></span>
            <span class="today-title">{entry.title}</span>
            {#if entry.next_episode}<span class="today-ep">Ep {entry.next_episode}</span>{/if}
            {#if entry.airing_at != null}
              <span class="when" class:soon={entry.airing_at > now}>{todayRowLabel(entry.airing_at, now)}</span>
            {/if}
          </button>
        {/each}
      </div>
    {/if}
  </section>

  <div class="cols">
    <section data-testid="ready-to-watch">
      <h3>Ready to watch <span class="count">unwatched episodes in your library</span></h3>
      {#if loading}
        <div class="skeleton-row"></div>
      {:else if readyEntries.length === 0}
        <p class="muted">Nothing queued up — new downloads show here when the next unwatched episode is on disk.</p>
      {:else}
        <div class="ready-grid">
          {#each readyEntries as entry (entry.anime_id)}
            <div class="ready-card" tabindex="0" role="button" on:click={() => select(entry.anime_id)} on:keydown={(e) => e.key === 'Enter' && select(entry.anime_id)}>
              {#if entry.image_url}
                <img class="thumb" src={entry.image_url} alt={entry.title} loading="lazy" />
              {:else}
                <div class="thumb placeholder"></div>
              {/if}
              <div class="cw-info">
                <p class="cw-title">{entry.title}</p>
                <div class="cw-meta">
                  <span>{entry.ready_count === 1 ? 'Next unwatched' : `${entry.ready_count} episodes ready`}</span>
                </div>
              </div>
              <span class="ep-chip">Ep {entry.next_episode}</span>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <section data-testid="missing-downloads">
      <h3>Missing downloads <span class="count">aired, not in library</span></h3>
      {#if loading}
        <div class="skeleton-row"></div>
      {:else if missingEntries.length === 0}
        <p class="muted">All caught up — every aired episode is in your library.</p>
      {:else}
        <div class="missing-list">
          {#each missingEntries as entry (`${entry.anime_id}:${entry.next_episode}`)}
            <button class="missing-row" on:click={() => select(entry.anime_id)}>
              <span class="dot missing"></span>
              <span class="missing-title">{entry.title}</span>
              {#if entry.next_episode}<span class="missing-ep">Ep {entry.next_episode}</span>{/if}
              {#if entry.airing_at != null}<span class="aired-ago">{airedAgoShort(entry.airing_at, now)}</span>{/if}
            </button>
          {/each}
        </div>
      {/if}
    </section>
  </div>
</div>

<style>
  .home { display: flex; flex-direction: column; gap: 1.5rem; padding: 1.25rem; }

  .home-head { display: flex; align-items: center; justify-content: space-between; gap: 1rem; flex-wrap: wrap; }
  .home-head h1 { margin: 0; font-size: 1.25rem; font-weight: 700; letter-spacing: -0.01em; }
  .head-right { display: flex; align-items: center; gap: 0.6rem; }
  .pill { font-size: 0.75rem; padding: 0.25rem 0.7rem; border-radius: 999px; border: 1px solid rgba(var(--color-accent-rgb),0.25); color: var(--color-muted); background: transparent; }
  button.pill { cursor: pointer; }
  button.pill:hover { background: rgba(var(--color-accent-rgb),0.1); }
  .pill-state.ok { color: var(--color-success); }
  .pill-state.bad { color: var(--color-warning); }

  h3 { font-size: 0.95rem; margin: 0 0 0.6rem; }
  .count { color: var(--color-muted); font-weight: 400; font-size: 0.85rem; margin-left: 0.4rem; }
  .muted { color: var(--color-muted); font-size: 0.85rem; }

  .cw-grid, .ready-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr)); gap: 0.7rem; }
  .cw-card, .ready-card { display: flex; gap: 0.7rem; padding: 0.55rem; border: 1px solid rgba(var(--color-accent-rgb),0.12); border-radius: 10px; background: rgba(255,255,255,0.03); cursor: pointer; transition: border-color 0.15s; }
  .cw-card:hover, .ready-card:hover { border-color: rgba(var(--color-accent-rgb),0.3); }
  .ready-card { border-color: rgba(var(--color-success-rgb),0.25); background: rgba(var(--color-success-rgb),0.05); }
  .ready-card:hover { border-color: rgba(var(--color-success-rgb),0.55); }

  .thumb { width: 3rem; height: 4.2rem; border-radius: 6px; object-fit: cover; flex-shrink: 0; }
  .thumb.placeholder { background: rgba(var(--color-accent-rgb),0.12); }
  .cw-info { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 0.3rem; justify-content: center; }
  .cw-title { font-size: 0.85rem; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; margin: 0; }
  .bar { height: 0.3rem; border-radius: 2px; background: rgba(255,255,255,0.08); overflow: hidden; }
  .bar i { display: block; height: 100%; background: rgba(var(--color-accent-rgb),0.55); }
  .cw-meta { display: flex; justify-content: space-between; gap: 0.5rem; font-size: 0.72rem; color: var(--color-muted); }
  .next-up { color: var(--color-success); font-weight: 600; }
  .ep-chip { align-self: center; flex-shrink: 0; font-size: 0.72rem; font-weight: 700; color: var(--color-success); background: rgba(var(--color-success-rgb),0.15); border-radius: 999px; padding: 0.2rem 0.55rem; }

  .today-list, .missing-list { display: flex; flex-direction: column; gap: 0.45rem; }
  .today-row, .missing-row { display: flex; align-items: center; gap: 0.7rem; width: 100%; text-align: left; padding: 0.5rem 0.65rem; border: 1px solid rgba(var(--color-accent-rgb),0.1); border-radius: 10px; background: rgba(var(--color-accent-rgb),0.03); color: var(--color-text); font-size: 0.85rem; cursor: pointer; }
  .today-row:hover, .missing-row:hover { border-color: rgba(var(--color-accent-rgb),0.3); }
  .dot { width: 0.5rem; height: 0.5rem; border-radius: 50%; flex-shrink: 0; box-sizing: border-box; }
  .dot.have { background: var(--color-success); }
  .dot.missing { background: transparent; border: 1.5px solid var(--color-warning); }
  .dot.future { background: rgba(var(--color-accent-rgb),0.25); }
  .today-title, .missing-title { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .today-ep { color: var(--color-muted); flex-shrink: 0; }
  .when { flex-shrink: 0; font-size: 0.75rem; font-variant-numeric: tabular-nums; color: var(--color-muted); padding: 0.15rem 0.55rem; border-radius: 999px; border: 1px solid rgba(var(--color-accent-rgb),0.15); }
  .when.soon { color: var(--color-accent); border-color: rgba(var(--color-accent-rgb),0.4); }

  .missing-row { border-color: rgba(var(--color-warning-rgb),0.25); background: rgba(var(--color-warning-rgb),0.05); }
  .missing-row:hover { border-color: rgba(var(--color-warning-rgb),0.5); }
  .missing-ep { color: var(--color-warning); font-weight: 600; flex-shrink: 0; }
  .aired-ago { color: var(--color-muted); font-size: 0.72rem; flex-shrink: 0; }

  .cols { display: grid; grid-template-columns: 3fr 2fr; gap: 1.2rem; align-items: start; }

  .skeleton-row { height: 4.75rem; border-radius: 10px; background: rgba(255,255,255,0.04); animation: pulse 2s infinite; }
  @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.4; } }

  @media (max-width: 800px) {
    .cols { grid-template-columns: 1fr; }
  }
  @media (max-width: 480px) {
    .home { padding: 0.75rem; gap: 1rem; }
  }
</style>
