<script lang="ts">
  import { confirmIdentification, type MatchCandidate, type EngineEvent } from './api';

  export let events: EngineEvent[] = [];
  export let onConfirmed: () => void = () => {};

  let candidates: MatchCandidate[] = [];
  let filePath: string | null = null;
  let episodeGuess: number | null = null;
  let confirmed: string | null = null;
  let error: string | null = null;
  let loading = false;

  $: {
    for (const event of events) {
      if ('PlaybackDetected' in event) {
        const pd = event.PlaybackDetected;
        const [first] = pd.candidates;
        if (first && first.confidence < 60) {
          candidates = pd.candidates;
          filePath = pd.file_path;
          episodeGuess = pd.episode_guess;
          confirmed = null;
          error = null;
          break;
        }
      }
    }
  }

  async function confirm(animeId: number, episode: number) {
    if (!filePath || loading) return;
    loading = true;
    error = null;
    try {
      await confirmIdentification(filePath, animeId, episode);
      candidates = [];
      confirmed = `Confirmed: anime ${animeId} episode ${episode}`;
      onConfirmed();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }
</script>

{#if candidates.length > 0 || confirmed || error}
  <section class="recognition-card" aria-live="polite">
    <p class="eyebrow">Recognition</p>

    {#if error}
      <p class="error">{error}</p>
    {/if}

    {#if confirmed}
      <p class="confirm-msg">{confirmed}</p>
    {:else if candidates.length > 0}
      <p class="rc-hint">
        Detected: {filePath} — match?
      </p>
      <ul class="rc-list" role="list">
        {#each candidates.slice(0, 5) as candidate}
          <li class="rc-item" role="listitem">
            <span class="rc-title">{candidate.title}</span>
            <span class="rc-score">{candidate.confidence}%</span>
            <button
              class="rc-confirm"
              type="button"
              on:click={() => confirm(candidate.anime_id, episodeGuess ?? 1)}
              disabled={loading}
            >
              Confirm ep {episodeGuess ?? 1}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
{/if}

<style>
  .recognition-card {
    border: 1px solid rgba(255, 193, 100, 0.25);
    border-radius: var(--radius-card);
    background: rgba(255, 193, 100, 0.06);
    padding: 1.25rem;
    display: grid;
    gap: 0.75rem;
  }

  .eyebrow {
    color: #ffc164;
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.78rem;
    font-weight: 800;
  }

  .rc-hint { color: var(--color-muted); font-size: 0.82rem; }
  .confirm-msg { color: var(--color-accent); font-size: 0.85rem; }
  .error { color: var(--color-error, #ff9d9d); font-size: 0.82rem; }

  .rc-list { display: grid; gap: 0.5rem; padding: 0; margin: 0; }
  .rc-item {
    display: grid;
    grid-template-columns: 1fr 3rem auto;
    gap: 0.75rem;
    align-items: center;
    font-size: 0.85rem;
  }
  .rc-title { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .rc-score { color: var(--color-accent); font-variant-numeric: tabular-nums; text-align: right; }

  .rc-confirm {
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 999px;
    padding: 0.35rem 0.75rem;
    font-size: 0.75rem;
    background: rgba(143, 183, 255, 0.15);
    color: #e9eefc;
    cursor: pointer;
    white-space: nowrap;
  }
  .rc-confirm:hover { background: rgba(143, 183, 255, 0.28); }
  .rc-confirm:focus { outline: 2px solid var(--color-accent); outline-offset: 2px; }
  .rc-confirm:disabled { opacity: 0.4; cursor: default; }
</style>
