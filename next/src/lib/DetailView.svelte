<script lang="ts">
  import { onMount, createEventDispatcher } from 'svelte';
  import { fetchAnimeDetail, updateListEntry, type AnimeDetail } from './api';

  export let animeId: number;

  const dispatch = createEventDispatcher<{ onBack: void }>();

  let detail: AnimeDetail | null = null;
  let loading = false;
  let error: string | null = null;

  let watchedEpisodes: number | null = null;
  let status: string | null = null;
  let score: number | null = null;

  let savingProgress = false;
  let savingStatus = false;
  let savingScore = false;

  const statusOptions = [
    { value: 'watching', label: 'Watching' },
    { value: 'completed', label: 'Completed' },
    { value: 'on_hold', label: 'On Hold' },
    { value: 'dropped', label: 'Dropped' },
    { value: 'plan_to_watch', label: 'Plan to Watch' },
  ];

  const statusColors: Record<string, string> = {
    watching: '#8fb7ff',
    completed: '#7dd3a8',
    on_hold: '#ffc164',
    dropped: '#ff9d9d',
    plan_to_watch: '#c9a8ff',
  };

  function parseTitle(titlesJson: string): string {
    try {
      const obj = JSON.parse(titlesJson);
      if (obj && typeof obj === 'object') {
        return obj.romaji || obj.english || obj.native || '';
      }
    } catch {
      // ignore parse errors
    }
    return titlesJson;
  }

  function statusLabel(s: string): string {
    const map: Record<string, string> = {
      watching: 'Watching',
      completed: 'Completed',
      on_hold: 'On Hold',
      dropped: 'Dropped',
      plan_to_watch: 'Plan to Watch',
    };
    return map[s] ?? s;
  }

  function animeStatusLabel(s: string | null): string {
    if (!s) return 'Unknown';
    const map: Record<string, string> = {
      RELEASING: 'Releasing',
      FINISHED: 'Finished',
      NOT_YET_RELEASED: 'Not Yet Released',
      CANCELLED: 'Cancelled',
      HIATUS: 'Hiatus',
    };
    return map[s] ?? s;
  }

  function clampProgress(v: number): number {
    const max = detail?.episode_count ?? Infinity;
    return Math.max(0, Math.min(v, max));
  }

  function adjustProgress(delta: number) {
    watchedEpisodes = clampProgress((watchedEpisodes ?? 0) + delta);
  }

  async function load() {
    loading = true;
    error = null;
    try {
      detail = await fetchAnimeDetail(animeId);
      if (detail) {
        watchedEpisodes = detail.watched_episodes ?? 0;
        status = detail.list_status;
        score = detail.score;
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function saveProgress() {
    if (!detail) return;
    savingProgress = true;
    try {
      await updateListEntry(animeId, null, watchedEpisodes, null);
      await load();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      savingProgress = false;
    }
  }

  async function saveStatus() {
    if (!detail) return;
    savingStatus = true;
    try {
      await updateListEntry(animeId, status, null, null);
      await load();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      savingStatus = false;
    }
  }

  async function saveScore() {
    if (!detail) return;
    savingScore = true;
    try {
      await updateListEntry(animeId, null, null, score);
      await load();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      savingScore = false;
    }
  }

  async function addToList() {
    status = 'plan_to_watch';
    await saveStatus();
  }

  onMount(() => {
    void load();
  });
</script>

<section class="detail-view" aria-label="Anime detail">
  <button type="button" class="back-btn" on:click={() => dispatch('onBack')}>
    ← Back to Library
  </button>

  {#if error}
    <div class="error-box">
      <p class="error">{error}</p>
      <button type="button" class="retry-btn" on:click={load}>Retry</button>
    </div>
  {:else if loading}
    <div class="detail-card skeleton-card" aria-hidden="true">
      <div class="skeleton-header">
        <div class="skeleton-cover"></div>
        <div class="skeleton-meta">
          <div class="skeleton-line" style="width: 70%; height: 1.4rem"></div>
          <div class="skeleton-line" style="width: 40%; height: 0.85rem"></div>
          <div class="skeleton-line" style="width: 30%; height: 0.85rem"></div>
        </div>
      </div>
      <div class="skeleton-body">
        <div class="skeleton-line" style="width: 100%; height: 0.85rem"></div>
        <div class="skeleton-line" style="width: 90%; height: 0.85rem"></div>
        <div class="skeleton-line" style="width: 60%; height: 0.85rem"></div>
      </div>
    </div>
  {:else if detail}
    <div class="detail-card">
      <div class="detail-header">
        {#if detail.image_url}
          <img src={detail.image_url} alt="" class="cover" loading="lazy" />
        {:else}
          <div class="cover-placeholder" aria-hidden="true"></div>
        {/if}
        <div class="detail-meta">
          <h1 class="title">{parseTitle(detail.titles_json)}</h1>
          <div class="meta-row">
            {#if detail.episode_count != null}
              <span class="meta-pill">{detail.episode_count} episodes</span>
            {/if}
            <span class="meta-pill anime-status">{animeStatusLabel(detail.anime_status)}</span>
          </div>
        </div>
      </div>

      {#if detail.synopsis}
        <div class="synopsis">
          <p class="eyebrow">Synopsis</p>
          <p class="synopsis-text">{detail.synopsis}</p>
        </div>
      {/if}

      {#if detail.list_status == null}
        <div class="empty-list">
          <p class="empty-title">Not in your list yet</p>
          <p class="empty-hint">Start watching to track progress and sync with AniList.</p>
          <button type="button" class="btn-primary" on:click={addToList}>Add to list</button>
        </div>
      {:else}
        <div class="editors">
          <div class="editor-group">
            <p class="eyebrow">Progress</p>
            <div class="editor-row">
              <button type="button" class="step-btn" on:click={() => adjustProgress(-1)} disabled={savingProgress}>−</button>
              <input
                type="number"
                class="number-input"
                min="0"
                max={detail.episode_count ?? undefined}
                bind:value={watchedEpisodes}
                disabled={savingProgress}
              />
              <button type="button" class="step-btn" on:click={() => adjustProgress(1)} disabled={savingProgress}>+</button>
              <button type="button" class="btn-primary" on:click={saveProgress} disabled={savingProgress}>
                {savingProgress ? 'Saving…' : 'Save Progress'}
              </button>
            </div>
          </div>

          <div class="editor-group">
            <p class="eyebrow">Status</p>
            <div class="editor-row">
              <select class="status-select" bind:value={status} disabled={savingStatus}>
                {#each statusOptions as opt}
                  <option value={opt.value}>{opt.label}</option>
                {/each}
              </select>
              <button type="button" class="btn-primary" on:click={saveStatus} disabled={savingStatus}>
                {savingStatus ? 'Saving…' : 'Save Status'}
              </button>
            </div>
          </div>

          <div class="editor-group">
            <p class="eyebrow">Score</p>
            <div class="editor-row">
              <input
                type="number"
                class="number-input"
                min="0"
                max="10"
                step="1"
                bind:value={score}
                disabled={savingScore}
              />
              <button type="button" class="btn-primary" on:click={saveScore} disabled={savingScore}>
                {savingScore ? 'Saving…' : 'Save Score'}
              </button>
            </div>
          </div>
        </div>
      {/if}

      <div class="history-placeholder">
        <p class="eyebrow">Recent Watch History</p>
        <p class="hint">Watch history coming soon.</p>
      </div>

      <div class="tracker">
        <p class="eyebrow">AniList</p>
        {#if detail.tracker_id}
          <p class="tracker-id">AniList ID: {detail.tracker_id}</p>
        {:else}
          <p class="hint">Not synced.</p>
        {/if}
      </div>
    </div>
  {/if}
</section>

<style>
  .detail-view {
    display: grid;
    gap: 1rem;
  }

  .back-btn {
    justify-self: start;
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 999px;
    padding: 0.5rem 0.85rem;
    font-size: 0.78rem;
    cursor: pointer;
    background: rgba(143, 183, 255, 0.12);
    color: #e9eefc;
  }

  .back-btn:hover {
    background: rgba(143, 183, 255, 0.22);
  }

  .back-btn:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .detail-card {
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: var(--radius-card);
    background: rgba(255, 255, 255, 0.04);
    padding: 1.5rem;
    display: grid;
    gap: 1.25rem;
  }

  .detail-header {
    display: flex;
    gap: 1.25rem;
    align-items: flex-start;
    flex-wrap: wrap;
  }

  .cover {
    width: 160px;
    height: 240px;
    border-radius: 12px;
    object-fit: cover;
    flex-shrink: 0;
    box-shadow: var(--shadow-card);
  }

  .cover-placeholder {
    width: 160px;
    height: 240px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.08);
    flex-shrink: 0;
  }

  .detail-meta {
    display: grid;
    gap: 0.75rem;
    min-width: 0;
  }

  .title {
    margin: 0;
    font-size: 1.6rem;
    font-weight: 800;
    line-height: 1.1;
    letter-spacing: -0.03em;
  }

  .meta-row {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .meta-pill {
    display: inline-block;
    padding: 0.25rem 0.6rem;
    border-radius: 999px;
    font-size: 0.72rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    background: rgba(255, 255, 255, 0.1);
    color: var(--color-text);
  }

  .anime-status {
    background: rgba(143, 183, 255, 0.18);
    color: #e9eefc;
  }

  .eyebrow {
    color: var(--color-accent);
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.78rem;
    font-weight: 800;
    margin: 0 0 0.35rem;
  }

  .synopsis-text {
    margin: 0;
    color: var(--color-muted);
    font-size: 0.9rem;
    line-height: 1.55;
    max-height: 12rem;
    overflow-y: auto;
  }

  .editors {
    display: grid;
    gap: 1rem;
  }

  .editor-group {
    display: grid;
    gap: 0.5rem;
  }

  .editor-row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .step-btn {
    width: 2rem;
    height: 2rem;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.06);
    color: var(--color-text);
    font-size: 1rem;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .step-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
  }

  .step-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .number-input {
    width: 4rem;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 999px;
    padding: 0.45rem 0.7rem;
    color: var(--color-text);
    font-size: 0.85rem;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }

  .number-input:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .status-select {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 999px;
    padding: 0.45rem 0.7rem;
    color: var(--color-text);
    font-size: 0.85rem;
    cursor: pointer;
  }

  .status-select:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .btn-primary {
    border-radius: 999px;
    padding: 0.45rem 0.9rem;
    font-size: 0.78rem;
    cursor: pointer;
    border: 1px solid rgba(143, 183, 255, 0.35);
    background: rgba(143, 183, 255, 0.18);
    color: #e9eefc;
    white-space: nowrap;
  }

  .btn-primary:hover:not(:disabled) {
    background: rgba(143, 183, 255, 0.28);
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .btn-primary:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .empty-list {
    border: 1px dashed rgba(143, 183, 255, 0.25);
    border-radius: var(--radius-card);
    background: rgba(255, 255, 255, 0.03);
    padding: 1.5rem;
    display: grid;
    gap: 0.5rem;
  }

  .empty-title {
    color: var(--color-text);
    font-size: 1rem;
    font-weight: 700;
    margin: 0;
  }

  .empty-hint {
    color: var(--color-muted);
    font-size: 0.85rem;
    margin: 0;
  }

  .history-placeholder,
  .tracker {
    display: grid;
    gap: 0.35rem;
  }

  .hint {
    color: var(--color-muted);
    font-size: 0.85rem;
    margin: 0;
  }

  .tracker-id {
    color: var(--color-text);
    font-size: 0.85rem;
    margin: 0;
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  }

  .error-box {
    border: 1px solid rgba(255, 157, 157, 0.25);
    border-radius: var(--radius-card);
    background: rgba(255, 157, 157, 0.06);
    padding: 1.25rem;
    display: grid;
    gap: 0.75rem;
  }

  .error {
    color: var(--color-error, #ff9d9d);
    font-size: 0.85rem;
    margin: 0;
  }

  .retry-btn {
    justify-self: start;
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 999px;
    padding: 0.5rem 0.85rem;
    font-size: 0.78rem;
    cursor: pointer;
    background: rgba(143, 183, 255, 0.18);
    color: #e9eefc;
  }

  .retry-btn:hover {
    background: rgba(143, 183, 255, 0.28);
  }

  .retry-btn:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .skeleton-card {
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }

  .skeleton-header {
    display: flex;
    gap: 1.25rem;
    align-items: flex-start;
  }

  .skeleton-cover {
    width: 160px;
    height: 240px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.08);
    flex-shrink: 0;
  }

  .skeleton-meta {
    display: grid;
    gap: 0.6rem;
    flex: 1;
  }

  .skeleton-body {
    display: grid;
    gap: 0.5rem;
  }

  .skeleton-line {
    height: 0.85rem;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.08);
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }
</style>
