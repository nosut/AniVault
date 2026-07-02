<script lang="ts">
  import { onMount } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { fetchAnimeDetail, updateListEntry, type AnimeDetail } from './api';

  export let animeId: number;

  const dispatch = createEventDispatcher<{ back: void }>();

  let detail: AnimeDetail | null = null;
  let loading = false;
  let error: string | null = null;
  let savingField: 'progress' | 'status' | 'score' | null = null;
  let saveOk: string | null = null;

  let draftProgress = 0;
  let draftStatus = '';
  let draftScore = 0;

  const STATUS_OPTIONS = [
    { value: 'watching', label: 'Watching' },
    { value: 'completed', label: 'Completed' },
    { value: 'on_hold', label: 'On Hold' },
    { value: 'dropped', label: 'Dropped' },
    { value: 'plan_to_watch', label: 'Plan to Watch' },
  ];

  function parseTitles(titlesJson: string | null | undefined): { romaji?: string; english?: string; native?: string } {
    if (!titlesJson) return {};
    try {
      const parsed = JSON.parse(titlesJson);
      if (parsed && typeof parsed === 'object') return parsed;
    } catch {
      // ignore
    }
    return {};
  }

  function pickTitle(d: AnimeDetail): string {
    const titles = parseTitles(d.titles_json);
    return titles.romaji || titles.english || titles.native || `Anime #${d.anime_id}`;
  }

  function formatStatus(status: string | null): string {
    if (!status) return 'Unknown';
    const map: Record<string, string> = {
      watching: 'Watching',
      completed: 'Completed',
      on_hold: 'On Hold',
      dropped: 'Dropped',
      plan_to_watch: 'Plan to Watch',
    };
    return map[status] || status;
  }

  function formatMediaStatus(status: string | null): string {
    if (!status) return 'Unknown';
    const map: Record<string, string> = {
      finished: 'Finished',
      releasing: 'Releasing',
      not_yet_released: 'Not Yet Released',
      cancelled: 'Cancelled',
      hiatus: 'Hiatus',
    };
    return map[status] || status;
  }

  function setDraftsFromDetail(d: AnimeDetail) {
    draftProgress = d.watched_episodes ?? 0;
    draftStatus = d.list_status ?? '';
    draftScore = d.score ?? 0;
  }

  async function load() {
    loading = true;
    error = null;
    saveOk = null;
    try {
      const d = await fetchAnimeDetail(animeId);
      detail = d;
      setDraftsFromDetail(d);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    load();
  });

  $: if (animeId) {
    // reactive reload when prop changes
    load();
  }

  function clearSaveOkSoon() {
    setTimeout(() => {
      saveOk = null;
    }, 1500);
  }

  async function saveProgress() {
    if (!detail) return;
    savingField = 'progress';
    saveOk = null;
    try {
      await updateListEntry(animeId, { watched_episodes: draftProgress });
      saveOk = 'Progress saved';
      await load();
      clearSaveOkSoon();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      savingField = null;
    }
  }

  async function saveStatus() {
    if (!detail) return;
    savingField = 'status';
    saveOk = null;
    try {
      await updateListEntry(animeId, { status: draftStatus || null });
      saveOk = 'Status saved';
      await load();
      clearSaveOkSoon();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      savingField = null;
    }
  }

  async function saveScore() {
    if (!detail) return;
    savingField = 'score';
    saveOk = null;
    try {
      await updateListEntry(animeId, { score: draftScore });
      saveOk = 'Score saved';
      await load();
      clearSaveOkSoon();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      savingField = null;
    }
  }

  function clampProgress() {
    const max = detail?.episode_count ?? Number.MAX_SAFE_INTEGER;
    if (draftProgress < 0) draftProgress = 0;
    if (draftProgress > max) draftProgress = max;
  }

  function adjustProgress(delta: number) {
    draftProgress += delta;
    clampProgress();
  }

  function formatDate(ts: number | null): string {
    if (!ts) return '—';
    return new Date(ts * 1000).toLocaleString();
  }
</script>

<section class="detail-view" aria-label="Anime detail">
  <button
    class="back-btn"
    type="button"
    on:click={() => dispatch('back')}
    aria-label="Back"
  >
    ← Back
  </button>

  {#if loading && !detail}
    <div class="skeleton-wrap" aria-busy="true" aria-label="Loading anime detail">
      <div class="skeleton-cover" />
      <div class="skeleton-lines">
        <div class="skeleton-line short" />
        <div class="skeleton-line" />
        <div class="skeleton-line" />
        <div class="skeleton-line medium" />
      </div>
    </div>
  {:else if error && !detail}
    <div class="error-panel" role="alert">
      <p class="error-text">{error}</p>
      <button class="action-btn" type="button" on:click={load}>Retry</button>
    </div>
  {:else if detail}
    <div class="detail-layout">
      <div class="media-col">
        {#if detail.image_url}
          <img
            class="cover"
            src={detail.image_url}
            alt="Cover for {pickTitle(detail)}"
            loading="lazy"
          />
        {:else}
          <div class="cover-placeholder" aria-hidden="true">
            <span>No Cover</span>
          </div>
        {/if}

        <div class="meta-block">
          <p class="meta-item">
            <span class="meta-label">Episodes</span>
            <span class="meta-value">{detail.episode_count ?? '?'}</span>
          </p>
          <p class="meta-item">
            <span class="meta-label">Status</span>
            <span class="meta-value">{formatMediaStatus(detail.anime_status)}</span>
          </p>
          <p class="meta-item">
            <span class="meta-label">AniList</span>
            <span class="meta-value">{detail.tracker_id ? detail.tracker_id : 'Not mapped'}</span>
          </p>
          {#if detail.local_updated}
            <p class="meta-item">
              <span class="meta-label">Updated</span>
              <span class="meta-value">{formatDate(detail.local_updated)}</span>
            </p>
          {/if}
        </div>
      </div>

      <div class="info-col">
        <h1 class="title">{pickTitle(detail)}</h1>

        {#if detail.synopsis}
          <p class="synopsis">{detail.synopsis}</p>
        {:else}
          <p class="synopsis muted">No synopsis available.</p>
        {/if}

        {#if !detail.list_status && detail.watched_episodes == null && detail.score == null}
          <div class="empty-prompt">
            <p>Add to list by saving a status or progress.</p>
          </div>
        {/if}

        {#if saveOk}
          <p class="save-ok" aria-live="polite">{saveOk}</p>
        {/if}
        {#if error}
          <p class="error-text" role="alert">{error}</p>
        {/if}

        <div class="editors">
          <div class="editor-group">
            <label class="editor-label" for="progress-input">Progress</label>
            <div class="editor-row">
              <button
                class="step-btn"
                type="button"
                aria-label="Decrease progress"
                on:click={() => adjustProgress(-1)}
                disabled={savingField === 'progress'}
              >
                −
              </button>
              <input
                id="progress-input"
                class="num-input"
                type="number"
                min={0}
                max={detail.episode_count ?? undefined}
                bind:value={draftProgress}
                on:change={clampProgress}
                disabled={savingField === 'progress'}
                aria-label="Watched episodes"
              />
              <button
                class="step-btn"
                type="button"
                aria-label="Increase progress"
                on:click={() => adjustProgress(1)}
                disabled={savingField === 'progress'}
              >
                +
              </button>
              <button
                class="action-btn"
                type="button"
                on:click={saveProgress}
                disabled={savingField === 'progress'}
              >
                {savingField === 'progress' ? 'Saving…' : 'Save'}
              </button>
            </div>
          </div>

          <div class="editor-group">
            <label class="editor-label" for="status-select">Status</label>
            <div class="editor-row">
              <select
                id="status-select"
                class="select-input"
                bind:value={draftStatus}
                disabled={savingField === 'status'}
                aria-label="List status"
              >
                <option value="">— Select —</option>
                {#each STATUS_OPTIONS as opt}
                  <option value={opt.value}>{opt.label}</option>
                {/each}
              </select>
              <button
                class="action-btn"
                type="button"
                on:click={saveStatus}
                disabled={savingField === 'status'}
              >
                {savingField === 'status' ? 'Saving…' : 'Save'}
              </button>
            </div>
          </div>

          <div class="editor-group">
            <label class="editor-label" for="score-input">Score</label>
            <div class="editor-row">
              <input
                id="score-input"
                class="num-input"
                type="number"
                min={0}
                max={10}
                step={1}
                bind:value={draftScore}
                disabled={savingField === 'score'}
                aria-label="Score out of 10"
              />
              <button
                class="action-btn"
                type="button"
                on:click={saveScore}
                disabled={savingField === 'score'}
              >
                {savingField === 'score' ? 'Saving…' : 'Save'}
              </button>
            </div>
          </div>
        </div>

        {#if detail.recent_history && detail.recent_history.length > 0}
          <div class="history">
            <h2 class="section-heading">Recent Watch History</h2>
            <ul class="history-list" role="list">
              {#each detail.recent_history as h (h.id)}
                <li class="history-row" role="listitem">
                  <span class="history-ep">Ep {h.episode}</span>
                  <span class="history-player">{h.player ?? '—'}</span>
                  <span class="history-file" title={h.file_path ?? undefined}>
                    {h.file_path ? h.file_path.split(/[\\/]/).pop() : '—'}
                  </span>
                  <span class="history-time">{formatDate(h.watched_at)}</span>
                </li>
              {/each}
            </ul>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</section>

<style>
  .detail-view {
    display: grid;
    gap: 1rem;
    padding: 1rem;
    max-width: 1100px;
    margin: 0 auto;
  }

  .back-btn {
    justify-self: start;
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 999px;
    padding: 0.45rem 0.9rem;
    background: rgba(143, 183, 255, 0.12);
    color: #e9eefc;
    cursor: pointer;
    font-size: 0.85rem;
  }

  .back-btn:hover {
    background: rgba(143, 183, 255, 0.22);
  }

  .back-btn:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .detail-layout {
    display: grid;
    grid-template-columns: 260px 1fr;
    gap: 1.5rem;
    align-items: start;
  }

  @media (max-width: 720px) {
    .detail-layout {
      grid-template-columns: 1fr;
    }
  }

  .media-col {
    display: grid;
    gap: 0.75rem;
    position: sticky;
    top: 1rem;
  }

  @media (max-width: 720px) {
    .media-col {
      position: static;
    }
  }

  .cover {
    width: 100%;
    aspect-ratio: 2 / 3;
    object-fit: cover;
    border-radius: 16px;
    border: 1px solid rgba(143, 183, 255, 0.18);
    background: rgba(255, 255, 255, 0.04);
  }

  .cover-placeholder {
    width: 100%;
    aspect-ratio: 2 / 3;
    border-radius: 16px;
    border: 1px solid rgba(143, 183, 255, 0.18);
    background: rgba(255, 255, 255, 0.04);
    display: grid;
    place-items: center;
    color: var(--color-muted);
    font-size: 0.9rem;
  }

  .meta-block {
    display: grid;
    gap: 0.4rem;
    padding: 0.75rem;
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.03);
  }

  .meta-item {
    display: flex;
    justify-content: space-between;
    gap: 0.5rem;
    margin: 0;
    font-size: 0.8rem;
  }

  .meta-label {
    color: var(--color-muted);
  }

  .meta-value {
    color: var(--color-text);
    text-align: right;
  }

  .info-col {
    display: grid;
    gap: 1rem;
  }

  .title {
    margin: 0;
    font-size: 1.6rem;
    font-weight: 700;
    line-height: 1.2;
    color: var(--color-text);
  }

  .synopsis {
    margin: 0;
    line-height: 1.55;
    color: #c8d2e0;
  }

  .synopsis.muted {
    color: var(--color-muted);
  }

  .empty-prompt {
    padding: 0.75rem 1rem;
    border: 1px dashed rgba(143, 183, 255, 0.35);
    border-radius: 12px;
    color: var(--color-accent);
    font-size: 0.9rem;
  }

  .save-ok {
    margin: 0;
    color: var(--color-accent);
    font-size: 0.9rem;
  }

  .error-text {
    margin: 0;
    color: var(--color-error);
    font-size: 0.9rem;
  }

  .error-panel {
    display: grid;
    gap: 0.75rem;
    padding: 1rem;
    border: 1px solid rgba(255, 157, 157, 0.35);
    border-radius: 14px;
    background: rgba(255, 157, 157, 0.08);
  }

  .editors {
    display: grid;
    gap: 0.9rem;
    padding: 1rem;
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.03);
  }

  .editor-group {
    display: grid;
    gap: 0.35rem;
  }

  .editor-label {
    font-size: 0.78rem;
    color: var(--color-muted);
  }

  .editor-row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .num-input {
    width: 5rem;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(143, 183, 255, 0.25);
    border-radius: 8px;
    padding: 0.5rem 0.65rem;
    color: var(--color-text);
    font-size: 0.9rem;
  }

  .num-input:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .select-input {
    min-width: 10rem;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(143, 183, 255, 0.25);
    border-radius: 8px;
    padding: 0.5rem 0.65rem;
    color: var(--color-text);
    font-size: 0.9rem;
  }

  .select-input:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .step-btn {
    width: 2.2rem;
    height: 2.2rem;
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 8px;
    background: rgba(143, 183, 255, 0.12);
    color: #e9eefc;
    cursor: pointer;
    font-size: 1rem;
    line-height: 1;
  }

  .step-btn:hover {
    background: rgba(143, 183, 255, 0.22);
  }

  .step-btn:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .step-btn:disabled,
  .action-btn:disabled,
  .num-input:disabled,
  .select-input:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .action-btn {
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 999px;
    padding: 0.5rem 0.9rem;
    background: rgba(143, 183, 255, 0.18);
    color: #e9eefc;
    cursor: pointer;
    font-size: 0.82rem;
  }

  .action-btn:hover {
    background: rgba(143, 183, 255, 0.28);
  }

  .action-btn:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .section-heading {
    margin: 0;
    font-size: 0.85rem;
    text-transform: uppercase;
    letter-spacing: 0.14em;
    color: var(--color-accent);
  }

  .history {
    display: grid;
    gap: 0.5rem;
  }

  .history-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 0.4rem;
  }

  .history-row {
    display: grid;
    grid-template-columns: 4rem 7rem 1fr 10rem;
    gap: 0.6rem;
    align-items: center;
    padding: 0.5rem 0.6rem;
    border: 1px solid rgba(143, 183, 255, 0.12);
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.03);
    font-size: 0.82rem;
  }

  @media (max-width: 860px) {
    .history-row {
      grid-template-columns: 4rem 1fr auto;
    }
    .history-time {
      display: none;
    }
  }

  .history-ep {
    font-weight: 700;
    color: var(--color-accent);
  }

  .history-player {
    color: var(--color-muted);
  }

  .history-file {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: #c8d2e0;
  }

  .history-time {
    color: var(--color-muted);
    text-align: right;
  }

  .skeleton-wrap {
    display: grid;
    grid-template-columns: 260px 1fr;
    gap: 1.5rem;
    align-items: start;
  }

  @media (max-width: 720px) {
    .skeleton-wrap {
      grid-template-columns: 1fr;
    }
  }

  .skeleton-cover {
    width: 100%;
    aspect-ratio: 2 / 3;
    border-radius: 16px;
    background: linear-gradient(90deg, rgba(255,255,255,0.06) 25%, rgba(255,255,255,0.10) 50%, rgba(255,255,255,0.06) 75%);
    background-size: 200% 100%;
    animation: shimmer 1.4s infinite;
  }

  .skeleton-lines {
    display: grid;
    gap: 0.6rem;
  }

  .skeleton-line {
    height: 1rem;
    border-radius: 8px;
    background: linear-gradient(90deg, rgba(255,255,255,0.06) 25%, rgba(255,255,255,0.10) 50%, rgba(255,255,255,0.06) 75%);
    background-size: 200% 100%;
    animation: shimmer 1.4s infinite;
  }

  .skeleton-line.short { width: 40%; }
  .skeleton-line.medium { width: 70%; }

  @keyframes shimmer {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }
</style>
