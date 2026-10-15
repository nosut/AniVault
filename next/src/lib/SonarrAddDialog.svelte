<script lang="ts">
  import ErrorNotice from './ErrorNotice.svelte';
  import { explainError, type ExplainedError } from './errors';
  import { createEventDispatcher } from 'svelte';
  import { addToSonarr, linkSonarrCoverage, lookupSonarrByTvdb, lookupSonarrCandidates, type SonarrCandidate } from './api';
  import { candidateAction, candidateLabel } from './sonarrCoverageUi';

  export let animeId: number;
  export let title: string;

  const dispatch = createEventDispatcher<{ done: { animeId: number }; close: void }>();

  let candidates: SonarrCandidate[] = [];
  let loading = true;
  let busyTvdb: number | null = null;
  let error: (ExplainedError & { retry: () => void }) | null = null;
  // A TVDB ID or link typed in when the name search misses the show.
  let tvdbInput = '';
  let findingTvdb = false;

  // The show the candidates were looked up for. The parent can reuse this
  // component for another show, so re-search whenever animeId changes and
  // never act on a previous show's results.
  let candidatesFor: number | null = null;
  $: if (animeId !== candidatesFor) search(animeId);

  async function search(id: number) {
    candidatesFor = id;
    candidates = [];
    tvdbInput = '';
    loading = true;
    error = null;
    try {
      const found = await lookupSonarrCandidates(id);
      if (id === candidatesFor) candidates = found;
    } catch (e) {
      if (id === candidatesFor) error = { ...explainError("AniVault couldn't look this show up in Sonarr.", e), retry: () => search(id) };
    } finally {
      if (id === candidatesFor) loading = false;
    }
  }

  async function findByTvdb() {
    const id = candidatesFor;
    const input = tvdbInput.trim();
    if (id === null || !input) return;
    findingTvdb = true;
    error = null;
    try {
      const found = await lookupSonarrByTvdb(input);
      if (id !== candidatesFor) return;
      // The pasted series goes first; drop its copy from the name search.
      const ids = new Set(found.map((c) => c.tvdb_id));
      candidates = [...found, ...candidates.filter((c) => !ids.has(c.tvdb_id))];
      tvdbInput = '';
    } catch (e) {
      if (id === candidatesFor) error = { ...explainError(`AniVault couldn't find "${input}" in Sonarr.`, e), retry: findByTvdb };
    } finally {
      if (id === candidatesFor) findingTvdb = false;
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
      const summary = candidateAction(c) === 'link'
        ? `"${title}" couldn't be linked to ${c.title} in Sonarr.`
        : `"${title}" couldn't be added to Sonarr.`;
      error = { ...explainError(summary, e), retry: () => choose(c) };
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
    <p class="muted">Sonarr found no match by name.</p>
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
  {#if !loading}
    <form class="tvdb" on:submit|preventDefault={findByTvdb}>
      <input
        bind:value={tvdbInput}
        placeholder="Not listed? Paste a TVDB ID or thetvdb.com link"
        aria-label="TVDB ID or link"
        disabled={findingTvdb}
      />
      <button type="submit" disabled={findingTvdb || !tvdbInput.trim()}>{findingTvdb ? '…' : 'Find'}</button>
    </form>
  {/if}
  {#if error}
    <ErrorNotice compact message={error.message} details={error.details} actionLabel="Try again" on:action={error.retry} on:dismiss={() => (error = null)} />
  {/if}
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
  .tvdb { display: flex; gap: 0.4rem; }
  .tvdb input { flex: 1; min-width: 0; border: 1px solid rgba(var(--color-accent-rgb),0.25); border-radius: 6px; padding: 0.35rem 0.55rem; background: rgba(255,255,255,0.06); color: var(--color-text); font-size: 0.8rem; }
  .muted { color: var(--color-muted); font-size: 0.85rem; margin: 0; }
</style>
