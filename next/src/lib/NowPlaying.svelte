<script lang="ts">
  import ErrorNotice from './ErrorNotice.svelte';
  import EpisodeGrid from './EpisodeGrid.svelte';
  import { explainError, type ExplainedError } from './errors';
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';
  import { confirmIdentification, setTrackingEnabled, type EngineEvent, type PlaybackDetectedEvent } from './api';
  import { playFile } from './notify';
  import { buildEpisodeTiles, playbackControls, type EpisodeTile } from './episodePlayback';
  import {
    currentShow,
    flatTiles,
    noteEngineEvents,
    playingEpisode,
    pollTrackingStatus,
    showTitle,
    startPlaybackPolling,
    trackingPollError,
    trackingStatus,
  } from './playback';

  export let events: EngineEvent[] = [];
  export let collapsed = false;

  const dispatch = createEventDispatcher<{ open: void }>();

  $: status = $trackingStatus;
  let lastEvent: string | null = null;
  // The status poll clears its own failure once it succeeds again; a failed
  // button action stays until dismissed or retried.
  type ActionError = ExplainedError & { retry?: () => void };
  let actionError: ActionError | null = null;
  $: shownError = actionError ?? $trackingPollError;
  let loading = false;
  let lastPlaybackEvent: PlaybackDetectedEvent['PlaybackDetected'] | null = null;
  let playbackCandidates: PlaybackDetectedEvent['PlaybackDetected']['candidates'] | null = null;
  let confirming: number | null = null;
  let stopPolling: (() => void) | null = null;

  // High-confidence / already-mapped matches are auto-tracked by the engine, so
  // no manual Confirm is needed.
  $: topCandidate = playbackCandidates && playbackCandidates.length > 0 ? playbackCandidates[0] : null;
  $: autoTracked = topCandidate ? (topCandidate.confidence >= 80 || topCandidate.match_source === 'file_index') : false;

  $: {
    const last = events.at(-1);
    if (last) {
      if ('PlaybackDetected' in last) {
        const pd = last.PlaybackDetected;
        lastPlaybackEvent = pd;
        playbackCandidates = pd.candidates;
        lastEvent = `Detected: ${pd.player_name}${pd.episode_guess ? ` ep ${pd.episode_guess}` : ''}`;
      } else if ('ProgressAdvanced' in last) {
        const pa = last.ProgressAdvanced;
        lastEvent = `Progress: anime ${pa.anime_id} ep ${pa.new_episode}`;
      }
    }
  }
  $: noteEngineEvents(events);

  // The identified show (playing now, or the last one played).
  $: show = $currentShow;
  $: identified = status.watching?.anime_id != null;
  $: playing = $playingEpisode;
  $: controls = show
    ? playbackControls(show.files, show.detail.watched_episodes ?? 0, show.detail.recent_history ?? [], playing, show.detail.episode_count ?? null)
    : null;
  $: stripTiles = show && controls
    ? flatTiles(buildEpisodeTiles(show.files, {
        watched: show.detail.watched_episodes ?? 0,
        episodeCount: show.detail.episode_count ?? null,
        playingEpisode: playing,
        nextEpisode: controls.next?.episode ?? null,
      }))
    : [];
  $: showLine = playing != null
    ? `Ep ${playing} · ${status.watching?.player_name ?? 'Playing'}`
    : controls?.current != null
      ? `Last watched Ep ${controls.current}`
      : 'Not playing';

  $: dotActive = status.active && status.watching !== null;
  $: dotTitle = status.watching
    ? (show && playing != null ? `${showTitle(show.detail)} · Ep ${playing}` : lastEvent ?? `Tracking ${status.watching.player_name}`)
    : status.active
      ? 'Waiting for playback…'
      : 'Tracking stopped';

  function playTile(e: CustomEvent<EpisodeTile>) {
    if (e.detail.file) playFile(e.detail.file.file_path);
  }

  async function handleStart() {
    loading = true;
    actionError = null;
    try {
      await setTrackingEnabled(true);
      await pollTrackingStatus();
    } catch (e) {
      actionError = { ...explainError("Tracking couldn't be started.", e), retry: handleStart };
    } finally {
      loading = false;
    }
  }

  async function handleConfirm(animeId: number) {
    const filePath = status.watching?.file_path;
    const episode = status.watching?.episode_guess ?? 0;
    if (!filePath || episode <= 0) return;
    confirming = animeId;
    actionError = null;
    try {
      await confirmIdentification(filePath, animeId, episode);
      lastEvent = `Confirmed: anime ${animeId} ep ${episode}`;
    } catch (e) {
      actionError = { ...explainError("That show couldn't be confirmed as what's playing.", e), retry: () => handleConfirm(animeId) };
    } finally {
      confirming = null;
    }
  }

  async function handleStop() {
    loading = true;
    actionError = null;
    try {
      await setTrackingEnabled(false);
      trackingStatus.set({ active: false, watching: null });
    } catch (e) {
      actionError = { ...explainError("Tracking couldn't be stopped.", e), retry: handleStop };
    } finally {
      loading = false;
    }
  }

  // Tracking auto-starts on the backend; the shared poll reflects it.
  onMount(() => { stopPolling = startPlaybackPolling(); });
  onDestroy(() => stopPolling?.());
</script>

{#if collapsed}
  <div class="np-dot-wrap" title={dotTitle}>
    <span class="np-dot" class:active={dotActive}></span>
  </div>
{:else}
<section class="now-playing-card">
  <div class="np-header">
    <p class="eyebrow">Now Playing</p>
    {#if status.active}
      <button type="button" class="np-btn-stop" disabled={loading} on:click={handleStop}>Stop tracking</button>
    {:else}
      <button type="button" class="np-btn-start" disabled={loading} on:click={handleStart}>Start tracking</button>
    {/if}
  </div>

  {#if shownError}
    <ErrorNotice
      compact
      message={shownError.message}
      details={shownError.details}
      actionLabel="Try again"
      on:action={() => (actionError ? actionError.retry?.() : pollTrackingStatus())}
      on:dismiss={() => { if (actionError) actionError = null; else trackingPollError.set(null); }}
    />
  {/if}

  {#if show && controls}
    <div class="np-show">
      <button type="button" class="np-show-top" on:click={() => dispatch('open')} title="Open Now Playing">
        {#if show.detail.image_url}
          <img class="np-cover" src={show.detail.image_url} alt="" loading="lazy" />
        {:else}
          <span class="np-cover placeholder" aria-hidden="true"></span>
        {/if}
        <span class="np-show-text">
          <span class="np-show-title">{showTitle(show.detail)}</span>
          <span class="np-show-line" class:live={playing != null}>{showLine}</span>
        </span>
      </button>
      {#if stripTiles.length > 0}
        <EpisodeGrid compact tiles={stripTiles} on:play={playTile} />
      {/if}
      <div class="np-ctrl">
        <button type="button" class="np-ctl" disabled={!controls.prev} title={controls.prev ? `Play episode ${controls.prev.episode}` : 'No earlier episode on disk'} on:click={() => controls?.prev && playFile(controls.prev.file_path)}>
          ◀ {controls.prev ? `Ep ${controls.prev.episode}` : 'Prev'}
        </button>
        <button type="button" class="np-ctl primary" disabled={!controls.next} title={controls.next ? `Play episode ${controls.next.episode}` : 'No later episode on disk'} on:click={() => controls?.next && playFile(controls.next.file_path)}>
          {controls.next ? `Ep ${controls.next.episode}` : 'Next'} ▶
        </button>
        <button type="button" class="np-ctl" disabled={!controls.currentFile} title={controls.current != null ? `Play episode ${controls.current} again` : 'Nothing to replay'} on:click={() => controls?.currentFile && playFile(controls.currentFile.file_path)}>
          Replay
        </button>
      </div>
      <button type="button" class="np-open" on:click={() => dispatch('open')}>Open Now Playing →</button>
    </div>
  {/if}

  {#if status.watching && !identified}
    <div class="np-meta">
      <span class="np-chip">{status.watching.player_name}</span>
      {#if status.watching.episode_guess}
        <span class="np-chip accent">Ep {status.watching.episode_guess}</span>
      {/if}
    </div>
    {#if status.watching.window_title || status.watching.file_path}
      <p class="np-filename" title={status.watching.file_path ?? status.watching.window_title}>
        {status.watching.window_title ?? status.watching.file_path}
      </p>
    {/if}

    {#if autoTracked && topCandidate}
      <div class="np-autotrack">
        <span class="np-autotrack-icon">✓</span>
        <div class="np-autotrack-info">
          <span class="np-autotrack-title">{topCandidate.title}</span>
          <span class="np-autotrack-sub">Auto-tracking · {topCandidate.confidence}% match</span>
        </div>
      </div>
    {:else if playbackCandidates && playbackCandidates.length > 0}
      <div class="np-candidates">
        <p class="np-candidates-label">Match candidates:</p>
        {#each playbackCandidates.slice(0, 5) as c}
          <div class="np-candidate">
            <span class="candidate-title">{c.title}</span>
            <span class="candidate-confidence">{c.confidence}%</span>
            <button class="confirm-btn"
              on:click|stopPropagation={() => handleConfirm(c.anime_id)}
              disabled={confirming !== null}
            >
              {confirming === c.anime_id ? '...' : 'Confirm'}
            </button>
          </div>
        {/each}
      </div>
    {:else if lastPlaybackEvent}
      <p class="np-idle">No library matches found. Import your library from AniList first.</p>
    {/if}
  {:else if !show && status.active}
    <p class="np-idle" aria-live="polite">Waiting for playback…</p>
  {:else if !status.active}
    <p class="np-idle" aria-live="polite">Tracking stopped.</p>
  {/if}

  {#if lastEvent && !show}
    <p class="np-event" aria-live="polite">{lastEvent}</p>
  {/if}
</section>
{/if}

<style>
  .now-playing-card {
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
    border-radius: var(--radius-card);
    padding: 1rem;
    background: rgba(255, 255, 255, 0.04);
    display: grid;
    gap: 0.6rem;
    min-width: 0;
    max-width: 100%;
    overflow: hidden;
  }

  .now-playing-card * {
    min-width: 0;
  }

  .np-dot-wrap {
    display: flex;
    justify-content: center;
    padding: 0.4rem 0;
  }

  .np-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--color-muted);
  }

  .np-dot.active {
    background: var(--color-success);
    box-shadow: 0 0 6px rgba(var(--color-success-rgb), 0.6);
  }

  .np-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  .np-header .eyebrow, .np-btn-start, .np-btn-stop {
    white-space: nowrap;
  }

  .eyebrow {
    color: var(--color-accent);
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.78rem;
    font-weight: 800;
  }

  .np-btn-start, .np-btn-stop {
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    border-radius: 999px;
    padding: 0.5rem 0.85rem;
    font-size: 0.78rem;
    cursor: pointer;
  }

  .np-btn-start {
    background: rgba(var(--color-accent-rgb), 0.18);
    color: #e9eefc;
  }

  .np-btn-start:hover {
    background: rgba(var(--color-accent-rgb), 0.28);
  }

  .np-btn-start:focus {
    outline: 2px solid rgba(var(--color-accent-rgb), 0.5);
    outline-offset: 2px;
  }

  .np-btn-stop {
    background: rgba(var(--color-error-rgb), 0.15);
    border-color: rgba(var(--color-error-rgb), 0.35);
    color: var(--color-error);
  }

  .np-btn-stop:hover {
    background: rgba(var(--color-error-rgb), 0.25);
  }

  .np-btn-stop:focus {
    outline: 2px solid rgba(var(--color-error-rgb), 0.5);
    outline-offset: 2px;
  }

  .np-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.35rem;
  }

  .np-chip {
    font-size: 0.68rem;
    padding: 0.12rem 0.5rem;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--color-muted);
    white-space: nowrap;
  }

  .np-chip.accent {
    background: rgba(var(--color-accent-rgb), 0.15);
    color: var(--color-accent);
    font-weight: 600;
  }

  .np-filename {
    margin: 0;
    font-size: 0.72rem;
    color: var(--color-muted);
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
    line-height: 1.35;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .np-idle, .np-event {
    color: var(--color-muted);
    font-size: 0.85rem;
  }

  .np-autotrack {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.6rem;
    padding: 0.5rem 0.6rem;
    border: 1px solid rgba(var(--color-success-rgb), 0.3);
    border-radius: 8px;
    background: rgba(var(--color-success-rgb), 0.08);
  }
  .np-autotrack-icon { color: var(--color-success); font-weight: 700; }
  .np-autotrack-info { display: flex; flex-direction: column; min-width: 0; }
  .np-autotrack-title { font-size: 0.82rem; color: var(--color-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .np-autotrack-sub { font-size: 0.72rem; color: var(--color-success); }

  .np-candidates {
    margin-top: 0.25rem;
  }

  .np-candidates-label {
    color: var(--color-muted);
    font-size: 0.78rem;
    margin-bottom: 0.4rem;
  }

  .np-candidate {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-areas:
      'title conf'
      'title btn';
    align-items: center;
    gap: 0.1rem 0.4rem;
    padding: 0.4rem 0.5rem;
    border: 1px solid rgba(var(--color-accent-rgb), 0.2);
    border-radius: 6px;
    margin-bottom: 0.3rem;
    font-size: 0.78rem;
  }

  .candidate-title {
    grid-area: title;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    line-height: 1.25;
  }

  .candidate-confidence {
    grid-area: conf;
    color: var(--color-accent);
    font-weight: 600;
    font-size: 0.72rem;
    justify-self: end;
  }

  .confirm-btn { grid-area: btn; justify-self: end; }

  .confirm-btn {
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    border-radius: 4px;
    padding: 0.15rem 0.5rem;
    background: rgba(var(--color-accent-rgb), 0.15);
    color: var(--color-accent);
    cursor: pointer;
    font-size: 0.75rem;
  }
  .confirm-btn:hover {
    background: rgba(var(--color-accent-rgb), 0.25);
  }
  .confirm-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .np-show { display: grid; gap: 0.55rem; min-width: 0; }
  .np-show-top {
    display: grid; grid-template-columns: 42px minmax(0, 1fr); gap: 0.6rem; align-items: center;
    background: none; border: 0; padding: 0; color: inherit; text-align: left; cursor: pointer; font: inherit;
  }
  .np-show-top:focus-visible, .np-open:focus-visible, .np-ctl:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; border-radius: 8px; }
  .np-cover { width: 42px; aspect-ratio: 0.7; object-fit: cover; border-radius: 7px; background: var(--color-surface-raised); display: block; }
  .np-show-text { display: grid; gap: 0.1rem; min-width: 0; }
  .np-show-title {
    font-weight: 700; font-size: 0.84rem; line-height: 1.25; color: var(--color-text);
    display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
  }
  .np-show-line { font-size: 0.72rem; color: var(--color-muted); }
  .np-show-line.live { color: var(--color-success); }
  .np-show-line.live::before { content: '● '; }
  .np-ctrl { display: grid; grid-template-columns: 1fr 1.25fr 1fr; gap: 0.3rem; }
  .np-ctl {
    border: 1px solid rgba(var(--color-accent-rgb), 0.3); border-radius: 999px; background: transparent;
    color: var(--color-text); font-size: 0.72rem; padding: 0.35rem 0.3rem; cursor: pointer; white-space: nowrap;
  }
  .np-ctl:hover:not(:disabled) { background: rgba(var(--color-accent-rgb), 0.14); }
  .np-ctl.primary { background: var(--color-accent); border-color: var(--color-accent); color: var(--color-bg); font-weight: 700; }
  .np-ctl.primary:hover:not(:disabled) { background: var(--color-accent); filter: brightness(1.08); }
  .np-ctl:disabled { opacity: 0.4; cursor: default; }
  .np-open { background: none; border: 0; color: var(--color-accent); font-size: 0.74rem; cursor: pointer; padding: 0.1rem; justify-self: center; }
  .np-open:hover { text-decoration: underline; }
</style>
