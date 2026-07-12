<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { confirmIdentification, getTrackingStatus, startTracking, stopTracking, type TrackingStatus, type EngineEvent, type PlaybackDetectedEvent } from './api';

  export let events: EngineEvent[] = [];
  export let collapsed = false;

  let status: TrackingStatus = { active: false, watching: null };
  let lastEvent: string | null = null;
  let error: string | null = null;
  let intervalId: ReturnType<typeof setInterval> | null = null;
  let loading = false;
  let lastPlaybackEvent: PlaybackDetectedEvent['PlaybackDetected'] | null = null;
  let playbackCandidates: PlaybackDetectedEvent['PlaybackDetected']['candidates'] | null = null;
  let confirming: number | null = null;

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

  $: dotActive = status.active && status.watching !== null;
  $: dotTitle = status.watching
    ? (lastEvent ?? `Tracking ${status.watching.player_name}`)
    : status.active
      ? 'Waiting for playback…'
      : 'Tracking stopped';

  async function poll() {
    if (loading) return;
    loading = true;
    try {
      status = await getTrackingStatus();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function startPolling() {
    poll();
    if (intervalId) clearInterval(intervalId);
    intervalId = setInterval(poll, 2000);
  }

  function stopPolling() {
    if (intervalId) {
      clearInterval(intervalId);
      intervalId = null;
    }
  }

  async function handleStart() {
    loading = true;
    try {
      await startTracking();
      startPolling();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function handleConfirm(animeId: number) {
    const filePath = status.watching?.file_path;
    const episode = status.watching?.episode_guess ?? 0;
    if (!filePath || episode <= 0) return;
    confirming = animeId;
    try {
      await confirmIdentification(filePath, animeId, episode);
      lastEvent = `Confirmed: anime ${animeId} ep ${episode}`;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      confirming = null;
    }
  }

  async function handleStop() {
    loading = true;
    stopPolling();
    try {
      await stopTracking();
      status = { active: false, watching: null };
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  // Tracking auto-starts on the backend; poll immediately so the panel reflects it.
  onMount(startPolling);
  onDestroy(stopPolling);
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

  {#if error}
    <p class="error" aria-live="polite">{error}</p>
  {/if}

  {#if status.watching}
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
  {:else if status.active}
    <p class="np-idle" aria-live="polite">Waiting for playback…</p>
  {:else}
    <p class="np-idle" aria-live="polite">Tracking stopped.</p>
  {/if}

  {#if lastEvent}
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
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .np-idle, .np-event {
    color: var(--color-muted);
    font-size: 0.85rem;
  }

  .error {
    color: var(--color-error);
    font-size: 0.82rem;
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
</style>
