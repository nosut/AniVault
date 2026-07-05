<script lang="ts">
  import { onMount } from 'svelte';
  import {
    listKnownFiles,
    rematchUnmappedFiles,
    setKnownFileIgnored,
    deleteKnownFile,
    setKnownFilesIgnored,
    deleteKnownFiles,
    setKnownFileMappings,
    searchLibrary,
    searchAnime,
    importAnilistAnime,
    deepMatchViaAnilist,
    type FileIndexEntry,
  } from './api';

  type Filter = 'all' | 'unmapped' | 'mapped' | 'ignored';
  type MapSource = 'library' | 'anilist';
  interface Group {
    key: string;
    files: FileIndexEntry[];
  }
  interface MapCandidate {
    anime_id: number;
    title: string;
    from_anilist: boolean;
  }

  let entries: FileIndexEntry[] = [];
  let loading = true;
  let error: string | null = null;

  let search = '';
  let filter: Filter = 'all';

  let rematching = false;
  let deepMatching = false;
  let statusMsg: string | null = null;

  // Selection (set of file_path). Reassigned on every mutation so Svelte reacts.
  let selected = new Set<string>();
  let collapsed = new Set<string>();

  // Bulk-map panel state
  let mapOpen = false;
  let mapSource: MapSource = 'library';
  let mapQuery = '';
  let mapResults: MapCandidate[] = [];
  let mapSearching = false;
  let mapSelected: MapCandidate | null = null;
  let mapOffset = 0;
  let mapSaving = false;
  let mapError: string | null = null;

  const filterDefs: { id: Filter; label: string }[] = [
    { id: 'all', label: 'All' },
    { id: 'unmapped', label: 'Unmapped' },
    { id: 'mapped', label: 'Mapped' },
    { id: 'ignored', label: 'Ignored' },
  ];

  async function load() {
    loading = true;
    error = null;
    try {
      entries = await listKnownFiles(5000);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  function basename(p: string): string {
    const parts = p.split(/[\\/]/);
    return parts[parts.length - 1] || p;
  }

  // Derive a series+season key: the title portion before the SxxExx marker plus
  // its season, so different seasons of a show form separate groups (each maps to
  // its own AniList entry). Falls back to parent folder, then the bare filename.
  function seriesKey(path: string): string {
    const name = basename(path).replace(/\.[^.]+$/, '');
    const m = name.match(/^(.*?)[ ._-]+S(\d{1,2})E\d{1,3}/i);
    if (m && m[1].trim()) {
      const title = m[1].replace(/[._]+/g, ' ').trim();
      return `${title} — Season ${parseInt(m[2], 10)}`;
    }
    const parts = path.split(/[\\/]/);
    if (parts.length >= 2) return parts[parts.length - 2];
    return name;
  }

  function statusOf(e: FileIndexEntry): Filter {
    if (e.ignored) return 'ignored';
    return e.anime_id != null ? 'mapped' : 'unmapped';
  }

  $: counts = {
    all: entries.length,
    unmapped: entries.filter((e) => statusOf(e) === 'unmapped').length,
    mapped: entries.filter((e) => statusOf(e) === 'mapped').length,
    ignored: entries.filter((e) => statusOf(e) === 'ignored').length,
  };

  $: visible = entries.filter((e) => {
    if (filter !== 'all' && statusOf(e) !== filter) return false;
    if (search.trim()) {
      const q = search.trim().toLowerCase();
      if (!`${e.file_path} ${e.anime_id ?? ''}`.toLowerCase().includes(q)) return false;
    }
    return true;
  });

  $: groups = buildGroups(visible);

  function buildGroups(list: FileIndexEntry[]): Group[] {
    const map = new Map<string, FileIndexEntry[]>();
    for (const e of list) {
      const k = seriesKey(e.file_path);
      const arr = map.get(k);
      if (arr) arr.push(e);
      else map.set(k, [e]);
    }
    return [...map.entries()]
      .map(([key, files]) => ({
        key,
        files: files.sort((a, b) => basename(a.file_path).localeCompare(basename(b.file_path))),
      }))
      .sort((a, b) => a.key.localeCompare(b.key));
  }

  // Consensus mapping label for a group.
  function groupBadge(g: Group): string {
    const active = g.files.filter((f) => !f.ignored);
    if (active.length === 0) return 'All ignored';
    const ids = new Set(active.map((f) => f.anime_id ?? -1));
    if (ids.size === 1) {
      const id = [...ids][0];
      return id === -1 ? 'Unmapped' : `Mapped → #${id}`;
    }
    return 'Mixed';
  }

  $: selectedCount = selected.size;
  $: visiblePaths = visible.map((e) => e.file_path);
  $: allVisibleSelected = visiblePaths.length > 0 && visiblePaths.every((p) => selected.has(p));

  function toggleFile(path: string) {
    const next = new Set(selected);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    selected = next;
  }

  function groupSelectedState(g: Group): 'none' | 'some' | 'all' {
    let sel = 0;
    for (const f of g.files) if (selected.has(f.file_path)) sel++;
    if (sel === 0) return 'none';
    return sel === g.files.length ? 'all' : 'some';
  }

  function toggleGroup(g: Group) {
    const next = new Set(selected);
    const all = g.files.every((f) => next.has(f.file_path));
    for (const f of g.files) {
      if (all) next.delete(f.file_path);
      else next.add(f.file_path);
    }
    selected = next;
  }

  function toggleAllVisible() {
    const next = new Set(selected);
    if (allVisibleSelected) {
      for (const p of visiblePaths) next.delete(p);
    } else {
      for (const p of visiblePaths) next.add(p);
    }
    selected = next;
  }

  function clearSelection() {
    selected = new Set();
  }

  function toggleCollapse(key: string) {
    const next = new Set(collapsed);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    collapsed = next;
  }

  function selectedEntries(): FileIndexEntry[] {
    return entries.filter((e) => selected.has(e.file_path));
  }

  async function afterMutation(msg: string) {
    statusMsg = msg;
    clearSelection();
    await load();
  }

  async function handleRematch() {
    rematching = true;
    statusMsg = null;
    try {
      const n = await rematchUnmappedFiles();
      await load();
      statusMsg = `${n} files auto-matched`;
    } catch (e) {
      statusMsg = e instanceof Error ? e.message : String(e);
    } finally {
      rematching = false;
    }
  }

  async function handleDeepMatch() {
    deepMatching = true;
    statusMsg = null;
    error = null;
    try {
      const r = await deepMatchViaAnilist();
      await load();
      const unmatchedNote =
        r.unmatched.length > 0
          ? ` ${r.unmatched.length} series still unmapped — map those manually.`
          : '';
      statusMsg = `Matched ${r.groups_matched} of ${r.groups_total} series (${r.files_mapped} files).${unmatchedNote}`;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      deepMatching = false;
    }
  }

  // ── Single-file quick actions ──
  async function handleIgnoreOne(e: FileIndexEntry, ignored: boolean) {
    try {
      await setKnownFileIgnored(e.file_path, ignored);
      await load();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }
  async function handleRemoveOne(e: FileIndexEntry) {
    try {
      await deleteKnownFile(e.file_path);
      const next = new Set(selected);
      next.delete(e.file_path);
      selected = next;
      await load();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  // ── Bulk actions ──
  async function bulkIgnore(ignored: boolean) {
    const paths = selectedEntries().map((e) => e.file_path);
    if (paths.length === 0) return;
    try {
      const n = await setKnownFilesIgnored(paths, ignored);
      await afterMutation(`${n} files ${ignored ? 'ignored' : 'un-ignored'}`);
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  async function bulkRemove() {
    const paths = selectedEntries().map((e) => e.file_path);
    if (paths.length === 0) return;
    try {
      const n = await deleteKnownFiles(paths);
      await afterMutation(`${n} files removed`);
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  function openMap() {
    mapOpen = true;
    mapQuery = '';
    mapResults = [];
    mapSelected = null;
    mapOffset = 0;
    mapError = null;
  }
  function closeMap() {
    mapOpen = false;
    mapSelected = null;
    mapError = null;
  }

  function setMapSource(src: MapSource) {
    if (mapSource === src) return;
    mapSource = src;
    mapResults = [];
    mapSelected = null;
    mapError = null;
  }

  async function handleMapSearch() {
    if (!mapQuery.trim()) return;
    mapSearching = true;
    mapError = null;
    mapSelected = null;
    try {
      if (mapSource === 'anilist') {
        const res = await searchAnime(mapQuery.trim());
        mapResults = res.map((r) => ({ anime_id: r.id, title: r.title, from_anilist: true }));
      } else {
        const res = await searchLibrary(mapQuery.trim(), null, 25, 0);
        mapResults = res.map((r) => ({ anime_id: r.anime_id, title: r.title, from_anilist: false }));
      }
    } catch (err) {
      mapError = err instanceof Error ? err.message : String(err);
    } finally {
      mapSearching = false;
    }
  }

  async function applyMap() {
    if (!mapSelected) return;
    const sel = selectedEntries();
    if (sel.length === 0) return;
    mapSaving = true;
    mapError = null;
    // Capture the target before closeMap() clears mapSelected — otherwise the
    // status message (and the reload it triggers) would throw on a null read and
    // the list wouldn't refresh until navigating away.
    const target = mapSelected;
    try {
      // If the pick came from AniList, import it into the library first so the
      // mapping resolves to a real local anime.
      if (target.from_anilist) {
        await importAnilistAnime(target.anime_id);
      }
      const mappings = sel.map((e) => ({
        file_path: e.file_path,
        anime_id: target.anime_id,
        episode: Math.max(0, (e.episode ?? 1) + mapOffset),
      }));
      const n = await setKnownFileMappings(mappings);
      closeMap();
      await afterMutation(`${n} files mapped to ${target.title}`);
    } catch (err) {
      mapError = err instanceof Error ? err.message : String(err);
    } finally {
      mapSaving = false;
    }
  }
</script>

<section class="card">
  <div class="section-header">
    <h3>File Management</h3>
    <div class="header-actions">
      <button class="action-btn" on:click={handleRematch} disabled={rematching || deepMatching}>
        {rematching ? 'Matching…' : 'Re-match unmapped'}
      </button>
      <button class="action-btn" on:click={handleDeepMatch} disabled={deepMatching || rematching}>
        {deepMatching ? 'Matching via AniList… (this can take a few minutes)' : 'Match via AniList'}
      </button>
    </div>
  </div>
  <p class="hint">
    Files are grouped by series. Select whole groups (or individual files) with the checkboxes,
    then use the bulk bar to map, ignore, or remove them. Mapping keeps each file's episode number
    from its filename (adjust with the offset for multi-season sets). Removing a file that still
    exists on disk re-adds it on the next Library scan — use <strong>Ignore</strong> to suppress it
    permanently.
  </p>

  {#if statusMsg}
    <p class="success-msg">{statusMsg}</p>
  {/if}

  <div class="toolbar">
    <input class="form-input search" type="text" placeholder="Filter by path or anime id…" bind:value={search} />
    <div class="chips" role="tablist" aria-label="Filter files">
      {#each filterDefs as def (def.id)}
        <button type="button" class="chip" class:active={filter === def.id} on:click={() => (filter = def.id)}>
          {def.label} <span class="chip-count">{counts[def.id]}</span>
        </button>
      {/each}
    </div>
  </div>

  {#if !loading && !error && visible.length > 0}
    <div class="select-all-row">
      <label class="checkbox-label">
        <input type="checkbox" checked={allVisibleSelected} on:change={toggleAllVisible} />
        Select all visible ({visible.length})
      </label>
    </div>
  {/if}

  <!-- Bulk action bar -->
  {#if selectedCount > 0}
    <div class="bulk-bar">
      <span class="bulk-count">{selectedCount} selected</span>
      <div class="bulk-actions">
        <button class="action-btn" on:click={openMap}>Map to anime…</button>
        <button class="mini-btn" on:click={() => bulkIgnore(true)}>Ignore</button>
        <button class="mini-btn" on:click={() => bulkIgnore(false)}>Un-ignore</button>
        <button class="mini-btn danger" on:click={bulkRemove}>Remove</button>
        <button class="mini-btn" on:click={clearSelection}>Clear</button>
      </div>
    </div>

    {#if mapOpen}
      <div class="map-editor">
        <div class="map-source-toggle" role="tablist" aria-label="Search source">
          <button type="button" class="chip" class:active={mapSource === 'library'} on:click={() => setMapSource('library')}>
            My library
          </button>
          <button type="button" class="chip" class:active={mapSource === 'anilist'} on:click={() => setMapSource('anilist')}>
            AniList
          </button>
          {#if mapSource === 'anilist'}
            <span class="map-source-hint">Selecting an AniList result imports it to your library as “Watching”.</span>
          {/if}
        </div>
        <div class="map-search-row">
          <input
            class="form-input"
            type="text"
            placeholder={mapSource === 'anilist' ? 'Search AniList…' : 'Search your library…'}
            bind:value={mapQuery}
            on:keydown={(ev) => ev.key === 'Enter' && handleMapSearch()}
          />
          <button class="action-btn" on:click={handleMapSearch} disabled={mapSearching || !mapQuery.trim()}>
            {mapSearching ? 'Searching…' : 'Search'}
          </button>
        </div>

        {#if mapResults.length > 0}
          <ul class="map-results" role="listbox">
            {#each mapResults as r (r.anime_id)}
              <li>
                <button
                  type="button"
                  class="map-result"
                  class:selected={mapSelected?.anime_id === r.anime_id}
                  on:click={() => (mapSelected = r)}
                >
                  <span class="map-result-title">{r.title}</span>
                  <span class="map-result-id">#{r.anime_id}</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}

        <div class="map-confirm-row">
          <label class="map-ep-label">
            Episode offset
            <input class="form-input ep-input" type="number" bind:value={mapOffset} />
          </label>
          <span class="map-selected-label">
            {mapSelected ? `→ ${mapSelected.title}` : 'Select an anime above'}
          </span>
          <button class="action-btn" on:click={applyMap} disabled={mapSaving || !mapSelected}>
            {mapSaving ? 'Applying…' : `Apply to ${selectedCount} file${selectedCount === 1 ? '' : 's'}`}
          </button>
          <button class="mini-btn" on:click={closeMap}>Cancel</button>
        </div>
        {#if mapError}<p class="error">{mapError}</p>{/if}
      </div>
    {/if}
  {/if}

  {#if loading}
    <p class="muted">Loading…</p>
  {:else if error}
    <div class="error-row"><p class="error">{error}</p><button class="btn-retry" on:click={load}>Retry</button></div>
  {:else if entries.length === 0}
    <p class="muted">No indexed files yet. Add folders in the Library tab and run a scan.</p>
  {:else if groups.length === 0}
    <p class="muted">No files match this filter.</p>
  {:else}
    <div class="group-list">
      {#each groups as g (g.key)}
        {@const gState = groupSelectedState(g)}
        <div class="group">
          <div class="group-header">
            <input
              type="checkbox"
              class="group-check"
              checked={gState === 'all'}
              indeterminate={gState === 'some'}
              on:change={() => toggleGroup(g)}
              aria-label="Select group {g.key}"
            />
            <button class="group-title-btn" on:click={() => toggleCollapse(g.key)}>
              <span class="group-caret">{collapsed.has(g.key) ? '▶' : '▼'}</span>
              <span class="group-title">{g.key}</span>
              <span class="group-count">{g.files.length}</span>
            </button>
            <span class="group-badge">{groupBadge(g)}</span>
          </div>

          {#if !collapsed.has(g.key)}
            <ul class="file-list" role="list">
              {#each g.files as e (e.file_path)}
                <li class="file-item" class:is-ignored={e.ignored} class:is-selected={selected.has(e.file_path)}>
                  <input
                    type="checkbox"
                    checked={selected.has(e.file_path)}
                    on:change={() => toggleFile(e.file_path)}
                    aria-label="Select file"
                  />
                  <span class="file-name" title={e.file_path}>{basename(e.file_path)}</span>
                  <span class="file-badge {statusOf(e)}">
                    {#if e.ignored}
                      Ignored
                    {:else if e.anime_id != null}
                      #{e.anime_id} · ep {e.episode ?? '?'} · {e.confidence}%
                    {:else}
                      Unmapped
                    {/if}
                  </span>
                  <div class="file-actions">
                    {#if !e.ignored}
                      <button class="mini-btn" on:click={() => handleIgnoreOne(e, true)}>Ignore</button>
                    {:else}
                      <button class="mini-btn" on:click={() => handleIgnoreOne(e, false)}>Un-ignore</button>
                    {/if}
                    <button class="mini-btn danger" on:click={() => handleRemoveOne(e)}>Remove</button>
                  </div>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  .card {
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: var(--radius-card);
    padding: 1.25rem;
    background: rgba(255, 255, 255, 0.04);
    display: grid;
    gap: 0.75rem;
  }
  .section-header { display: flex; align-items: center; justify-content: space-between; gap: 1rem; flex-wrap: wrap; }
  .header-actions { display: flex; gap: 0.5rem; flex-wrap: wrap; }
  h3 { margin: 0; font-size: 1rem; font-weight: 700; color: var(--color-text); }
  .hint { margin: 0; font-size: 0.78rem; color: var(--color-muted); }
  .muted { color: var(--color-muted); font-size: 0.85rem; margin: 0; }
  .error { color: var(--color-error, #ff9d9d); font-size: 0.82rem; margin: 0; }
  .error-row { display: flex; align-items: center; justify-content: space-between; gap: 1rem; }
  .success-msg { color: #7ee87e; font-size: 0.82rem; margin: 0; }

  .toolbar { display: grid; gap: 0.6rem; }
  .search { width: 100%; }
  .chips { display: flex; gap: 0.4rem; flex-wrap: wrap; }
  .chip {
    border: 1px solid rgba(143, 183, 255, 0.25); border-radius: 999px; padding: 0.3rem 0.7rem;
    font-size: 0.75rem; background: transparent; color: var(--color-muted); cursor: pointer; font-family: inherit;
  }
  .chip:hover { color: var(--color-text); }
  .chip.active { background: rgba(143, 183, 255, 0.2); color: #e9eefc; border-color: rgba(143, 183, 255, 0.5); }
  .chip-count { opacity: 0.7; font-variant-numeric: tabular-nums; }

  .select-all-row { font-size: 0.78rem; }
  .checkbox-label { display: flex; align-items: center; gap: 0.4rem; color: var(--color-muted); cursor: pointer; }

  .bulk-bar {
    position: sticky; top: 0; z-index: 2;
    display: flex; align-items: center; justify-content: space-between; gap: 0.75rem; flex-wrap: wrap;
    padding: 0.55rem 0.8rem; border: 1px solid rgba(143, 183, 255, 0.4); border-radius: 10px;
    background: rgba(30, 40, 66, 0.96);
  }
  .bulk-count { font-size: 0.82rem; color: #e9eefc; font-weight: 600; }
  .bulk-actions { display: flex; gap: 0.4rem; flex-wrap: wrap; }

  .group-list { display: grid; gap: 0.5rem; }
  .group { border: 1px solid rgba(143, 183, 255, 0.12); border-radius: 8px; overflow: hidden; }
  .group-header {
    display: flex; align-items: center; gap: 0.55rem; padding: 0.45rem 0.6rem;
    background: rgba(143, 183, 255, 0.06);
  }
  .group-check { flex-shrink: 0; }
  .group-title-btn {
    display: flex; align-items: center; gap: 0.5rem; flex: 1; min-width: 0;
    background: transparent; border: none; color: var(--color-text); cursor: pointer;
    font-family: inherit; font-size: 0.85rem; text-align: left; padding: 0;
  }
  .group-caret { color: var(--color-muted); font-size: 0.7rem; flex-shrink: 0; }
  .group-title { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .group-count {
    flex-shrink: 0; font-size: 0.7rem; color: var(--color-muted);
    background: rgba(255, 255, 255, 0.06); border-radius: 999px; padding: 0.05rem 0.45rem;
  }
  .group-badge { flex-shrink: 0; font-size: 0.72rem; color: var(--color-accent); }

  .file-list { list-style: none; padding: 0.25rem 0.4rem 0.4rem; margin: 0; display: grid; gap: 0.2rem; }
  .file-item {
    display: grid; grid-template-columns: auto 1fr auto auto; gap: 0.6rem; align-items: center;
    padding: 0.3rem 0.4rem; border-radius: 6px;
  }
  .file-item.is-selected { background: rgba(143, 183, 255, 0.1); }
  .file-item.is-ignored { opacity: 0.55; }
  .file-name {
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: 0.76rem; color: var(--color-text);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0;
  }
  .file-badge { font-size: 0.7rem; white-space: nowrap; }
  .file-badge.mapped { color: var(--color-accent); }
  .file-badge.unmapped { color: #f0c040; }
  .file-badge.ignored { color: var(--color-muted); }
  .file-actions { display: flex; gap: 0.3rem; }

  .mini-btn {
    border: 1px solid rgba(143, 183, 255, 0.25); border-radius: 999px; padding: 0.22rem 0.55rem;
    font-size: 0.7rem; cursor: pointer; background: rgba(143, 183, 255, 0.1); color: #e9eefc;
    font-family: inherit; white-space: nowrap;
  }
  .mini-btn:hover:not(:disabled) { background: rgba(143, 183, 255, 0.22); }
  .mini-btn.danger { border-color: rgba(255, 130, 130, 0.4); background: rgba(255, 130, 130, 0.1); color: #ffb0b0; }
  .mini-btn.danger:hover { background: rgba(255, 130, 130, 0.2); }

  .map-editor {
    padding: 0.7rem 0.8rem; border: 1px solid rgba(143, 183, 255, 0.2); border-radius: 10px;
    background: rgba(143, 183, 255, 0.05); display: grid; gap: 0.6rem;
  }
  .map-source-toggle { display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; }
  .map-source-hint { font-size: 0.72rem; color: var(--color-muted); }
  .map-search-row { display: flex; gap: 0.5rem; }
  .map-search-row .form-input { flex: 1; }
  .map-results { list-style: none; padding: 0; margin: 0; display: grid; gap: 0.2rem; max-height: 14rem; overflow-y: auto; }
  .map-result {
    width: 100%; display: flex; justify-content: space-between; gap: 0.75rem; align-items: center;
    border: 1px solid rgba(143, 183, 255, 0.12); border-radius: 6px; padding: 0.4rem 0.6rem;
    background: rgba(255, 255, 255, 0.03); color: var(--color-text); cursor: pointer; font-family: inherit;
    font-size: 0.8rem; text-align: left;
  }
  .map-result:hover { border-color: rgba(143, 183, 255, 0.35); }
  .map-result.selected { background: rgba(143, 183, 255, 0.2); border-color: rgba(143, 183, 255, 0.55); }
  .map-result-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .map-result-id { color: var(--color-muted); flex-shrink: 0; }

  .map-confirm-row { display: flex; align-items: center; gap: 0.75rem; flex-wrap: wrap; }
  .map-ep-label { display: flex; align-items: center; gap: 0.4rem; font-size: 0.76rem; color: var(--color-muted); }
  .ep-input { width: 5rem; }
  .map-selected-label { flex: 1; font-size: 0.78rem; color: var(--color-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .action-btn {
    border: 1px solid rgba(143, 183, 255, 0.35); border-radius: 999px; padding: 0.4rem 0.85rem;
    background: rgba(143, 183, 255, 0.12); color: #e9eefc; cursor: pointer; font-size: 0.8rem;
    font-family: inherit; white-space: nowrap;
  }
  .action-btn:hover:not(:disabled) { background: rgba(143, 183, 255, 0.22); }
  .action-btn:disabled { opacity: 0.45; cursor: default; }

  .btn-retry {
    border: 1px solid rgba(143, 183, 255, 0.35); border-radius: 999px; padding: 0.35rem 0.75rem;
    font-size: 0.78rem; cursor: pointer; background: rgba(143, 183, 255, 0.18); color: #e9eefc; font-family: inherit;
  }

  .form-input {
    border: 1px solid rgba(143, 183, 255, 0.25); border-radius: 8px; padding: 0.5rem 0.7rem;
    background: rgba(255, 255, 255, 0.06); color: var(--color-text); font-size: 0.85rem; font-family: inherit;
  }
  .form-input:focus { outline: 2px solid rgba(143, 183, 255, 0.4); outline-offset: 1px; }
</style>
