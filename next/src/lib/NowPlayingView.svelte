<script lang="ts">
  // The Now Playing page: the show that's playing (or played last), its
  // episodes as one-click tiles, and the facts you'd glance at mid-binge.
  import { createEventDispatcher, onDestroy, onMount } from 'svelte';
  import { ArrowLeft, Play, FolderOpen, RotateCcw } from 'lucide-svelte';
  import EpisodeGrid from './EpisodeGrid.svelte';
  import ErrorNotice from './ErrorNotice.svelte';
  import { formatBytes } from './fileSize';
  import { playFile, showInFolder } from './notify';
  import { getSeriesDiskSize } from './api';
  import { buildEpisodeTiles, continueTarget, fileName, playbackControls, type EpisodeTile } from './episodePlayback';
  import {
    currentShow,
    loadShow,
    playingEpisode,
    showError,
    showTitle,
    startPlaybackPolling,
    trackingStatus,
  } from './playback';

  const dispatch = createEventDispatcher<{ select: { anime_id: number }; back: void }>();

  $: show = $currentShow;
  $: playing = $playingEpisode;
  $: watched = show?.detail.watched_episodes ?? 0;
  $: count = show?.detail.episode_count ?? null;
  $: controls = show ? playbackControls(show.files, watched, show.detail.recent_history ?? [], playing) : null;
  $: seasons = show
    ? buildEpisodeTiles(show.files, {
        watched,
        episodeCount: count,
        playingEpisode: playing,
        nextEpisode: controls?.next?.episode ?? null,
      })
    : [];
  let seasonIndex = 0;
  // Follow the season of the playing episode when it changes.
  $: {
    const at = seasons.findIndex((s) => s.tiles.some((t) => t.state === 'playing'));
    if (at >= 0) seasonIndex = at;
    else if (seasonIndex >= seasons.length) seasonIndex = 0;
  }
  $: shownSeason = seasons[seasonIndex] ?? null;
  // A finished show you're playing again, or partway back through.
  $: rewatching = show != null && count != null && watched >= count
    && (playing != null || continueTarget(show.files, watched, show.detail.recent_history ?? [])?.kind === 'rewatch');
  $: filesOnDisk = show ? new Set(show.files.filter((f) => f.episode != null).map((f) => f.episode)).size : 0;
  $: progressPct = count ? Math.min(100, Math.round((watched / count) * 100)) : 0;
  $: playerName = playing != null ? $trackingStatus.watching?.player_name ?? null : null;
  $: playingFile = playing != null ? controls?.currentFile ?? null : null;

  let diskBytes: number | null = null;
  let diskFor: number | null = null;
  $: if (show && show.animeId !== diskFor) loadDisk(show.animeId);
  async function loadDisk(id: number) {
    diskFor = id;
    diskBytes = null;
    try {
      const bytes = await getSeriesDiskSize(id);
      if (diskFor === id) diskBytes = bytes;
    } catch {
      // The size is a nicety; leave it blank.
    }
  }

  function playTile(e: CustomEvent<EpisodeTile>) {
    if (e.detail.file) playFile(e.detail.file.file_path);
  }

  function formatStatus(status: string | null): string {
    const map: Record<string, string> = {
      watching: 'Watching',
      completed: 'Completed',
      on_hold: 'On hold',
      dropped: 'Dropped',
      plan_to_watch: 'Plan to watch',
    };
    return status ? map[status] ?? status : 'Not on your list';
  }

  function stripHtml(html: string): string {
    return html.replace(/<br\s*\/?>/gi, '\n').replace(/<[^>]*>/g, '').trim();
  }

  let stopPolling: (() => void) | null = null;
  onMount(() => { stopPolling = startPlaybackPolling(); });
  onDestroy(() => stopPolling?.());
</script>

<div class="np-view">
  <button type="button" class="back-btn" on:click={() => dispatch('back')} aria-label="Back">
    <ArrowLeft size={15} /> Back
  </button>
  {#if $showError && !show}
    <ErrorNotice message={$showError.message} details={$showError.details} actionLabel={null} on:dismiss={() => showError.set(null)} />
  {/if}

  {#if !show || !controls}
    <div class="empty">
      <h2>Nothing playing yet</h2>
      <p>Play an episode from a series page, or open a file in your player. AniVault picks it up within a few seconds and shows the show here.</p>
    </div>
  {:else}
    <section class="hero">
      {#if show.detail.image_url}
        <img class="cover" src={show.detail.image_url} alt="Cover for {showTitle(show.detail)}" />
      {:else}
        <div class="cover placeholder" aria-hidden="true"></div>
      {/if}
      <div class="info">
        <div class="chips">
          {#if playing != null}
            <span class="chip live">Playing{playerName ? ` in ${playerName}` : ''}</span>
          {:else}
            <span class="chip">Not playing</span>
          {/if}
          {#if rewatching}
            <span class="chip accent">Rewatching</span>
          {/if}
        </div>
        <h2 class="title">{showTitle(show.detail)}</h2>
        <p class="sub">
          {#if playing != null}
            Episode {playing}{count ? ` of ${count}` : ''}
          {:else if controls.current != null}
            Last watched episode {controls.current}
          {:else}
            You haven't watched any of this show in AniVault yet
          {/if}
        </p>
        {#if playingFile}
          <p class="file" title={playingFile.file_path}>{fileName(playingFile.file_path)}</p>
        {/if}
        <div class="progress">
          <div class="progress-row"><span>Your progress</span><span>{watched}{count ? ` / ${count}` : ''}</span></div>
          <div class="bar" role="progressbar" aria-label="Your progress" aria-valuemin="0" aria-valuemax={count ?? undefined} aria-valuenow={watched}>
            <span style="width: {progressPct}%"></span>
          </div>
        </div>
        <div class="actions">
          {#if controls.next}
            {@const next = controls.next}
            <button type="button" class="btn primary" on:click={() => playFile(next.file_path)}><Play size={15} /> Play Ep {next.episode}</button>
          {/if}
          {#if controls.currentFile}
            {@const again = controls.currentFile}
            <button type="button" class="btn" on:click={() => playFile(again.file_path)}><RotateCcw size={14} /> Replay Ep {controls.current}</button>
          {/if}
          {#if show.files.length > 0}
            {@const any = playingFile ?? controls.next ?? show.files[0]}
            {#if any}
              <button type="button" class="btn" on:click={() => showInFolder(any.file_path)}><FolderOpen size={14} /> Open folder</button>
            {/if}
          {/if}
        </div>
      </div>
    </section>

    <div class="body">
      <section class="panel">
        <div class="panel-head">
          <h3>Episodes</h3>
          {#if seasons.length > 1}
            <div class="seg" role="group" aria-label="Season">
              {#each seasons as s, i (s.season)}
                <button type="button" class:on={i === seasonIndex} aria-pressed={i === seasonIndex} on:click={() => (seasonIndex = i)}>Season {s.season}</button>
              {/each}
            </div>
          {/if}
        </div>
        {#if shownSeason}
          <EpisodeGrid tiles={shownSeason.tiles} on:play={playTile} />
          <div class="legend" aria-hidden="true">
            <span><i class="w"></i>Watched</span><span><i class="p"></i>Playing</span><span><i class="n"></i>Up next</span><span><i class="m"></i>No file</span>
          </div>
        {:else}
          <p class="muted">No episode files are mapped to this show. Use Rescan or Map folder on its series page.</p>
        {/if}
      </section>

      <aside class="panel">
        <h3>Series</h3>
        <dl class="kv">
          <div><dt>List status</dt><dd>{formatStatus(show.detail.list_status)}</dd></div>
          <div><dt>Score</dt><dd>{show.detail.score != null ? `${show.detail.score} / 10` : '—'}</dd></div>
          <div><dt>Files</dt><dd>{filesOnDisk}{count ? ` of ${count}` : ''}{diskBytes ? ` · ${formatBytes(diskBytes)}` : ''}</dd></div>
        </dl>
        {#if show.detail.synopsis}
          <p class="synopsis">{stripHtml(show.detail.synopsis)}</p>
        {/if}
        <div class="aside-actions">
          <button type="button" class="btn small" on:click={() => show && dispatch('select', { anime_id: show.animeId })}>Open series page</button>
          <button type="button" class="btn small ghost" on:click={() => show && loadShow(show.animeId)} title="Reload this show's episodes and progress">Refresh</button>
        </div>
      </aside>
    </div>
  {/if}
</div>

<style>
  .np-view { display: grid; gap: 1.1rem; min-width: 0; }

  .back-btn {
    display: inline-flex; align-items: center; gap: 0.35rem; justify-self: start;
    border: 1px solid rgba(var(--color-accent-rgb), 0.35); border-radius: 999px;
    padding: 0.45rem 0.9rem; background: rgba(var(--color-accent-rgb), 0.12);
    color: var(--color-text); cursor: pointer; font-size: 0.85rem;
  }
  .back-btn:hover { background: rgba(var(--color-accent-rgb), 0.22); }
  .back-btn:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }

  .empty { padding: 3rem 1rem; text-align: center; display: grid; gap: 0.5rem; justify-items: center; }
  .empty h2 { font-size: 1.3rem; font-weight: 700; }
  .empty p { color: var(--color-muted); max-width: 46ch; }

  .hero {
    display: grid;
    grid-template-columns: 170px minmax(0, 1fr);
    gap: 1.4rem;
    align-items: end;
    padding: 1.25rem;
    border-radius: var(--radius-card);
    border: 1px solid rgba(var(--color-accent-rgb), 0.16);
    background: linear-gradient(135deg, rgba(var(--color-accent-rgb), 0.14), transparent 70%), var(--color-surface);
  }
  .cover { width: 100%; aspect-ratio: 0.7; object-fit: cover; border-radius: 14px; background: var(--color-surface-raised); display: block; }
  .info { display: grid; gap: 0.6rem; min-width: 0; }
  .chips { display: flex; flex-wrap: wrap; gap: 0.4rem; }
  .chip { display: inline-flex; align-items: center; gap: 0.4rem; font-size: 0.74rem; padding: 0.2rem 0.65rem; border-radius: 999px; background: var(--color-surface-raised); color: var(--color-muted); }
  .chip.live { color: var(--color-success); }
  .chip.live::before { content: ''; width: 7px; height: 7px; border-radius: 50%; background: var(--color-success); box-shadow: 0 0 6px rgba(var(--color-success-rgb), 0.6); }
  .chip.accent { background: rgba(var(--color-accent-rgb), 0.14); color: var(--color-accent); }
  .title { font-size: 1.7rem; line-height: 1.15; font-weight: 800; text-wrap: balance; margin: 0; }
  .sub { color: var(--color-muted); margin: 0; }
  .file { margin: 0; font-size: 0.75rem; color: var(--color-muted); font-family: ui-monospace, Consolas, monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .progress { display: grid; gap: 0.35rem; max-width: 420px; font-size: 0.8rem; color: var(--color-muted); }
  .progress-row { display: flex; justify-content: space-between; font-variant-numeric: tabular-nums; }
  .bar { height: 6px; border-radius: 99px; background: var(--color-surface-raised); overflow: hidden; }
  .bar span { display: block; height: 100%; background: var(--color-accent); border-radius: 99px; }
  .actions { display: flex; flex-wrap: wrap; gap: 0.5rem; margin-top: 0.2rem; }

  .btn {
    display: inline-flex; align-items: center; gap: 0.4rem;
    border: 1px solid rgba(var(--color-accent-rgb), 0.35); border-radius: 999px;
    padding: 0.55rem 1rem; background: rgba(var(--color-accent-rgb), 0.08);
    color: var(--color-text); font-size: 0.86rem; cursor: pointer; white-space: nowrap;
  }
  .btn:hover { background: rgba(var(--color-accent-rgb), 0.2); }
  .btn.primary { background: var(--color-accent); border-color: var(--color-accent); color: var(--color-bg); font-weight: 700; }
  .btn.primary:hover { background: var(--color-accent); filter: brightness(1.08); }
  .btn.small { padding: 0.35rem 0.8rem; font-size: 0.78rem; }
  .btn.ghost { background: transparent; color: var(--color-muted); }
  .btn:focus-visible, .seg button:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }

  .body { display: grid; grid-template-columns: minmax(0, 1fr) 260px; gap: 1.1rem; align-items: start; }
  .panel {
    display: grid; gap: 0.85rem; min-width: 0; padding: 1rem;
    border-radius: 16px; border: 1px solid rgba(var(--color-accent-rgb), 0.12); background: rgba(255, 255, 255, 0.03);
  }
  .panel h3 { font-size: 1rem; font-weight: 700; margin: 0; }
  .panel-head { display: flex; justify-content: space-between; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
  .seg { display: inline-flex; gap: 2px; padding: 3px; border-radius: 999px; background: var(--color-surface-raised); }
  .seg button { border: 0; background: transparent; color: var(--color-muted); font-size: 0.75rem; padding: 0.2rem 0.75rem; border-radius: 999px; cursor: pointer; }
  .seg button.on { background: var(--color-bg); color: var(--color-text); }

  .legend { display: flex; flex-wrap: wrap; gap: 0.9rem; font-size: 0.72rem; color: var(--color-muted); }
  .legend i { display: inline-block; width: 11px; height: 11px; border-radius: 3px; margin-right: 0.35rem; vertical-align: -1px; }
  .legend i.w { background: rgba(255, 255, 255, 0.03); outline: 1px solid rgba(var(--color-success-rgb), 0.45); }
  .legend i.p { background: var(--color-accent); }
  .legend i.n { border: 1px solid var(--color-accent); }
  .legend i.m { border: 1px dashed rgba(154, 166, 184, 0.5); }

  .kv { display: grid; gap: 0.5rem; margin: 0; font-size: 0.84rem; }
  .kv div { display: flex; justify-content: space-between; gap: 0.6rem; }
  .kv dt { color: var(--color-muted); }
  .kv dd { margin: 0; text-align: right; font-variant-numeric: tabular-nums; }
  .synopsis {
    margin: 0; font-size: 0.82rem; color: var(--color-muted); white-space: pre-line;
    display: -webkit-box; -webkit-line-clamp: 8; line-clamp: 8; -webkit-box-orient: vertical; overflow: hidden;
  }
  .aside-actions { display: flex; flex-wrap: wrap; gap: 0.4rem; }
  .muted { color: var(--color-muted); font-size: 0.85rem; }

  @media (max-width: 960px) {
    .body { grid-template-columns: minmax(0, 1fr); }
  }
  @media (max-width: 600px) {
    .hero { grid-template-columns: minmax(0, 1fr); }
    .cover { max-width: 140px; }
  }
</style>
