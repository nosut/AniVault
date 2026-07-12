<script lang="ts">
  import { createEventDispatcher, onDestroy } from 'svelte';
  import { remapSonarr, searchLibrary, type LibraryEntry } from './api';
  import { Pencil } from 'lucide-svelte';

  const dispatch = createEventDispatcher<{ changed: { animeId: number | null; title: string | null } }>();

  export let sonarrId: number;
  export let currentAnimeId: number | null;

  let open = false;
  let query = '';
  let results: LibraryEntry[] = [];
  let loading = false;
  let selectedId: number | null = null;
  let saving = false;

  function toggle() {
    open = !open;
    if (open && results.length === 0) {
      search('');
    }
  }

  async function search(q: string) {
    loading = true;
    try {
      results = await searchLibrary(q, null, 10, 0);
    } finally {
      loading = false;
    }
  }

  function onInput(e: Event) {
    query = (e.target as HTMLInputElement).value;
    search(query);
  }

  async function confirm() {
    if (selectedId === null) return;
    saving = true;
    try {
      await remapSonarr(sonarrId, selectedId);
      const title = results.find((r) => r.anime_id === selectedId)?.title ?? null;
      open = false;
      dispatch('changed', { animeId: selectedId, title });
    } finally {
      saving = false;
    }
  }

  let confirmingUnmap = false;
  let confirmUnmapTimer: ReturnType<typeof setTimeout> | null = null;

  // Armed state auto-expires so a stray click minutes later can't land on a
  // still-armed unmap button.
  function armUnmap() {
    confirmingUnmap = true;
    if (confirmUnmapTimer) clearTimeout(confirmUnmapTimer);
    confirmUnmapTimer = setTimeout(() => { confirmingUnmap = false; }, 4000);
  }

  async function unmap() {
    confirmingUnmap = false;
    if (confirmUnmapTimer) clearTimeout(confirmUnmapTimer);
    saving = true;
    try {
      await remapSonarr(sonarrId, null);
      open = false;
      dispatch('changed', { animeId: null, title: null });
    } finally {
      saving = false;
    }
  }

  onDestroy(() => {
    if (confirmUnmapTimer) clearTimeout(confirmUnmapTimer);
  });
</script>

<div class="remap-wrap">
  <button type="button" class="remap-toggle" on:click={toggle}>
    {#if open}Close{:else}<Pencil size={11} /> remap{/if}
  </button>

  {#if open}
    <div class="remap-dropdown">
      <input
        class="remap-search"
        type="text"
        placeholder="Search anime..."
        value={query}
        on:input={onInput}
      />

      {#if loading}
        <p class="remap-hint muted">Searching…</p>
      {:else if results.length === 0 && query}
        <p class="remap-hint muted">No matches</p>
      {/if}

      {#if results.length > 0}
        <ul class="remap-list" role="listbox">
          {#each results as entry}
            <li
              class="remap-option"
              class:selected={selectedId === entry.anime_id}
              role="option"
              aria-selected={selectedId === entry.anime_id}
              on:click={() => (selectedId = entry.anime_id)}
              on:keydown={(e) => e.key === 'Enter' && (selectedId = entry.anime_id)}
              tabindex="0"
            >
              <span class="remap-title">{entry.title}</span>
              <span class="remap-meta">{entry.status} · {entry.watched_episodes}/{entry.episode_count ?? '?'}</span>
            </li>
          {/each}
        </ul>
      {/if}

      <div class="remap-actions">
        <button
          type="button"
          class="remap-confirm"
          disabled={selectedId === null || saving}
          on:click={confirm}
        >
          {saving ? 'Saving…' : 'Confirm'}
        </button>
        {#if currentAnimeId}
          {#if confirmingUnmap}
            <button type="button" class="remap-unmap" disabled={saving} on:click={unmap}>
              {saving ? 'Saving…' : 'Confirm unmap?'}
            </button>
          {:else}
            <button
              type="button"
              class="remap-unmap"
              disabled={saving}
              on:click={armUnmap}
            >
              Unmap
            </button>
          {/if}
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .remap-wrap {
    position: relative;
  }

  .remap-toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    border: 1px solid rgba(var(--color-accent-rgb), 0.25);
    border-radius: 6px;
    padding: 0.25rem 0.6rem;
    background: rgba(var(--color-accent-rgb), 0.08);
    color: var(--color-muted);
    cursor: pointer;
    font-size: 0.72rem;
  }

  .remap-toggle:hover {
    background: rgba(var(--color-accent-rgb), 0.16);
    color: #e9eefc;
  }

  .remap-dropdown {
    position: absolute;
    top: 100%;
    right: 0;
    margin-top: 0.25rem;
    width: 320px;
    max-height: 360px;
    overflow-y: auto;
    border: 1px solid rgba(var(--color-accent-rgb), 0.25);
    border-radius: 12px;
    background: #171e2b;
    padding: 0.75rem;
    z-index: 10;
    display: grid;
    gap: 0.5rem;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  }

  .remap-search {
    border: 1px solid rgba(var(--color-accent-rgb), 0.25);
    border-radius: 8px;
    padding: 0.5rem 0.7rem;
    background: rgba(255, 255, 255, 0.06);
    color: var(--color-text);
    font-size: 0.85rem;
    width: 100%;
    box-sizing: border-box;
  }

  .remap-search:focus {
    outline: 2px solid rgba(var(--color-accent-rgb), 0.4);
    outline-offset: 1px;
  }

  .remap-hint {
    font-size: 0.78rem;
    margin: 0;
  }

  .remap-hint.muted {
    color: var(--color-muted);
  }

  .remap-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 0.25rem;
  }

  .remap-option {
    display: grid;
    gap: 0.1rem;
    padding: 0.4rem 0.6rem;
    border-radius: 6px;
    cursor: pointer;
    border: 1px solid transparent;
  }

  .remap-option:hover {
    background: rgba(var(--color-accent-rgb), 0.1);
  }

  .remap-option.selected {
    background: rgba(var(--color-accent-rgb), 0.18);
    border-color: rgba(var(--color-accent-rgb), 0.35);
  }

  .remap-title {
    font-size: 0.82rem;
    color: var(--color-text);
  }

  .remap-meta {
    font-size: 0.7rem;
    color: var(--color-muted);
  }

  .remap-actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }

  .remap-confirm {
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    border-radius: 6px;
    padding: 0.35rem 0.8rem;
    background: rgba(var(--color-accent-rgb), 0.15);
    color: #e9eefc;
    cursor: pointer;
    font-size: 0.78rem;
  }

  .remap-confirm:hover:not(:disabled) {
    background: rgba(var(--color-accent-rgb), 0.25);
  }

  .remap-confirm:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .remap-unmap {
    border: 1px solid rgba(var(--color-danger-rgb), 0.3);
    border-radius: 6px;
    padding: 0.35rem 0.8rem;
    background: transparent;
    color: var(--color-danger-text);
    cursor: pointer;
    font-size: 0.78rem;
  }

  .remap-unmap:hover:not(:disabled) {
    background: rgba(var(--color-danger-rgb), 0.12);
  }
</style>
