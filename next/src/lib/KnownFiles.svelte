<script lang="ts">
  import { onMount } from 'svelte';
  import { listKnownFiles, type FileIndexEntry } from './api';

  let entries: FileIndexEntry[] = [];
  let error: string | null = null;

  export async function load() {
    error = null;
    try {
      entries = await listKnownFiles(50);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(load);
</script>

<section class="known-files-card">
  <p class="eyebrow">Known files</p>
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
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: var(--radius-card);
    background: rgba(255, 255, 255, 0.04);
    padding: 1.25rem;
    display: grid;
    gap: 0.75rem;
  }
  .eyebrow {
    color: var(--color-accent);
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.78rem;
    font-weight: 800;
  }
  .empty { color: var(--color-muted); font-size: 0.82rem; }
  .error { color: var(--color-error, #ff9d9d); font-size: 0.82rem; }

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
