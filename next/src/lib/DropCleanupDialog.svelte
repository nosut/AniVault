<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { deleteAnimeFiles, getStatusChangePreview, removeFromSonarr, setSonarrMonitored } from './api';
  import { cleanupItems, loadCleanupSettings, sharedWarnings, STATUS_LABELS, type CleanupItem, type CleanupRequest, type SonarrAction } from './dropCleanup';
  import { formatBytes } from './fileSize';
  import PromptDialog from './PromptDialog.svelte';

  export let request: CleanupRequest;

  const dispatch = createEventDispatcher<{ close: void; done: { animeIds: number[] } }>();

  let items: CleanupItem[] = [];
  let sonarrError: string | null = null;
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
    // The next queued prompt reuses this component; start it from the defaults.
    sonarrAction = 'unmonitor';
    deleteSonarrFiles = false;
    deleteLocalFiles = false;
    try {
      const [settings, previews] = await Promise.all([
        loadCleanupSettings(),
        getStatusChangePreview(req.shows.map((s) => s.animeId)),
      ]);
      if (req !== loadedFor) return;
      items = cleanupItems(req.shows, previews, settings);
      sonarrError = settings.offerSonarr ? (previews.find((p) => p.sonarr_error)?.sonarr_error ?? null) : null;
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
  $: shared = sharedWarnings(items);
  $: nothingChosen = (inSonarr.length === 0 || sonarrAction === 'keep') && (withFiles.length === 0 || !deleteLocalFiles);
  $: heading = items.length === 1 ? items[0]!.title : `${items.length} shows`;
  $: sonarrLegend = items.length === 1 && inSonarr[0]?.sonarr ? `Sonarr: ${inSonarr[0].sonarr.title}` : 'Sonarr';

  async function apply() {
    busy = true;
    errors = [];
    const failed: string[] = [];
    // Shows on one series share a single Sonarr action.
    const doneSeries = new Set<number>();
    for (const item of items) {
      if (item.sonarr && sonarrAction !== 'keep' && !doneSeries.has(item.sonarr.sonarr_id)) {
        doneSeries.add(item.sonarr.sonarr_id);
        try {
          if (sonarrAction === 'unmonitor') await setSonarrMonitored(item.sonarr.sonarr_id, false, false);
          else await removeFromSonarr(item.sonarr.sonarr_id, deleteSonarrFiles);
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
</script>

{#if !loading && (items.length > 0 || errors.length > 0)}
  <PromptDialog
    eyebrow={`Moved to ${STATUS_LABELS[request.status] ?? request.status}`}
    heading={`Clean up ${heading}?`}
    {busy}
    on:close={() => dispatch('close')}
  >
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
          {#if shared.length > 0}
            <p class="warning">This series also covers {shared.join(', ')}, which you're still watching or planning. Unmonitoring or removing it affects {shared.length === 1 ? 'that show' : 'those shows'} too.</p>
          {/if}
        </fieldset>
      {:else if sonarrError}
        <p class="hint">Couldn't reach Sonarr, so it isn't offered: {sonarrError}</p>
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
  </PromptDialog>
{/if}
