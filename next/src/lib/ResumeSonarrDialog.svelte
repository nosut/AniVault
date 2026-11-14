<script lang="ts">
  // The reverse of the drop prompt: a show back in Watching or Planning gets
  // its Sonarr series monitored again, or added if it was removed.
  import { createEventDispatcher } from 'svelte';
  import { getStatusChangePreview, setSonarrMonitored } from './api';
  import { resumeItems, STATUS_LABELS, type CleanupRequest, type ResumeItem } from './dropCleanup';
  import PromptDialog from './PromptDialog.svelte';
  import SonarrAddDialog from './SonarrAddDialog.svelte';
  import ErrorNotice from './ErrorNotice.svelte';
  import { explainFailures, type ExplainedError, type StepFailure } from './errors';

  export let request: CleanupRequest;

  const dispatch = createEventDispatcher<{ close: void; done: { animeIds: number[] } }>();

  let items: ResumeItem[] = [];
  let loading = true;
  let busy = false;
  let failures: (ExplainedError & { items: string[] }) | null = null;
  let finished = false;

  let monitor = true;
  let search = true;
  let addingFor: number | null = null;
  let added = new Set<number>();
  // An explicit Monitor request says why there's nothing to do instead of
  // closing silently.
  let nothingReason: string | null = null;

  let loadedFor: CleanupRequest | null = null;
  $: if (request !== loadedFor) load(request);

  async function load(req: CleanupRequest) {
    loadedFor = req;
    loading = true;
    failures = null;
    finished = false;
    added = new Set();
    addingFor = null;
    monitor = true;
    search = true;
    nothingReason = null;
    const manual = req.kind === 'remonitor';
    try {
      const previews = await getStatusChangePreview(req.shows.map((s) => s.animeId));
      if (req !== loadedFor) return;
      items = resumeItems(req.shows, previews);
      // Already monitored, or Sonarr unreachable: nothing to offer.
      if (items.length === 0) {
        const unreachable = previews.find((p) => p.sonarr_error)?.sonarr_error;
        if (!manual) dispatch('close');
        else if (unreachable) nothingReason = `Sonarr couldn't be checked: ${unreachable}`;
        else nothingReason = req.shows.length === 1 ? 'Sonarr is already monitoring it.' : 'Sonarr is already monitoring them.';
      }
    } catch (e) {
      if (req !== loadedFor) return;
      if (!manual) dispatch('close');
      else nothingReason = `Sonarr couldn't be checked: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      if (req === loadedFor) loading = false;
    }
  }

  $: unmonitored = items.filter((i): i is Extract<ResumeItem, { kind: 'unmonitored' }> => i.kind === 'unmonitored');
  $: missing = items.filter((i) => i.kind === 'missing');
  $: named = items.length > 0 ? items : request.shows;
  $: heading = named.length === 1 ? named[0]!.title : `${named.length} shows`;
  $: monitorLabel = unmonitored.length === 1 && unmonitored[0]
    ? `Monitor “${unmonitored[0].sonarr.title}” again`
    : `Monitor ${unmonitored.length} series again`;
  $: addingTitle = missing.find((i) => i.animeId === addingFor)?.title ?? '';

  function addDone(animeId: number) {
    added = new Set(added).add(animeId);
    addingFor = null;
    dispatch('done', { animeIds: [animeId] });
  }

  async function apply() {
    if (!monitor || unmonitored.length === 0) {
      dispatch('close');
      return;
    }
    busy = true;
    const failed: StepFailure[] = [];
    for (const item of unmonitored) {
      try {
        await setSonarrMonitored(item.sonarr.sonarr_id, true, search);
      } catch (e) {
        failed.push({ what: `Sonarr couldn't start monitoring ${item.title} again.`, error: e });
      }
    }
    busy = false;
    dispatch('done', { animeIds: unmonitored.map((i) => i.animeId) });
    if (failed.length === 0) dispatch('close');
    else {
      failures = explainFailures("Some shows couldn't be picked back up in Sonarr.", failed);
      finished = true;
    }
  }
</script>

{#if !loading && (items.length > 0 || nothingReason)}
  <PromptDialog
    eyebrow={request.kind === 'remonitor' ? 'Sonarr' : `Moved to ${STATUS_LABELS[request.status] ?? request.status}`}
    heading={`Pick ${heading} back up in Sonarr?`}
    {busy}
    on:close={() => dispatch('close')}
  >
    {#if nothingReason && items.length === 0}
      <p class="hint">{nothingReason}</p>
      <div class="actions">
        <button class="primary" on:click={() => dispatch('close')}>Close</button>
      </div>
    {:else if finished}
      {#if failures}
        <ErrorNotice compact message={failures.message} items={failures.items} details={failures.details} on:dismiss={() => dispatch('close')} />
      {/if}
      <div class="actions">
        <button class="primary" on:click={() => dispatch('close')}>Close</button>
      </div>
    {:else}
      {#if unmonitored.length > 0}
        <fieldset>
          <legend>Unmonitored in Sonarr</legend>
          {#if unmonitored.length > 1}
            <ul class="shows">
              {#each unmonitored as item (item.animeId)}
                <li><span class="show-title">{item.title}</span><span class="show-meta">{item.sonarr.title}</span></li>
              {/each}
            </ul>
          {/if}
          <label><input type="checkbox" bind:checked={monitor} /> {monitorLabel}</label>
          {#if monitor}
            <label class="nested"><input type="checkbox" bind:checked={search} /> Search for missing episodes now</label>
          {/if}
        </fieldset>
      {/if}

      {#if missing.length > 0}
        <fieldset>
          <legend>Not in Sonarr</legend>
          <ul class="shows">
            {#each missing as item (item.animeId)}
              <li>
                <span class="show-title">{item.title}</span>
                {#if added.has(item.animeId)}
                  <span class="show-meta">Added ✓</span>
                {:else}
                  <button class="pill" disabled={busy || addingFor === item.animeId} on:click={() => (addingFor = item.animeId)}>Add to Sonarr…</button>
                {/if}
              </li>
            {/each}
          </ul>
          {#if addingFor !== null}
            <SonarrAddDialog
              animeId={addingFor}
              title={addingTitle}
              on:done={(e) => addDone(e.detail.animeId)}
              on:close={() => (addingFor = null)}
            />
          {/if}
        </fieldset>
      {/if}

      <div class="actions">
        <button on:click={() => dispatch('close')} disabled={busy}>{unmonitored.length > 0 ? 'Skip' : 'Close'}</button>
        {#if unmonitored.length > 0}
          <button class="primary" on:click={apply} disabled={busy || !monitor}>{busy ? 'Working…' : 'Apply'}</button>
        {/if}
      </div>
    {/if}
  </PromptDialog>
{/if}
