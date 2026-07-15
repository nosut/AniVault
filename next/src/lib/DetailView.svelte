<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { fetchAnimeDetail, getSonarrAvailability, updateListEntry, deleteAnime, getEpisodeFiles, openEpisodeFile, openContainingFolder, getAnimeRelations, getNextAiring, rescanAnimeFiles, repairAnimeFileMappings, pickFolder, mapFolderToAnime, unmapKnownFiles, type AnimeDetail, type SonarrAvailability, type FileIndexEntry, type LibraryScanReport, type RelationEntry, type NextAiring, type EngineEvent } from './api';
  import { onDestroy } from 'svelte';
  import SonarrRemap from './SonarrRemap.svelte';
  import { mappingSourceLabel, partitionMappingConflicts } from './fileMappingUi';
  import { ArrowLeft, ExternalLink, FolderOpen, FolderInput, RotateCw, Play, Trash2 } from 'lucide-svelte';

  export let animeId: number;
  export let events: EngineEvent[] = [];

  const dispatch = createEventDispatcher<{ back: void; select: { anime_id: number } }>();

  let detail: AnimeDetail | null = null;
  let loading = false;
  let error: string | null = null;
  let savingField: 'progress' | 'status' | 'score' | null = null;
  let saveOk: string | null = null;

  let sonarrAvail: SonarrAvailability | null = null;
  let sonarrLoading = false;

  let episodeFiles: FileIndexEntry[] = [];
  let episodeFilesLoading = false;
  let rescanning = false;

  let fileScanReport: LibraryScanReport | null = null;
  let fileActionError: string | null = null;
  let fileActionMessage: string | null = null;
  let repairConfirming = false;
  let repairing = false;

  $: conflictGroups = partitionMappingConflicts(fileScanReport?.mapping_conflicts ?? []);

  let relations: RelationEntry[] = [];
  let relationsLoading = false;

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

  function parseTitles(titlesJson: string | null | undefined): { romaji?: string; english?: string; native?: string; synonyms?: string[] } {
    if (!titlesJson) return {};
    try {
      const parsed = JSON.parse(titlesJson);
      if (parsed && typeof parsed === 'object') {
        return {
          romaji: parsed.romaji || undefined,
          english: parsed.english || undefined,
          native: parsed.japanese || parsed.native || undefined,
          synonyms: parsed.synonyms || [],
        };
      }
    } catch {
      // ignore
    }
    return {};
  }

  let titles: { romaji?: string; english?: string; native?: string; synonyms?: string[] } = {};

  $: if (detail) {
    titles = parseTitles(detail.titles_json);
  }

  let confirmingDelete = false;
  let deleting = false;
  let confirmDeleteTimer: ReturnType<typeof setTimeout> | null = null;

  let nextAiring: NextAiring | null = null;
  let nowTs = Math.floor(Date.now() / 1000);
  const airTicker = setInterval(() => { nowTs = Math.floor(Date.now() / 1000); }, 1000);
  onDestroy(() => {
    clearInterval(airTicker);
    if (confirmDeleteTimer) clearTimeout(confirmDeleteTimer);
  });

  async function loadAiring() {
    const requestedId = animeId;
    nextAiring = null;
    try {
      const na = await getNextAiring(requestedId);
      if (requestedId === animeId) nextAiring = na;
    } catch {
      if (requestedId === animeId) nextAiring = null;
    }
  }

  function formatCountdown(secs: number): string {
    if (secs <= 0) return 'airing now';
    const d = Math.floor(secs / 86400);
    const h = Math.floor((secs % 86400) / 3600);
    const m = Math.floor((secs % 3600) / 60);
    if (d > 0) return `${d}d ${h}h`;
    if (h > 0) return `${h}h ${m}m`;
    return `${m}m`;
  }

  $: airingLabel = nextAiring
    ? `Ep ${nextAiring.episode} airs ${new Date(nextAiring.airing_at * 1000).toLocaleString()} · in ${formatCountdown(nextAiring.airing_at - nowTs)}`
    : '';

  // Armed state auto-expires so a stray click minutes later can't land on a
  // still-armed delete button.
  function armDelete() {
    confirmingDelete = true;
    if (confirmDeleteTimer) clearTimeout(confirmDeleteTimer);
    confirmDeleteTimer = setTimeout(() => { confirmingDelete = false; }, 4000);
  }

  function cancelDelete() {
    confirmingDelete = false;
    if (confirmDeleteTimer) clearTimeout(confirmDeleteTimer);
  }

  async function handleDelete() {
    confirmingDelete = false;
    if (confirmDeleteTimer) clearTimeout(confirmDeleteTimer);
    deleting = true;
    try {
      await deleteAnime(animeId);
      dispatch('back');
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      deleting = false;
    }
  }

  function pickTitle(d: AnimeDetail): string {
    const titles = parseTitles(d.titles_json);
    return titles.english || titles.romaji || titles.native || `Anime #${d.anime_id}`;
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
    // AniList stores these uppercase (FINISHED, RELEASING, …).
    return map[status.toLowerCase()] || status;
  }

  function setDraftsFromDetail(d: AnimeDetail) {
    draftProgress = d.watched_episodes ?? 0;
    draftStatus = d.list_status ?? '';
    draftScore = d.score ?? 0;
  }

  // Live-update progress when the engine advances it (auto-detected playback).
  // Depends only on `events` so it won't loop on the writes below.
  $: applyProgressEvents(events);
  function applyProgressEvents(evs: EngineEvent[]) {
    if (!detail || !evs || evs.length === 0) return;
    for (const ev of evs) {
      if ('ProgressAdvanced' in ev && ev.ProgressAdvanced.anime_id === animeId) {
        const ne = ev.ProgressAdvanced.new_episode;
        if (detail.watched_episodes == null || ne > detail.watched_episodes) {
          const completed = detail.episode_count != null && ne >= detail.episode_count;
          detail = {
            ...detail,
            watched_episodes: ne,
            list_status: completed ? 'completed' : detail.list_status,
          };
          draftProgress = ne;
          if (completed) draftStatus = 'completed';
        }
      }
    }
  }

  async function load() {
    const requestedId = animeId;
    loading = true;
    error = null;
    saveOk = null;
    fileScanReport = null;
    fileActionError = null;
    fileActionMessage = null;
    repairConfirming = false;
    try {
      const d = await fetchAnimeDetail(requestedId);
      if (requestedId !== animeId) return; // a newer anime is now showing
      detail = d;
      setDraftsFromDetail(d);
      loadSonarr();
      loadEpisodeFiles();
      loadRelations();
    } catch (e) {
      if (requestedId !== animeId) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (requestedId === animeId) loading = false;
    }
  }

  async function loadSonarr() {
    const requestedId = animeId;
    sonarrLoading = true;
    try {
      const avail = await getSonarrAvailability(requestedId);
      if (requestedId !== animeId) return;
      sonarrAvail = avail;
    } catch {
      if (requestedId === animeId) sonarrAvail = null;
    } finally {
      if (requestedId === animeId) sonarrLoading = false;
    }
  }

  async function loadEpisodeFiles() {
    const requestedId = animeId;
    episodeFilesLoading = true;
    try {
      const files = await getEpisodeFiles(requestedId);
      if (requestedId !== animeId) return;
      episodeFiles = files;
    } catch {
      if (requestedId === animeId) episodeFiles = [];
    } finally {
      if (requestedId === animeId) episodeFilesLoading = false;
    }
  }

  async function loadRelations() {
    const requestedId = animeId;
    relationsLoading = true;
    try {
      const r = await getAnimeRelations(requestedId);
      if (requestedId !== animeId) return;
      relations = r;
    } catch {
      if (requestedId === animeId) relations = [];
    } finally {
      if (requestedId === animeId) relationsLoading = false;
    }
  }

  let mappingFolder = false;
  let mapFolderMsg: string | null = null;

  async function handleRescanFiles() {
    rescanning = true;
    fileActionError = null;
    fileActionMessage = null;
    repairConfirming = false;
    try {
      // Re-read this show's own folders from disk (picks up added files, drops
      // deleted ones), then refresh the list.
      fileScanReport = await rescanAnimeFiles(animeId);
      await loadEpisodeFiles();
      if (fileScanReport.mapping_conflicts.length === 0) {
        fileActionMessage = 'Rescan complete. No mapping conflicts found.';
      }
    } catch (e) {
      fileActionError = e instanceof Error ? e.message : String(e);
    } finally {
      rescanning = false;
    }
  }

  function beginRepairMappings() {
    if (conflictGroups.repairable.length > 0) repairConfirming = true;
  }

  function cancelRepairMappings() {
    repairConfirming = false;
  }

  async function confirmRepairMappings() {
    if (repairing) return;
    repairing = true;
    fileActionError = null;
    try {
      const result = await repairAnimeFileMappings(animeId);
      await loadEpisodeFiles();
      fileScanReport = await rescanAnimeFiles(animeId);
      fileActionMessage = `Repaired ${result.repaired} file mapping${result.repaired === 1 ? '' : 's'}.`;
      repairConfirming = false;
    } catch (e) {
      fileActionError = e instanceof Error ? e.message : String(e);
    } finally {
      repairing = false;
    }
  }

  async function handleMapFolder() {
    if (mappingFolder) return;
    mapFolderMsg = null;
    let folder: string | null = null;
    try {
      folder = await pickFolder();
    } catch {
      return;
    }
    if (!folder) return; // cancelled
    mappingFolder = true;
    try {
      const n = await mapFolderToAnime(folder, animeId);
      await loadEpisodeFiles();
      mapFolderMsg = `Mapped ${n} file${n === 1 ? '' : 's'} from this folder`;
    } catch (e) {
      mapFolderMsg = e instanceof Error ? e.message : String(e);
    } finally {
      mappingFolder = false;
    }
  }

  let unmappingAll = false;

  async function handleUnmapFile(path: string) {
    try {
      await unmapKnownFiles([path]);
      episodeFiles = episodeFiles.filter((f) => f.file_path !== path);
    } catch { /* silent */ }
  }

  async function handleUnmapAll() {
    if (unmappingAll || episodeFiles.length === 0) return;
    unmappingAll = true;
    try {
      await unmapKnownFiles(episodeFiles.map((f) => f.file_path));
      episodeFiles = [];
    } catch { /* silent */ } finally {
      unmappingAll = false;
    }
  }

  function handlePlayFile(path: string) {
    openEpisodeFile(path);
  }

  // Reactive statements run once on init (in addition to on every later
  // animeId change), so this alone covers both first mount and navigating
  // to a different anime — an onMount(() => load()) alongside this would
  // double-fire every request on initial load.
  $: if (animeId) {
    load();
    loadAiring();
  }

  function seasonOf(path: string): number {
    const m = path.match(/[\\/ ._-]S(\d{1,2})E\d{1,3}/i) ?? path.match(/[\\/ ]Season\s*(\d{1,2})[\\/ ]/i);
    return m ? parseInt(m[1], 10) : 1;
  }

  // Sort files by (season, episode) so Season 1 always lists before Season 2.
  $: sortedEpisodeFiles = [...episodeFiles].sort((a, b) => {
    const sa = seasonOf(a.file_path);
    const sb = seasonOf(b.file_path);
    if (sa !== sb) return sa - sb;
    return (a.episode ?? 0) - (b.episode ?? 0);
  });

  $: episodesBySeason = (() => {
    const map = new Map<number, FileIndexEntry[]>();
    for (const f of sortedEpisodeFiles) {
      const s = seasonOf(f.file_path);
      const arr = map.get(s);
      if (arr) arr.push(f);
      else map.set(s, [f]);
    }
    return [...map.entries()].sort((a, b) => a[0] - b[0]);
  })();

  $: multiSeason = episodesBySeason.length > 1;

  function openFolder() {
    if (episodeFiles.length > 0) openContainingFolder(episodeFiles[0].file_path);
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

  function stripHtml(html: string): string {
    return html.replace(/<br\s*\/?>/gi, '\n').replace(/<[^>]*>/g, '').trim();
  }

  function formatDate(ts: number | null): string {
    if (!ts) return '—';
    return new Date(ts * 1000).toLocaleString();
  }
</script>

<section class="detail-view" aria-label="Anime detail">
  <div class="detail-topbar">
    <button
      class="back-btn"
      type="button"
      on:click={() => dispatch('back')}
      aria-label="Back"
    >
      <ArrowLeft size={15} /> Back
    </button>
    {#if confirmingDelete}
      <div class="delete-confirm-group">
        <button class="delete-btn" type="button" on:click={handleDelete} disabled={deleting} aria-label="Confirm delete from library">
          {deleting ? 'Deleting…' : 'Confirm delete?'}
        </button>
        <button class="cancel-btn" type="button" on:click={cancelDelete} disabled={deleting}>Cancel</button>
      </div>
    {:else}
      <button
        class="delete-btn"
        type="button"
        on:click={armDelete}
        aria-label="Delete from library"
      >
        <Trash2 size={14} /> Delete from library
      </button>
    {/if}
  </div>

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
          <p class="meta-item" title={airingLabel}>
            <span class="meta-label">Status</span>
            <span class="meta-value">{formatMediaStatus(detail.anime_status)}</span>
          </p>
          {#if nextAiring}
            <p class="meta-item airing" title={airingLabel}>
              <span class="meta-label">Next episode</span>
              <span class="meta-value">Ep {nextAiring.episode} · {new Date(nextAiring.airing_at * 1000).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })}</span>
              <span class="airing-countdown">in {formatCountdown(nextAiring.airing_at - nowTs)}</span>
            </p>
          {/if}
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
        <div class="detail-header">
          <h1 class="title">
            {pickTitle(detail)}
            <button class="anilist-link" on:click={() => openEpisodeFile(`https://anilist.co/anime/${detail.anime_id}`)} title="View on AniList" aria-label="View on AniList"><ExternalLink size={14} /></button>
          </h1>
          {#if titles.english && titles.english !== (titles.romaji ?? '')}
            <p class="alt-title">English: {titles.english}</p>
          {/if}
          {#if titles.native && titles.native !== (titles.romaji ?? '')}
            <p class="alt-title">Japanese: {titles.native}</p>
          {/if}
          {#if titles.synonyms && titles.synonyms.length > 0}
            <p class="alt-title">Also known as: {titles.synonyms.filter(s => s !== titles.romaji).join(', ')}</p>
          {/if}
        </div>

        {#if detail.synopsis}
          <p class="synopsis">{stripHtml(detail.synopsis)}</p>
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
                max={100}
                step={1}
                bind:value={draftScore}
                disabled={savingField === 'score'}
                aria-label="Score out of 100"
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

        {#if sonarrAvail}
          <div class="sonarr-section">
            <div class="section-header-row">
              <h2 class="section-heading">Sonarr</h2>
              <SonarrRemap sonarrId={sonarrAvail.sonarr_id} currentAnimeId={animeId} on:changed={loadSonarr} />
            </div>
            <div class="sonarr-detail">
              <div class="sonarr-field">
                <span class="field-label">Series</span>
                <span class="field-value">{sonarrAvail.sonarr_title}</span>
              </div>
              <div class="sonarr-field">
                <span class="field-label">Episodes</span>
                <span class="field-value">
                  {sonarrAvail.episode_file_count} / {sonarrAvail.episode_count} files on disk
                </span>
              </div>
              <div class="sonarr-field">
                <span class="field-label">Status</span>
                <span class="field-value">
                  {sonarrAvail.sonarr_status ? sonarrAvail.sonarr_status : 'Unknown'}
                  {#if sonarrAvail.monitored} · Monitored ✓{/if}
                  {#if sonarrAvail.next_airing} · Next airing: {new Date(sonarrAvail.next_airing * 1000).toLocaleDateString()}{/if}
                </span>
              </div>
              {#if sonarrAvail.path}
                <div class="sonarr-field">
                  <span class="field-label">Path</span>
                  <span class="field-value path">{sonarrAvail.path}</span>
                </div>
              {/if}
            </div>
          </div>
        {/if}

        {#if detail}
          <section class="card">
            <div class="section-header">
              <h3>Episode Files</h3>
              <div class="section-actions">
                <span class="file-count">{episodeFiles.length} files</span>
                {#if episodeFiles.length > 0}
                  <button class="action-btn small" on:click={openFolder}><FolderOpen size={13} /> Open folder</button>
                  <button class="action-btn small danger" on:click={handleUnmapAll} disabled={unmappingAll}>
                    {unmappingAll ? 'Unmapping…' : 'Unmap all'}
                  </button>
                {/if}
                <button class="action-btn small" on:click={handleRescanFiles} disabled={rescanning}>
                  {#if rescanning}Scanning…{:else}<RotateCw size={13} /> Rescan{/if}
                </button>
              </div>
            </div>

            <div class="manual-map">
              <button class="action-btn small" on:click={handleMapFolder} disabled={mappingFolder}>
                {#if mappingFolder}Mapping…{:else}<FolderInput size={13} /> Map folder…{/if}
              </button>
              <span class="map-folder-hint">Pick this show's series or season folder to map its files here.</span>
            </div>
            {#if mapFolderMsg}
              <p class="success-msg">{mapFolderMsg}</p>
            {/if}

            {#if fileScanReport && fileScanReport.mapping_conflicts.length > 0}
              <div class="mapping-conflict-warning">
                <p>
                  {fileScanReport.mapping_conflicts.length} conflicting file mapping{fileScanReport.mapping_conflicts.length === 1 ? '' : 's'} found.
                </p>
                <ul>
                  {#each fileScanReport.mapping_conflicts as conflict (conflict.file_path)}
                    <li>
                      <span>Ep {conflict.episode ?? '?'}</span>
                      <span>{conflict.current_anime_title} (#{conflict.current_anime_id})</span>
                      <span>{mappingSourceLabel(conflict.mapping_source)}</span>
                      <span class="ep-path">{conflict.file_path}</span>
                      {#if !conflict.repairable}<span class="protected-label">Manual mapping protected</span>{/if}
                    </li>
                  {/each}
                </ul>
                {#if conflictGroups.repairable.length > 0 && !repairConfirming}
                  <button class="action-btn small" on:click={beginRepairMappings}>Repair mappings</button>
                {:else if repairConfirming}
                  <div class="repair-confirm-row">
                    <span>Move {conflictGroups.repairable.length} eligible file{conflictGroups.repairable.length === 1 ? '' : 's'} to this anime?</span>
                    <button class="action-btn small" on:click={confirmRepairMappings} disabled={repairing}>
                      {repairing ? 'Repairing...' : 'Confirm repair'}
                    </button>
                    <button class="action-btn small" on:click={cancelRepairMappings} disabled={repairing}>Cancel</button>
                  </div>
                {/if}
                {#if conflictGroups.protected.length > 0}
                  <p class="muted">Protected manual mappings were not changed. Use Map folder to override them intentionally.</p>
                {/if}
              </div>
            {/if}
            {#if fileActionMessage}<p class="success-msg">{fileActionMessage}</p>{/if}
            {#if fileActionError}<p class="error-text">{fileActionError}</p>{/if}

            {#if episodeFilesLoading}
              <p class="muted">Loading…</p>
            {:else if episodeFiles.length > 0}
              <div class="episode-file-list">
                {#each episodesBySeason as [season, files] (season)}
                  {#if multiSeason}
                    <div class="season-header">Season {season}</div>
                  {/if}
                  {#each files as file (file.file_path)}
                    <div class="episode-file-row">
                      <span class="ep-num">Ep {file.episode ?? '?'}</span>
                      <span class="ep-path">{file.file_path}</span>
                      <button class="action-btn small" on:click={() => handlePlayFile(file.file_path)}><Play size={13} /> Play</button>
                      <button class="action-btn small danger" on:click={() => handleUnmapFile(file.file_path)} title="Remove this file from this anime">Unmap</button>
                    </div>
                  {/each}
                {/each}
              </div>
            {:else if !episodeFilesLoading}
              <p class="muted">No files found. Click Rescan to scan library folders.</p>
            {/if}
          </section>
        {/if}

        {#if relations.length > 0 || relationsLoading}
          <section class="card">
            <div class="section-header">
              <h3>Related Anime</h3>
            </div>
            {#if relationsLoading}
              <p class="muted">Loading…</p>
            {:else}
              <div class="relations-list">
                {#each relations as rel}
                  <div
                    class="relation-row clickable"
                    role="button"
                    tabindex="0"
                    on:click={() => dispatch('select', { anime_id: rel.id })}
                    on:keydown={(e) => e.key === 'Enter' && dispatch('select', { anime_id: rel.id })}
                  >
                    {#if rel.image_url}<img class="relation-thumb" src={rel.image_url} alt={rel.title} loading="lazy" />{/if}
                    <div class="relation-info">
                      <span class="relation-title">{rel.title}</span>
                      <span class="relation-type">{rel.relation_type.replace('_', ' ')} {rel.format ? `· ${rel.format}` : ''}</span>
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          </section>
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
    overflow-x: hidden;
  }

  .detail-topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
  }

  .back-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    justify-self: start;
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    border-radius: 999px;
    padding: 0.45rem 0.9rem;
    background: rgba(var(--color-accent-rgb), 0.12);
    color: #e9eefc;
    cursor: pointer;
    font-size: 0.85rem;
  }

  .back-btn:hover {
    background: rgba(var(--color-accent-rgb), 0.22);
  }

  .delete-confirm-group {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .delete-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    border: 1px solid rgba(var(--color-danger-rgb), 0.4);
    border-radius: 999px;
    padding: 0.45rem 0.9rem;
    background: rgba(var(--color-danger-rgb), 0.12);
    color: var(--color-danger-text);
    cursor: pointer;
    font-size: 0.85rem;
  }

  .delete-btn:hover:not(:disabled) {
    background: rgba(var(--color-danger-rgb), 0.24);
  }

  .delete-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .cancel-btn {
    border: 1px solid rgba(var(--color-accent-rgb), 0.3);
    border-radius: 999px;
    padding: 0.45rem 0.9rem;
    background: transparent;
    color: var(--color-muted);
    cursor: pointer;
    font-size: 0.85rem;
  }

  .cancel-btn:hover:not(:disabled) {
    background: rgba(var(--color-accent-rgb), 0.1);
    color: var(--color-text);
  }

  .back-btn:focus {
    outline: 2px solid rgba(var(--color-accent-rgb), 0.5);
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
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
    background: rgba(255, 255, 255, 0.04);
  }

  .cover-placeholder {
    width: 100%;
    aspect-ratio: 2 / 3;
    border-radius: 16px;
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
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
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
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

  .detail-header {
    display: grid;
    gap: 0.15rem;
  }

  .title {
    margin: 0;
    font-size: 1.6rem;
    font-weight: 700;
    line-height: 1.2;
    color: var(--color-text);
  }

  .alt-title {
    font-size: 0.85rem;
    color: var(--color-muted);
    margin-top: 0.15rem;
    margin-bottom: 0;
  }

  .anilist-link {
    font-size: 0.9rem;
    color: var(--color-accent);
    text-decoration: none;
    margin-left: 0.5rem;
    opacity: 0.7;
  }
  .anilist-link:hover { opacity: 1; }

  .synopsis {
    margin: 0;
    line-height: 1.55;
    color: #c8d2e0;
    white-space: pre-line;
  }

  .synopsis.muted {
    color: var(--color-muted);
  }

  .empty-prompt {
    padding: 0.75rem 1rem;
    border: 1px dashed rgba(var(--color-accent-rgb), 0.35);
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
    border: 1px solid rgba(var(--color-error-rgb), 0.35);
    border-radius: 14px;
    background: rgba(var(--color-error-rgb), 0.08);
  }

  .editors {
    display: grid;
    gap: 0.9rem;
    padding: 1rem;
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
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
    border: 1px solid rgba(var(--color-accent-rgb), 0.25);
    border-radius: 8px;
    padding: 0.5rem 0.65rem;
    color: var(--color-text);
    font-size: 0.9rem;
  }

  .num-input:focus {
    outline: 2px solid rgba(var(--color-accent-rgb), 0.5);
    outline-offset: 2px;
  }

  .select-input {
    min-width: 10rem;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(var(--color-accent-rgb), 0.25);
    border-radius: 8px;
    padding: 0.5rem 0.65rem;
    color: var(--color-text);
    font-size: 0.9rem;
  }

  .select-input option {
    background: #141820;
    color: var(--color-text);
  }

  .select-input:focus {
    outline: 2px solid rgba(var(--color-accent-rgb), 0.5);
    outline-offset: 2px;
  }

  .step-btn {
    width: 2.2rem;
    height: 2.2rem;
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    border-radius: 8px;
    background: rgba(var(--color-accent-rgb), 0.12);
    color: #e9eefc;
    cursor: pointer;
    font-size: 1rem;
    line-height: 1;
  }

  .step-btn:hover {
    background: rgba(var(--color-accent-rgb), 0.22);
  }

  .step-btn:focus {
    outline: 2px solid rgba(var(--color-accent-rgb), 0.5);
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
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    border-radius: 999px;
    padding: 0.5rem 0.9rem;
    background: rgba(var(--color-accent-rgb), 0.18);
    color: #e9eefc;
    cursor: pointer;
    font-size: 0.82rem;
  }

  .action-btn:hover {
    background: rgba(var(--color-accent-rgb), 0.28);
  }

  .action-btn:focus {
    outline: 2px solid rgba(var(--color-accent-rgb), 0.5);
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
    border: 1px solid rgba(var(--color-accent-rgb), 0.12);
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
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  .sonarr-section {
    border: 1px solid rgba(var(--color-accent-rgb), 0.15);
    border-radius: 14px;
    padding: 1rem;
    background: rgba(255, 255, 255, 0.03);
  }

  .section-header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.75rem;
  }

  .sonarr-detail {
    display: grid;
    gap: 0.5rem;
  }

  .sonarr-field {
    display: grid;
    gap: 0.15rem;
  }

  .field-label {
    font-size: 0.72rem;
    color: var(--color-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .field-value {
    font-size: 0.85rem;
    color: #c8d2e0;
  }

  .field-value.path {
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
    font-size: 0.75rem;
    color: var(--color-muted);
    overflow-wrap: anywhere;
  }

  .section-header { display: flex; align-items: center; justify-content: space-between; }
  .section-actions { display: flex; align-items: center; gap: 0.5rem; }
  .file-count { font-size: 0.78rem; color: var(--color-muted); }
  .manual-map { display: flex; gap: 0.5rem; align-items: center; margin-top: 0.5rem; flex-wrap: wrap; }
  .map-folder-hint { font-size: 0.72rem; color: var(--color-muted); }
  .success-msg { color: var(--color-success); font-size: 0.8rem; margin: 0.3rem 0 0; }
  .mapping-conflict-warning {
    margin-top: 0.5rem; padding: 0.5rem 0.6rem; border-radius: 6px;
    border: 1px solid rgba(var(--color-warning-rgb, 234, 179, 8), 0.35);
    background: rgba(var(--color-warning-rgb, 234, 179, 8), 0.08);
    font-size: 0.8rem;
  }
  .mapping-conflict-warning > p { margin: 0 0 0.4rem; font-weight: 600; }
  .mapping-conflict-warning ul { list-style: none; margin: 0 0 0.5rem; padding: 0; display: grid; gap: 0.3rem; }
  .mapping-conflict-warning li {
    display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap;
    padding: 0.3rem 0.4rem; border-radius: 6px; background: rgba(255,255,255,0.02);
  }
  .protected-label { color: var(--color-muted); font-style: italic; }
  .repair-confirm-row { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }
  .episode-file-list { display: grid; gap: 0.35rem; margin-top: 0.5rem; }
  .episode-file-row { display: flex; align-items: center; gap: 0.6rem; padding: 0.35rem 0.5rem; border: 1px solid rgba(var(--color-accent-rgb),0.08); border-radius: 6px; background: rgba(255,255,255,0.02); font-size: 0.82rem; min-width: 0; }
  .ep-num { font-weight: 600; color: var(--color-accent); min-width: 2.5rem; }
  .ep-path { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--color-muted); font-family: monospace; font-size: 0.75rem; }
  .action-btn.small { padding: 0.25rem 0.6rem; font-size: 0.75rem; }
  .action-btn.danger { border-color: rgba(var(--color-danger-rgb), 0.4); background: rgba(var(--color-danger-rgb), 0.12); color: var(--color-danger-text); }
  .action-btn.danger:hover:not(:disabled) { background: rgba(var(--color-danger-rgb), 0.24); }
  .meta-item.airing { flex-wrap: wrap; }
  .airing-countdown { font-size: 0.75rem; color: var(--color-accent); font-weight: 600; width: 100%; text-align: right; }
  .season-header { font-size: 0.75rem; font-weight: 700; color: var(--color-accent); text-transform: uppercase; letter-spacing: 0.06em; margin: 0.5rem 0 0.15rem; }
  .season-header:first-child { margin-top: 0; }
  .relations-list { display: grid; gap: 0.4rem; margin-top: 0.5rem; }
  .relation-row { display: flex; gap: 0.6rem; align-items: center; padding: 0.4rem 0.5rem; border: 1px solid rgba(var(--color-accent-rgb),0.08); border-radius: 6px; background: rgba(255,255,255,0.02); min-width: 0; }
  .relation-row.clickable { cursor: pointer; transition: border-color 0.15s, background 0.15s; }
  .relation-row.clickable:hover { border-color: rgba(var(--color-accent-rgb),0.3); background: rgba(var(--color-accent-rgb),0.06); }
  .relation-row.clickable:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 1px; }
  .relation-thumb { width: 2rem; height: 2.8rem; border-radius: 4px; object-fit: cover; flex-shrink: 0; }
  .relation-info { display: flex; flex-direction: column; gap: 0.1rem; min-width: 0; }
  .relation-title { font-size: 0.85rem; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .relation-type { font-size: 0.72rem; color: var(--color-muted); text-transform: capitalize; }
</style>
