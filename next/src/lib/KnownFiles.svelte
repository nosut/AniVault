<script lang="ts">
  import { onMount } from 'svelte';
  import { listKnownFiles, rematchUnmappedFiles, type FileIndexEntry } from './api';

  let entries: FileIndexEntry[] = [];
  let error: string | null = null;
  let rematching = false;
  let rematchResult: number | null = null;

  export async function load() {
    error = null;
    rematchResult = null;
    try {
      entries = (await listKnownFiles(50)).filter((e) => !e.ignored);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function handleRematch() {
    rematching = true;
    rematchResult = null;
    try {
      rematchResult = await rematchUnmappedFiles();
      await load();
    } catch {
      rematchResult = null;
    } finally {
      rematching = false;
    }
  }

  onMount(load);
</script>

<section class="known-files-card">
  <div class="kf-header">
    <p class="eyebrow">Known files</p>
    <button class="action-btn small" on:click={handleRematch} disabled={rematching}>
      {rematching ? 'Matching\u2026' : 'Re-match'}
    </button>
  </div>
  {#if rematchResult !== null}
    <p class="success-msg">{rematchResult} files matched</p>
  {/if}
  {#if error}
    <p class="error">{error}</p>
  {:else if entries.length === 0}
    <p class="empty">No known files yet. Confirm a detection to add one.</p>
  {:else}
    <ul class="kf-list" role="list">
      {#each entries as entry}
        <li class="kf-item" role="listitem">
          <span class="kf-path">{entry.file_path}</span>
          {#if entry.anime_id}
            <span class="kf-meta">#{entry.anime_id} ep {entry.episode}</span>
          {/if}
          <span class="kf-confidence">{entry.confidence}%</span>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .known-files-card {
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
    border-radius: var(--radius-card);
    background: rgba(255, 255, 255, 0.04);
    padding: 1.25rem;
    display: grid;
    gap: 0.75rem;
  }
  .kf-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
  }
  .action-btn {
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    border-radius: 999px;
    padding: 0.35rem 0.75rem;
    font-size: 0.75rem;
    cursor: pointer;
    background: rgba(var(--color-accent-rgb), 0.18);
    color: #e9eefc;
    transition: background 0.15s;
  }
  .action-btn:hover:not(:disabled) {
    background: rgba(var(--color-accent-rgb), 0.28);
  }
  .action-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .success-msg {
    color: var(--color-success);
    font-size: 0.82rem;
    margin: 0;
  }
  .eyebrow {
    color: var(--color-accent);
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.78rem;
    font-weight: 800;
  }
  .empty { color: var(--color-muted); font-size: 0.82rem; }
  .error { color: var(--color-error); font-size: 0.82rem; }

  .kf-list { display: grid; gap: 0.35rem; padding: 0; margin: 0; }
  .kf-item { display: grid; grid-template-columns: 1fr auto auto; gap: 0.75rem; font-size: 0.78rem; align-items: center; }
  .kf-path {
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    color: var(--color-muted);
  }
  .kf-meta { color: var(--color-text); font-size: 0.75rem; }
  .kf-confidence { color: var(--color-accent); font-size: 0.75rem; }
</style>
