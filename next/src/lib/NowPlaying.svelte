<script lang="ts">
  import { onDestroy } from 'svelte';
  import { confirmIdentification, getTrackingStatus, startTracking, stopTracking, type TrackingStatus, type EngineEvent, type PlaybackDetectedEvent } from './api';

  export let events: EngineEvent[] = [];

  let status: TrackingStatus = { active: false, watching: null };
  let lastEvent: string | null = null;
  let error: string | null = null;
  let intervalId: ReturnType<typeof setInterval> | null = null;
  let loading = false;
  let lastPlaybackEvent: PlaybackDetectedEvent['PlaybackDetected'] | null = null;
  let playbackCandidates: PlaybackDetectedEvent['PlaybackDetected']['candidates'] | null = null;
  let confirming: number | null = null;

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

  onDestroy(stopPolling);
</script>

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
    <dl class="np-details">
      <div>
        <dt>Player</dt>
        <dd>{status.watching.player_name}</dd>
      </div>
      {#if status.watching.window_title}
        <div>
          <dt>Title</dt>
          <dd>{status.watching.window_title}</dd>
        </div>
      {/if}
      {#if status.watching.file_path}
        <div>
          <dt>File</dt>
          <dd class="file-path">{status.watching.file_path}</dd>
        </div>
      {/if}
      {#if status.watching.episode_guess}
        <div>
          <dt>Episode</dt>
          <dd>{status.watching.episode_guess}</dd>
        </div>
      {/if}
    </dl>

    {#if playbackCandidates && playbackCandidates.length > 0}
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

<style>
  .now-playing-card {
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: var(--radius-card);
    padding: 1.25rem;
    background: rgba(255, 255, 255, 0.04);
    display: grid;
    gap: 0.75rem;
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
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 999px;
    padding: 0.5rem 0.85rem;
    font-size: 0.78rem;
    cursor: pointer;
  }

  .np-btn-start {
    background: rgba(143, 183, 255, 0.18);
    color: #e9eefc;
  }

  .np-btn-start:hover {
    background: rgba(143, 183, 255, 0.28);
  }

  .np-btn-start:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .np-btn-stop {
    background: rgba(255, 157, 157, 0.15);
    border-color: rgba(255, 157, 157, 0.35);
    color: #ff9d9d;
  }

  .np-btn-stop:hover {
    background: rgba(255, 157, 157, 0.25);
  }

  .np-btn-stop:focus {
    outline: 2px solid rgba(255, 157, 157, 0.5);
    outline-offset: 2px;
  }

  .np-details {
    display: grid;
    gap: 0.5rem;
  }

  .np-details div {
    display: grid;
    grid-template-columns: 5rem 1fr;
    gap: 0.25rem;
  }

  .np-details dt {
    color: var(--color-muted);
    font-size: 0.78rem;
  }

  .np-details dd {
    margin: 0;
  }

  .file-path {
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
    font-size: 0.75rem;
    overflow-wrap: anywhere;
  }

  .np-idle, .np-event {
    color: var(--color-muted);
    font-size: 0.85rem;
  }

  .error {
    color: var(--color-error, #ff9d9d);
    font-size: 0.82rem;
  }

  .np-candidates {
    margin-top: 0.25rem;
  }

  .np-candidates-label {
    color: var(--color-muted);
    font-size: 0.78rem;
    margin-bottom: 0.4rem;
  }

  .np-candidate {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.4rem 0.6rem;
    border: 1px solid rgba(143, 183, 255, 0.2);
    border-radius: 6px;
    margin-bottom: 0.3rem;
    font-size: 0.82rem;
  }

  .candidate-title {
    color: var(--color-text);
  }

  .candidate-confidence {
    color: var(--color-accent);
    font-weight: 600;
  }

  .confirm-btn {
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 4px;
    padding: 0.15rem 0.5rem;
    background: rgba(143, 183, 255, 0.15);
    color: var(--color-accent);
    cursor: pointer;
    font-size: 0.75rem;
  }
  .confirm-btn:hover {
    background: rgba(143, 183, 255, 0.25);
  }
  .confirm-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
