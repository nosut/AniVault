<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { addToSonarr, linkSonarrCoverage, lookupSonarrCandidates, type SonarrCandidate } from './api';
  import { candidateAction, candidateLabel } from './sonarrCoverageUi';

  export let animeId: number;
  export let title: string;

  const dispatch = createEventDispatcher<{ done: { animeId: number }; close: void }>();

  let candidates: SonarrCandidate[] = [];
  let loading = true;
  let busyTvdb: number | null = null;
  let error: string | null = null;

  // The show the candidates were looked up for. The parent can reuse this
  // component for another show, so re-search whenever animeId changes and
  // never act on a previous show's results.
  let candidatesFor: number | null = null;
  $: if (animeId !== candidatesFor) search(animeId);

  async function search(id: number) {
    candidatesFor = id;
    candidates = [];
    loading = true;
    error = null;
    try {
      const found = await lookupSonarrCandidates(id);
      if (id === candidatesFor) candidates = found;
    } catch (e) {
      if (id === candidatesFor) error = String(e);
    } finally {
      if (id === candidatesFor) loading = false;
    }
  }

  async function choose(c: SonarrCandidate) {
    const id = candidatesFor;
    if (id === null) return;
    busyTvdb = c.tvdb_id;
    error = null;
    try {
      if (candidateAction(c) === 'link' && c.sonarr_id != null) {
        await linkSonarrCoverage(id, c.sonarr_id);
      } else {
        await addToSonarr(id, c.tvdb_id);
      }
      dispatch('done', { animeId: id });
    } catch (e) {
      error = String(e);
    } finally {
      busyTvdb = null;
    }
  }
</script>

<div class="sonarr-add" role="dialog" aria-label={`Add ${title} to Sonarr`}>
  <div class="head">
    <strong>Add “{title}” to Sonarr</strong>
    <button class="close" on:click={() => dispatch('close')} aria-label="Close">×</button>
  </div>
  {#if loading}
    <p class="muted">Searching Sonarr…</p>
  {:else if candidates.length === 0 && !error}
    <p class="muted">Sonarr found no match. Add it in Sonarr directly.</p>
  {:else}
    <ul>
      {#each candidates as c (c.tvdb_id)}
        <li>
          {#if c.poster_url}<img src={c.poster_url} alt="" loading="lazy" />{/if}
          <span class="label">
            {candidateLabel(c)}
            {#if c.in_sonarr}<span class="badge">In Sonarr</span>{/if}
          </span>
          <button
            data-testid="candidate-action"
            disabled={busyTvdb !== null}
            on:click={() => choose(c)}
          >{busyTvdb === c.tvdb_id ? '…' : candidateAction(c) === 'link' ? 'Link' : 'Add'}</button>
        </li>
      {/each}
    </ul>
  {/if}
  {#if error}<p class="error">{error}</p>{/if}
</div>

<style>
  .sonarr-add { border: 1px solid rgba(var(--color-accent-rgb),0.25); border-radius: 8px; padding: 0.75rem; display: flex; flex-direction: column; gap: 0.5rem; }
  .head { display: flex; justify-content: space-between; align-items: center; }
  .close { background: none; border: none; color: var(--color-muted); cursor: pointer; font-size: 1.1rem; }
  ul { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.4rem; }
  li { display: flex; align-items: center; gap: 0.6rem; }
  img { width: 32px; height: 48px; object-fit: cover; border-radius: 3px; }
  .label { flex: 1; font-size: 0.85rem; }
  .badge { margin-left: 0.4rem; font-size: 0.7rem; color: var(--color-success); }
  .muted { color: var(--color-muted); font-size: 0.85rem; margin: 0; }
  .error { color: var(--color-warning); font-size: 0.85rem; margin: 0; }
</style>
