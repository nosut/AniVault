<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { deleteAnimeFiles, getDropCleanupPreview, removeFromSonarr, unmonitorSonarrSeries } from './api';
  import { cleanupItems, loadCleanupSettings, type CleanupItem, type CleanupRequest, type SonarrAction } from './dropCleanup';
  import { formatBytes } from './fileSize';

  export let request: CleanupRequest;

  const dispatch = createEventDispatcher<{ close: void; done: { animeIds: number[] } }>();

  const STATUS_LABELS: Record<string, string> = { dropped: 'Dropped', completed: 'Completed', on_hold: 'On Hold' };

  let items: CleanupItem[] = [];
  let loading = true;
  let busy = false;
  let errors: string[] = [];
  let finished = false;

  let sonarrAction: SonarrAction = 'unmonitor';
  let deleteSonarrFiles = false;
  // Deleting files is the destructive choice, so it starts unticked.
  let deleteLocalFiles = false;

  let loadedFor: CleanupRequest | null = null;
  $: if (request !== loadedFor) load(request);

  async function load(req: CleanupRequest) {
    loadedFor = req;
    loading = true;
    errors = [];
    finished = false;
    try {
      const [settings, previews] = await Promise.all([
        loadCleanupSettings(),
        getDropCleanupPreview(req.shows.map((s) => s.animeId)),
      ]);
      if (req !== loadedFor) return;
      items = cleanupItems(req.shows, previews, settings);
      // Nothing linked to Sonarr and no files: there is nothing to offer.
      if (items.length === 0) dispatch('close');
    } catch (e) {
      if (req === loadedFor) errors = [String(e)];
    } finally {
      if (req === loadedFor) loading = false;
    }
  }

  $: inSonarr = items.filter((i) => i.sonarr !== null);
  $: withFiles = items.filter((i) => i.fileCount > 0);
  $: fileCount = withFiles.reduce((n, i) => n + i.fileCount, 0);
  $: fileBytes = withFiles.reduce((n, i) => n + i.totalBytes, 0);
  $: nothingChosen = (inSonarr.length === 0 || sonarrAction === 'keep') && (withFiles.length === 0 || !deleteLocalFiles);
  $: heading = items.length === 1 ? items[0]!.title : `${items.length} shows`;
  $: sonarrLegend = items.length === 1 && inSonarr[0]?.sonarr ? `Sonarr: ${inSonarr[0].sonarr.title}` : 'Sonarr';

  async function apply() {
    busy = true;
    errors = [];
    const failed: string[] = [];
    for (const item of items) {
      if (item.sonarr && sonarrAction !== 'keep') {
        try {
          if (sonarrAction === 'unmonitor') await unmonitorSonarrSeries(item.animeId);
          else await removeFromSonarr(item.animeId, deleteSonarrFiles);
        } catch (e) {
          failed.push(`${item.title}: Sonarr: ${e}`);
        }
      }
      if (item.fileCount > 0 && deleteLocalFiles) {
        try {
          const report = await deleteAnimeFiles(item.animeId);
          for (const f of report.failed) failed.push(`${item.title}: ${f}`);
        } catch (e) {
          failed.push(`${item.title}: files: ${e}`);
        }
      }
    }
    busy = false;
    dispatch('done', { animeIds: items.map((i) => i.animeId) });
    if (failed.length === 0) dispatch('close');
    else {
      errors = failed;
      finished = true;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !busy) dispatch('close');
  }
</script>

<svelte:window on:keydown={onKeydown} />

{#if !loading && (items.length > 0 || errors.length > 0)}
  <div class="backdrop" role="presentation" on:click={() => !busy && dispatch('close')}></div>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="drop-cleanup-title">
    <span class="eyebrow">Moved to {STATUS_LABELS[request.status] ?? request.status}</span>
    <h2 id="drop-cleanup-title">Clean up {heading}?</h2>

    {#if finished}
      <p class="error">Some steps failed:</p>
      <ul class="errors">{#each errors as err}<li>{err}</li>{/each}</ul>
      <div class="actions">
        <button class="primary" on:click={() => dispatch('close')}>Close</button>
      </div>
    {:else}
      {#if items.length > 1}
        <ul class="shows">
          {#each items as item (item.animeId)}
            <li>
              <span class="show-title">{item.title}</span>
              <span class="show-meta">
                {#if item.sonarr}In Sonarr{item.sonarr.monitored ? '' : ' (unmonitored)'}{/if}
                {#if item.sonarr && item.fileCount > 0} · {/if}
                {#if item.fileCount > 0}{item.fileCount} files, {formatBytes(item.totalBytes)}{/if}
              </span>
            </li>
          {/each}
        </ul>
      {/if}

      {#if inSonarr.length > 0}
        <fieldset>
          <legend>{sonarrLegend}</legend>
          <label><input type="radio" bind:group={sonarrAction} value="keep" /> Leave it as it is</label>
          <label><input type="radio" bind:group={sonarrAction} value="unmonitor" /> Unmonitor, so Sonarr stops downloading it</label>
          <label><input type="radio" bind:group={sonarrAction} value="remove" /> Remove the series from Sonarr</label>
          {#if sonarrAction === 'remove'}
            <label class="nested"><input type="checkbox" bind:checked={deleteSonarrFiles} /> Also have Sonarr delete its files</label>
          {/if}
        </fieldset>
      {/if}

      {#if withFiles.length > 0}
        <fieldset>
          <legend>Episode files</legend>
          <label>
            <input type="checkbox" bind:checked={deleteLocalFiles} />
            Move {fileCount} {fileCount === 1 ? 'file' : 'files'} ({formatBytes(fileBytes)}) to the Recycle Bin
          </label>
          <p class="hint">Files on a network drive are deleted outright, because Windows has no Recycle Bin there.</p>
        </fieldset>
      {/if}

      {#if errors.length > 0}<p class="error">{errors[0]}</p>{/if}

      <div class="actions">
        <button on:click={() => dispatch('close')} disabled={busy}>Skip</button>
        <button class="primary" on:click={apply} disabled={busy || nothingChosen}>{busy ? 'Working…' : 'Apply'}</button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .backdrop { position: fixed; inset: 0; z-index: 60; background: rgba(0, 0, 0, 0.55); }
  .dialog {
    position: fixed; z-index: 61; left: 50%; top: 50%; transform: translate(-50%, -50%);
    width: min(28rem, calc(100vw - 2rem)); max-height: calc(100vh - 2rem); overflow-y: auto;
    display: flex; flex-direction: column; gap: 0.75rem;
    padding: 1.1rem 1.25rem; border-radius: 12px;
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    background: var(--color-surface); box-shadow: 0 16px 40px rgba(0, 0, 0, 0.55);
  }
  .eyebrow { font-size: 0.66rem; font-weight: 800; letter-spacing: 0.12em; text-transform: uppercase; color: var(--color-accent); }
  h2 { margin: 0; font-size: 1.05rem; color: var(--color-text); }
  .shows { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.3rem; max-height: 10rem; overflow-y: auto; }
  .shows li { display: flex; justify-content: space-between; gap: 0.75rem; font-size: 0.82rem; }
  .show-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .show-meta { color: var(--color-muted); white-space: nowrap; }
  fieldset { border: 1px solid rgba(255, 255, 255, 0.08); border-radius: 8px; padding: 0.6rem 0.75rem; margin: 0; display: flex; flex-direction: column; gap: 0.4rem; }
  legend { padding: 0 0.3rem; font-size: 0.78rem; color: var(--color-muted); }
  label { display: flex; align-items: center; gap: 0.5rem; font-size: 0.85rem; cursor: pointer; }
  label.nested { margin-left: 1.5rem; }
  .hint { margin: 0; font-size: 0.72rem; color: var(--color-muted); }
  .error { margin: 0; font-size: 0.82rem; color: var(--color-warning); }
  .errors { margin: 0; padding-left: 1.1rem; font-size: 0.78rem; color: var(--color-muted); word-break: break-all; }
  .actions { display: flex; justify-content: flex-end; gap: 0.5rem; }
  .actions button {
    border: 1px solid rgba(255, 255, 255, 0.12); border-radius: 999px; padding: 0.4rem 0.95rem;
    background: transparent; color: var(--color-text); font-size: 0.82rem; cursor: pointer;
  }
  .actions button.primary { border-color: rgba(var(--color-accent-rgb), 0.35); background: rgba(var(--color-accent-rgb), 0.18); }
  .actions button:disabled { opacity: 0.5; cursor: default; }
</style>
