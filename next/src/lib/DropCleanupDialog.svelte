<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { deleteAnimeFiles, getStatusChangePreview, removeFromSonarr, setSonarrMonitored } from './api';
  import { cleanupItems, loadCleanupSettings, sharedWarnings, STATUS_LABELS, type CleanupItem, type CleanupRequest, type SonarrAction } from './dropCleanup';
  import { formatBytes } from './fileSize';
  import PromptDialog from './PromptDialog.svelte';
  import ErrorNotice from './ErrorNotice.svelte';
  import { explainError, explainFailures, type ExplainedError, type StepFailure } from './errors';

  export let request: CleanupRequest;

  const dispatch = createEventDispatcher<{ close: void; done: { animeIds: number[] } }>();

  let items: CleanupItem[] = [];
  let sonarrError: ExplainedError | null = null;
  let loading = true;
  let busy = false;
  let loadError: ExplainedError | null = null;
  let failures: (ExplainedError & { items: string[] }) | null = null;
  let finished = false;
  // Set once loading decides the dialog has something to ask. Dismissing a
  // notice inside it must not hide it, or the queue would wait on it forever.
  let shown = false;

  let sonarrAction: SonarrAction = 'unmonitor';
  let deleteSonarrFiles = false;
  // Deleting files is the destructive choice, so it starts unticked.
  let deleteLocalFiles = false;

  let loadedFor: CleanupRequest | null = null;
  $: if (request !== loadedFor) load(request);

  async function load(req: CleanupRequest) {
    loadedFor = req;
    loading = true;
    shown = false;
    loadError = null;
    failures = null;
    finished = false;
    // The next queued prompt reuses this component; start it from the defaults.
    sonarrAction = 'unmonitor';
    deleteSonarrFiles = false;
    // Asking to clean up a show is asking for its files to go, so that one
    // starts ticked; they still go to the Recycle Bin.
    deleteLocalFiles = req.kind === 'cleanup';
    try {
      const [saved, previews] = await Promise.all([
        loadCleanupSettings(),
        getStatusChangePreview(req.shows.map((s) => s.animeId)),
      ]);
      if (req !== loadedFor) return;
      const settings = req.kind === 'cleanup' ? { ...saved, offerSonarr: true, offerFiles: true } : saved;
      items = cleanupItems(req.shows, previews, settings);
      const sonarrRaw = settings.offerSonarr ? (previews.find((p) => p.sonarr_error)?.sonarr_error ?? null) : null;
      sonarrError = sonarrRaw ? explainError("Sonarr options aren't shown because AniVault couldn't check this show in Sonarr.", sonarrRaw) : null;
      // Nothing linked to Sonarr and no files: there is nothing to offer. A
      // removal still asks when Sonarr couldn't be checked, since the series
      // may keep downloading once the show is gone.
      // An explicit Clean up always answers, even if only to say there's nothing.
      if (items.length === 0 && req.kind !== 'cleanup' && !(req.kind === 'remove' && sonarrError)) finish(true);
      else shown = true;
    } catch (e) {
      if (req === loadedFor) {
        loadError = explainError("The clean-up options couldn't be loaded.", e);
        shown = true;
      }
    } finally {
      if (req === loadedFor) loading = false;
    }
  }

  $: removing = request.kind === 'remove';
  $: manual = request.kind === 'cleanup';
  $: eyebrow = removing ? 'Removing from library' : manual ? 'Clean up' : `Moved to ${STATUS_LABELS[request.status] ?? request.status}`;

  /// Close the dialog; a removal also learns whether to go ahead.
  function finish(proceed: boolean) {
    request.settle?.(proceed);
    dispatch('close');
  }

  $: inSonarr = items.filter((i) => i.sonarr !== null);
  $: withFiles = items.filter((i) => i.fileCount > 0);
  $: fileCount = withFiles.reduce((n, i) => n + i.fileCount, 0);
  $: fileBytes = withFiles.reduce((n, i) => n + i.totalBytes, 0);
  $: shared = sharedWarnings(items);
  $: nothingChosen = (inSonarr.length === 0 || sonarrAction === 'keep') && (withFiles.length === 0 || !deleteLocalFiles);
  $: named = items.length > 0 ? items : request.shows;
  $: heading = named.length === 1 ? named[0]!.title : `${named.length} shows`;
  $: sonarrLegend = items.length === 1 && inSonarr[0]?.sonarr ? `Sonarr: ${inSonarr[0].sonarr.title}` : 'Sonarr';

  async function apply() {
    busy = true;
    failures = null;
    const failed: StepFailure[] = [];
    // Shows on one series share a single Sonarr action.
    const doneSeries = new Set<number>();
    for (const item of items) {
      if (item.sonarr && sonarrAction !== 'keep' && !doneSeries.has(item.sonarr.sonarr_id)) {
        doneSeries.add(item.sonarr.sonarr_id);
        try {
          if (sonarrAction === 'unmonitor') await setSonarrMonitored(item.sonarr.sonarr_id, false, false);
          else await removeFromSonarr(item.sonarr.sonarr_id, deleteSonarrFiles);
        } catch (e) {
          const did = sonarrAction === 'unmonitor' ? 'unmonitor' : 'remove';
          failed.push({ what: `Sonarr couldn't ${did} ${item.title}.`, error: e });
        }
      }
      if (item.fileCount > 0 && deleteLocalFiles) {
        try {
          const report = await deleteAnimeFiles(item.animeId);
          for (const f of report.failed) {
            failed.push({ what: `A file from ${item.title} couldn't be moved to the Recycle Bin.`, error: f });
          }
        } catch (e) {
          failed.push({ what: `The files for ${item.title} couldn't be deleted.`, error: e });
        }
      }
    }
    busy = false;
    dispatch('done', { animeIds: items.map((i) => i.animeId) });
    if (failed.length === 0) finish(true);
    else {
      failures = explainFailures("Some of the clean-up didn't finish.", failed);
      finished = true;
    }
  }
</script>

{#if !loading && shown}
  <PromptDialog
    {eyebrow}
    heading={removing ? `Clean up before removing ${heading}?` : `Clean up ${heading}?`}
    {busy}
    on:close={() => finish(false)}
  >
    {#if finished && failures}
      {#if removing}
        <ErrorNotice compact message={failures.message} items={failures.items} details={failures.details} />
        <div class="actions">
          <button on:click={() => finish(false)}>Keep in library</button>
          <button class="primary" on:click={() => finish(true)}>Remove anyway</button>
        </div>
      {:else}
        <ErrorNotice compact message={failures.message} items={failures.items} details={failures.details} on:dismiss={() => finish(false)} />
        <div class="actions">
          <button class="primary" on:click={() => finish(false)}>Close</button>
        </div>
      {/if}
    {:else if manual && items.length === 0 && !loadError}
      <p class="hint">{named.length === 1 ? "It isn't" : "They aren't"} in Sonarr and {named.length === 1 ? 'has' : 'have'} no episode files, so there's nothing to clean up.</p>
      {#if sonarrError}
        <ErrorNotice compact message={sonarrError.message} details={sonarrError.details} on:dismiss={() => (sonarrError = null)} />
      {/if}
      <div class="actions">
        <button class="primary" on:click={() => finish(false)}>Close</button>
      </div>
    {:else}
      {#if manual}
        <p class="hint">{named.length === 1 ? 'The show stays' : 'The shows stay'} in your library with {named.length === 1 ? 'its' : 'their'} progress, score and history.</p>
      {/if}
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
          {#if removing}
            <p class="hint">Once the show is gone, AniVault can't find this series for you, and Sonarr keeps downloading it unless you stop it here.</p>
          {/if}
          {#if shared.length > 0}
            <p class="warning">This series also covers {shared.join(', ')}, which you're still watching or planning. Unmonitoring or removing it affects {shared.length === 1 ? 'that show' : 'those shows'} too.</p>
          {/if}
        </fieldset>
      {:else if sonarrError}
        <ErrorNotice compact message={sonarrError.message} details={sonarrError.details} on:dismiss={() => (sonarrError = null)} />
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

      {#if loadError}
        <ErrorNotice compact message={loadError.message} details={loadError.details} actionLabel="Try again" on:action={() => load(request)} on:dismiss={() => (loadError = null)} />
      {/if}

      {#if removing}
        <div class="actions">
          <button on:click={() => finish(false)} disabled={busy}>Cancel</button>
          {#if nothingChosen}
            <button class="primary" on:click={() => finish(true)} disabled={busy}>Remove</button>
          {:else}
            <button class="primary" on:click={apply} disabled={busy}>{busy ? 'Working…' : 'Clean up and remove'}</button>
          {/if}
        </div>
      {:else}
        <div class="actions">
          <button on:click={() => finish(false)} disabled={busy}>Skip</button>
          <button class="primary" on:click={apply} disabled={busy || nothingChosen}>{busy ? 'Working…' : 'Apply'}</button>
        </div>
      {/if}
    {/if}
  </PromptDialog>
{/if}
