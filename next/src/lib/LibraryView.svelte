<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { createEventDispatcher } from 'svelte';
  import { searchLibrary, updateListEntry, deleteAnime, getEpisodeFiles, openEpisodeFile, openContainingFolder, scanLibraryFolders, getLibraryStats, getCalendar, type LibraryEntry, type FileIndexEntry, type LibraryStats, type EngineEvent, type CalendarEntry } from './api';
  import {
    normalizeStatusFilter, groupBySeason, flattenGroups, asDisplayRows,
    seasonSortVal, getCurrentSeason,
    nextAiringByAnime, formatAiringCountdown, nextAiringSortVal,
  } from './libraryUi';
  import { LayoutGrid, List, ChevronUp, ChevronDown, ChevronRight, ChevronLeft, Play, FolderOpen, RotateCw, Trash2 } from 'lucide-svelte';

  export let events: EngineEvent[] = [];

  const dispatch = createEventDispatcher<{ select: { anime_id: number } }>();

  // Every value a tab can select. Declared here rather than derived from
  // `statusOptions` because that constant is defined further down the file,
  // after this function runs.
  const KNOWN_STATUS_FILTERS: (string | null)[] = [
    null, 'watching', 'completed', 'on_hold', 'dropped', 'plan_to_watch',
  ];

  function loadPersistedFilter(): string | null {
    try {
      return normalizeStatusFilter(
        localStorage.getItem('anivault-library-filter'),
        KNOWN_STATUS_FILTERS,
      );
    } catch { return null; }
  }

  function persistFilter(value: string | null) {
    try { localStorage.setItem('anivault-library-filter', value ?? ''); }
    catch {}
  }

  function loadPref(key: string, fallback: string): string {
    try { return localStorage.getItem(key) ?? fallback; }
    catch { return fallback; }
  }
  function persistPref(key: string, value: string) {
    try { localStorage.setItem(key, value); } catch {}
  }

  let query = loadPref('anivault-library-query', '');
  let statusFilter: string | null = loadPersistedFilter();
  let entries: LibraryEntry[] = [];
  let loading = false;
  let error = '';
  let episodeFilesMap = new Map<number, FileIndexEntry[]>();
  let stats: LibraryStats | null = null;

  let calendar: CalendarEntry[] = [];
  let nowSec = Math.floor(Date.now() / 1000);
  let clockTimer: ReturnType<typeof setInterval> | undefined;
  let calendarTimer: ReturnType<typeof setInterval> | undefined;

  // Cached behind CALENDAR_CACHE_TTL_SECS (15 min) in the backend and already
  // scoped to watching + plan-to-watch shows, so a refresh is free as long as
  // it lands inside that window — but the Library is the app's start page, so
  // a cold cache (first load, or a launch more than one TTL after the last)
  // means this does a real AniList round-trip. Failure is non-fatal: the
  // column falls back to a dash.
  async function loadCalendar() {
    try { calendar = await getCalendar(); } catch { calendar = []; }
  }

  // The cache above expires after 15 minutes; without a periodic refresh, a
  // long-lived session (this is a desktop app left open for days) eventually
  // sees every cached episode's airing_at fall in the past and the Next
  // Episode column goes permanently blank. Refreshing every 10 minutes keeps
  // us inside the TTL so this normally just re-reads the warm cache.
  const CALENDAR_REFRESH_MS = 10 * 60_000;

  type SortKey = 'title' | 'status' | 'progress' | 'season' | 'next_airing';
  type Sort = { key: SortKey; dir: 'asc' | 'desc' };

  // Per-category sort preferences: each status tab remembers its own sort, so
  // switching tabs (e.g. Watching→progress, Plan to Watch→season) never forces
  // you to re-apply the sort. Persisted across restarts.
  const SORT_STORE_KEY = 'anivault-library-sort-by-category';
  const DEFAULT_SORT: Record<string, Sort> = {
    watching: { key: 'next_airing', dir: 'asc' },
    on_hold: { key: 'progress', dir: 'asc' },
    plan_to_watch: { key: 'season', dir: 'asc' },
  };
  const categoryKey = (filter: string | null): string => filter ?? 'all';
  const defaultSortFor = (cat: string): Sort => DEFAULT_SORT[cat] ?? { key: 'title', dir: 'asc' };

  function loadCategorySort(): Record<string, Sort> {
    try {
      const raw = localStorage.getItem(SORT_STORE_KEY);
      if (raw) return JSON.parse(raw) as Record<string, Sort>;
    } catch { /* fall through to migration */ }
    // Migrate a pre-existing single global sort into the current category so the
    // active view keeps its ordering after upgrading to per-category sorting.
    const legacyKey = loadPref('anivault-library-sortkey', '') as SortKey;
    const legacyDir = loadPref('anivault-library-sortdir', '') as 'asc' | 'desc';
    return legacyKey && legacyDir
      ? { [categoryKey(statusFilter)]: { key: legacyKey, dir: legacyDir } }
      : {};
  }

  let categorySort: Record<string, Sort> = loadCategorySort();
  const currentSort = (): Sort =>
    categorySort[categoryKey(statusFilter)] ?? defaultSortFor(categoryKey(statusFilter));

  let sortKey: SortKey = currentSort().key;

  function formatSeason(season: string | null, year: number | null): string {
    if (!season && !year) return '—';
    const s = season ? season.charAt(0) + season.slice(1).toLowerCase() : '';
    return [s, year ?? ''].filter((x) => x !== '' && x != null).join(' ');
  }
  let sortDir: 'asc' | 'desc' = currentSort().dir;
  let viewMode: 'table' | 'grid' = loadPref('anivault-library-viewmode', 'table') as 'table' | 'grid';
  let compact = loadPref('anivault-library-compact', 'false') === 'true';

  const GROUP_PREF_KEY = 'anivault-library-group-by-season';
  const COLLAPSE_KEY = 'anivault-library-season-collapsed';

  let groupBySeasonPref = loadPref(GROUP_PREF_KEY, 'true') === 'true';
  $: persistPref(GROUP_PREF_KEY, groupBySeasonPref ? 'true' : 'false');

  // Season key -> collapsed. A season absent from the map is open, so a newly
  // announced season never arrives hidden.
  function loadCollapsed(): Record<string, boolean> {
    try {
      const raw = localStorage.getItem(COLLAPSE_KEY);
      return raw ? (JSON.parse(raw) as Record<string, boolean>) : {};
    } catch { return {}; }
  }
  let collapsedSeasons: Record<string, boolean> = loadCollapsed();

  function toggleGroup(key: string) {
    collapsedSeasons = { ...collapsedSeasons, [key]: !collapsedSeasons[key] };
    try { localStorage.setItem(COLLAPSE_KEY, JSON.stringify(collapsedSeasons)); } catch { /* ignore */ }
  }

  $: persistPref('anivault-library-viewmode', viewMode);
  $: persistPref('anivault-library-compact', compact ? 'true' : 'false');
  $: persistPref('anivault-library-query', query);

  async function loadStats() {
    try { stats = await getLibraryStats(); } catch { /* non-fatal */ }
  }

  // Effective episode count for the download bar when the real count is unknown:
  // a single cour (13), stepping up by a cour once a still-airing show passes it
  // (26, 39, …). Never less than what's already watched. Used only for the
  // *visual* bar — the label still shows the real total (or "?").
  function effectiveCount(e: LibraryEntry): number {
    if (e.episode_count && e.episode_count > 0) return e.episode_count;
    const airing = e.airing_status === 'RELEASING';
    let n = 13;
    if (airing && e.watched_episodes > 13) n = Math.ceil(e.watched_episodes / 13) * 13;
    return Math.max(n, e.watched_episodes || 0);
  }
  // Label total: the real count when known, otherwise "?" (never "0").
  function totalLabel(e: LibraryEntry): string | number {
    return e.episode_count && e.episode_count > 0 ? e.episode_count : '?';
  }
  function progressPct(e: LibraryEntry): number {
    const c = effectiveCount(e);
    return c > 0 ? Math.min(100, (e.watched_episodes / c) * 100) : 0;
  }

  // Does an entry still belong under the active status tab?
  function matchesActiveFilter(e: LibraryEntry): boolean {
    if (!statusFilter) return true; // "All"
    return e.status === statusFilter;
  }
  // Re-commit `entries`, dropping any that no longer match the active tab (e.g. a
  // show that just completed while viewing "Watching"). While searching, results
  // span every category, so nothing is pruned.
  function commitEntries() {
    entries = query.trim() ? [...entries] : entries.filter(matchesActiveFilter);
  }

  // Live-update rows when the engine advances progress (auto-detected playback).
  // Depends only on `events` so it won't loop on the writes below.
  $: applyProgressEvents(events);
  function applyProgressEvents(evs: EngineEvent[]) {
    if (!evs || evs.length === 0 || entries.length === 0) return;
    let changed = false;
    for (const ev of evs) {
      if (!('ProgressAdvanced' in ev)) continue;
      const { anime_id, new_episode } = ev.ProgressAdvanced;
      const entry = entries.find((e) => e.anime_id === anime_id);
      if (entry && new_episode > entry.watched_episodes) {
        entry.watched_episodes = new_episode;
        if (entry.episode_count && new_episode >= entry.episode_count) {
          entry.status = 'completed';
        }
        changed = true;
      }
    }
    if (changed) {
      commitEntries();
      void loadStats();
    }
  }

  // Refresh stats and episode-file lists when an automatic scan changes the
  // index (new episode downloaded, file deleted, …).
  $: applyLibraryUpdated(events);
  function applyLibraryUpdated(evs: EngineEvent[]) {
    if (!evs || !evs.some((e) => 'LibraryUpdated' in e)) return;
    void loadStats();
    if (entries.length > 0) void loadEpisodeFiles(entries);
  }

  function tabCount(value: string | null): number | null {
    if (!stats) return null;
    switch (value) {
      case null: return stats.total;
      case 'watching': return stats.watching;
      case 'completed': return stats.completed;
      case 'on_hold': return stats.on_hold;
      case 'dropped': return stats.dropped;
      case 'plan_to_watch': return stats.plan_to_watch;
      default: return null;
    }
  }

  let debounceTimer: ReturnType<typeof setTimeout>;

  const statusOptions = [
    { value: null, label: 'All' },
    { value: 'watching', label: 'Watching' },
    { value: 'completed', label: 'Completed' },
    { value: 'on_hold', label: 'On Hold' },
    { value: 'dropped', label: 'Dropped' },
    { value: 'plan_to_watch', label: 'Plan to Watch' },
  ];

  function formatStatus(status: string) {
    return status.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
  }

  // The Library is local SQLite over IPC, so fetching every row for the active
  // tab costs nothing meaningful — and it is the only way the client-side sort
  // in `sortedEntries` can be correct. A page size instead truncates by
  // `ORDER BY a.id` and then sorts the survivors, which silently shows the
  // lowest-id subset re-sorted to look complete.
  const LIBRARY_FETCH_LIMIT = 10000;

  async function load() {
    loading = true;
    error = '';
    try {
      // A search spans the whole library — ignore the selected category tab so
      // matches from every status show up. With no query, the tab filters as usual.
      const searchFilter = query.trim() ? null : statusFilter;
      const results = await searchLibrary(query, searchFilter, LIBRARY_FETCH_LIMIT, 0);
      entries = results;
      if (results.length > 0) {
        loadEpisodeFiles(results);
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function loadEpisodeFiles(entries: LibraryEntry[]) {
    for (const entry of entries.slice(0, 50)) {
      try {
        const files = await getEpisodeFiles(entry.anime_id);
        if (files.length > 0) {
          episodeFilesMap.set(entry.anime_id, files);
        }
      } catch {}
    }
    episodeFilesMap = new Map(episodeFilesMap);
  }

  function debouncedReload() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      void load();
    }, 300);
  }

  function setSort(key: SortKey) {
    if (sortKey === key) {
      sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    } else {
      sortKey = key;
      sortDir = 'asc';
    }
    saveCurrentSort();
  }

  // Remember the active sort for the current category and persist the whole map.
  function saveCurrentSort() {
    categorySort = { ...categorySort, [categoryKey(statusFilter)]: { key: sortKey, dir: sortDir } };
    try { localStorage.setItem(SORT_STORE_KEY, JSON.stringify(categorySort)); } catch { /* ignore */ }
  }

  // Switch category and restore that category's own remembered sort.
  function selectStatus(value: string | null) {
    statusFilter = value;
    persistFilter(statusFilter);
    const s = currentSort();
    sortKey = s.key;
    sortDir = s.dir;
    load();
  }

  let progressUpdating = new Set<number>();

  let dragEntry: LibraryEntry | null = null;

  function handleDragStart(e: DragEvent, entry: LibraryEntry) {
    dragEntry = entry;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = 'move';
      // Required for the drop event to fire in Chromium/WebView2.
      e.dataTransfer.setData('text/plain', String(entry.anime_id));
    }
  }

  async function handleDrop(newStatus: string | null) {
    if (!dragEntry || !newStatus || dragEntry.status === newStatus) return;
    const entry = dragEntry;
    dragEntry = null;
    try {
      await updateListEntry(entry.anime_id, { status: newStatus });
      entry.status = newStatus;
      // Reflect the change: drop the row if the active filter now excludes it,
      // otherwise reassign so the badge re-renders.
      if (statusFilter && statusFilter !== newStatus) {
        entries = entries.filter((e) => e.anime_id !== entry.anime_id);
      } else {
        entries = [...entries];
      }
      void loadStats();
    } catch (e) {
      // revert on next reload
    }
  }

  async function handleIncrement(entry: LibraryEntry) {
    if (progressUpdating.has(entry.anime_id)) return;
    const newEp = entry.watched_episodes + 1;
    if (entry.episode_count && newEp > entry.episode_count) return;
    progressUpdating.add(entry.anime_id);
    try {
      await updateListEntry(entry.anime_id, { watched_episodes: newEp });
      entry.watched_episodes = newEp;
      // Auto-complete when the cap is reached (mirrors the backend), and drop it
      // from the current tab if it no longer matches (e.g. Watching → Completed).
      if (entry.episode_count && newEp >= entry.episode_count) {
        entry.status = 'completed';
      }
      commitEntries();
    } catch (e) {
      // revert on error handled by refresh
    } finally {
      progressUpdating.delete(entry.anime_id);
    }
  }

  async function handleDecrement(entry: LibraryEntry) {
    if (progressUpdating.has(entry.anime_id)) return;
    const newEp = Math.max(0, entry.watched_episodes - 1);
    progressUpdating.add(entry.anime_id);
    try {
      await updateListEntry(entry.anime_id, { watched_episodes: newEp });
      entry.watched_episodes = newEp;
      entries = [...entries];
    } catch (e) {
      // revert on error
    } finally {
      progressUpdating.delete(entry.anime_id);
    }
  }

  let selectedIds = new Set<number>();
  let allSelected = false;
  let batchUpdating = false;

  function toggleSelectAll() {
    if (allSelected) {
      selectedIds.clear();
    } else {
      sortedEntries.forEach(e => selectedIds.add(e.anime_id));
    }
    allSelected = !allSelected;
    selectedIds = new Set(selectedIds);
  }

  function toggleSelect(animeId: number) {
    if (selectedIds.has(animeId)) { selectedIds.delete(animeId); }
    else { selectedIds.add(animeId); }
    allSelected = sortedEntries.length > 0 && selectedIds.size === sortedEntries.length;
    selectedIds = new Set(selectedIds);
  }

  function batchSetStatus(status: string) {
    return async () => {
      if (batchUpdating) return;
      batchUpdating = true;
      for (const id of selectedIds) {
        try {
          await updateListEntry(id, { status });
          const entry = entries.find(e => e.anime_id === id);
          if (entry) entry.status = status;
        } catch { /* continue */ }
      }
      selectedIds.clear(); allSelected = false;
      selectedIds = new Set(selectedIds);
      commitEntries();
      void loadStats();
      batchUpdating = false;
    };
  }

  async function batchIncrementProgress() {
    if (batchUpdating) return;
    batchUpdating = true;
    for (const id of selectedIds) {
      const entry = entries.find(e => e.anime_id === id);
      if (!entry) continue;
      const newEp = entry.watched_episodes + 1;
      if (entry.episode_count && newEp > entry.episode_count) continue;
      try {
        await updateListEntry(id, { watched_episodes: newEp });
        entry.watched_episodes = newEp;
        if (entry.episode_count && newEp >= entry.episode_count) {
          entry.status = 'completed';
        }
      } catch { /* continue */ }
    }
    selectedIds.clear(); allSelected = false;
    selectedIds = new Set(selectedIds);
    commitEntries();
    batchUpdating = false;
  }

  let confirmingDelete = false;
  let confirmDeleteTimer: ReturnType<typeof setTimeout> | null = null;

  // Armed state auto-expires so a stray click minutes later can't land on a
  // still-armed delete button.
  function armBatchDelete() {
    confirmingDelete = true;
    if (confirmDeleteTimer) clearTimeout(confirmDeleteTimer);
    confirmDeleteTimer = setTimeout(() => { confirmingDelete = false; }, 4000);
  }

  function cancelBatchDelete() {
    confirmingDelete = false;
    if (confirmDeleteTimer) clearTimeout(confirmDeleteTimer);
  }

  async function confirmBatchDelete() {
    if (batchUpdating) return;
    confirmingDelete = false;
    if (confirmDeleteTimer) clearTimeout(confirmDeleteTimer);
    batchUpdating = true;
    const ids = [...selectedIds];
    for (const id of ids) {
      try {
        await deleteAnime(id);
      } catch { /* continue */ }
    }
    entries = entries.filter((e) => !selectedIds.has(e.anime_id));
    selectedIds.clear(); allSelected = false;
    selectedIds = new Set(selectedIds);
    void loadStats();
    batchUpdating = false;
  }

  function handleRowActivate(entry: LibraryEntry) {
    dispatch('select', { anime_id: entry.anime_id });
  }

  function onRowKeydown(e: KeyboardEvent, entry: LibraryEntry) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      handleRowActivate(entry);
    }
  }

  function hasNewEpisode(entry: LibraryEntry): boolean {
    const files = episodeFilesMap.get(entry.anime_id);
    if (!files) return false;
    return files.some(f => (f.episode ?? 0) > entry.watched_episodes);
  }

  function playEpisode(animeId: number, episode: number) {
    const files = episodeFilesMap.get(animeId);
    if (!files) return;
    const file = files.find(f => (f.episode ?? 0) === episode);
    if (file) openEpisodeFile(file.file_path);
  }

  // Right-click context menu.
  let ctxMenu: { x: number; y: number; entry: LibraryEntry } | null = null;

  function openContextMenu(e: MouseEvent, entry: LibraryEntry) {
    e.preventDefault();
    ctxMenu = { x: Math.min(e.clientX, window.innerWidth - 240), y: Math.min(e.clientY, window.innerHeight - 320), entry };
  }
  function closeContextMenu() { ctxMenu = null; }

  function ctxFiles(): { ep: number; path: string }[] {
    if (!ctxMenu) return [];
    return (episodeFilesMap.get(ctxMenu.entry.anime_id) ?? [])
      .map(f => ({ ep: f.episode ?? 0, path: f.file_path }))
      .filter(f => f.ep > 0)
      .sort((a, b) => a.ep - b.ep);
  }
  function playCtxPath(path: string) { openEpisodeFile(path); closeContextMenu(); }
  function ctxNextEp(): number | null {
    if (!ctxMenu) return null;
    const files = ctxFiles();
    const f = files.find(x => x.ep === ctxMenu!.entry.watched_episodes + 1)
      ?? files.find(x => x.ep > ctxMenu!.entry.watched_episodes);
    return f ? f.ep : null;
  }
  function ctxPrevEp(): number | null {
    if (!ctxMenu) return null;
    const want = ctxMenu.entry.watched_episodes;
    const f = ctxFiles().filter(x => x.ep <= want).sort((a, b) => b.ep - a.ep)[0];
    return f ? f.ep : null;
  }
  function playCtxEp(ep: number | null) {
    if (ep == null) return;
    const f = ctxFiles().find(x => x.ep === ep);
    if (f) playCtxPath(f.path);
  }
  function ctxOpenFolder() {
    const first = ctxFiles()[0];
    if (first) openContainingFolder(first.path);
    closeContextMenu();
  }
  async function ctxDelete() {
    if (!ctxMenu) return;
    const id = ctxMenu.entry.anime_id;
    closeContextMenu();
    try {
      await deleteAnime(id);
      entries = entries.filter(e => e.anime_id !== id);
      void loadStats();
    } catch { /* ignore */ }
  }
  async function ctxRescan() {
    if (!ctxMenu) return;
    const id = ctxMenu.entry.anime_id;
    closeContextMenu();
    try {
      await scanLibraryFolders();
      const files = await getEpisodeFiles(id);
      episodeFilesMap.set(id, files);
      episodeFilesMap = new Map(episodeFilesMap);
    } catch { /* ignore */ }
  }

  // Category display order for grouping cross-category search results.
  const CATEGORY_ORDER = ['watching', 'completed', 'on_hold', 'dropped', 'plan_to_watch'];
  function categoryRank(status: string): number {
    const i = CATEGORY_ORDER.indexOf(status);
    return i === -1 ? CATEGORY_ORDER.length : i;
  }

  $: sortedEntries = (() => {
    const list = [...entries];

    // While searching (results span every category), group by category, then name.
    if (query.trim()) {
      list.sort((a, b) => {
        const c = categoryRank(a.status) - categoryRank(b.status);
        return c !== 0 ? c : a.title.localeCompare(b.title);
      });
      return list;
    }

    const dir = sortDir === 'asc' ? 1 : -1;
    list.sort((a, b) => {
      let cmp = 0;
      switch (sortKey) {
        case 'title':
          cmp = a.title.localeCompare(b.title);
          break;
        case 'status':
          cmp = a.status.localeCompare(b.status);
          break;
        case 'progress': {
          const pa = a.watched_episodes ?? 0;
          const pb = b.watched_episodes ?? 0;
          cmp = pa - pb;
          break;
        }
        case 'season': {
          const sa = seasonSortVal(a);
          const sb = seasonSortVal(b);
          // Equal values (including two undated shows, both Infinity) fall
          // back to title so grouped entries get a stable, readable order
          // instead of Infinity - Infinity (NaN) or backend id order.
          cmp = sa === sb ? a.title.localeCompare(b.title) : sa - sb;
          break;
        }
        case 'next_airing': {
          const va = nextAiringSortVal(a.anime_id, nextAiring);
          const vb = nextAiringSortVal(b.anime_id, nextAiring);
          // Both missing: fall back to title so the tail has a stable order.
          if (va === vb) { cmp = a.title.localeCompare(b.title); break; }
          // Shows with nothing upcoming sort last in both directions. These
          // early returns bypass the `cmp * dir` at the end of the comparator,
          // which is exactly what pins the tail.
          if (va === Number.POSITIVE_INFINITY) return 1;
          if (vb === Number.POSITIVE_INFINITY) return -1;
          cmp = va - vb;
          break;
        }
      }
      return cmp * dir;
    });
    return list;
  })();

  // Grouping is a Plan to Watch affordance only, and a search spans every
  // category, so it switches off for the duration of a query.
  $: groupingActive = groupBySeasonPref && statusFilter === 'plan_to_watch' && !query.trim();
  $: displayRows = groupingActive
    ? flattenGroups(groupBySeason(sortedEntries, getCurrentSeason()), collapsedSeasons)
    : asDisplayRows(sortedEntries);

  // The group header carries the season, so the column is redundant there.
  $: showSeason = statusFilter === 'plan_to_watch' && !groupingActive;
  $: nextAiring = nextAiringByAnime(calendar, nowSec);
  $: showNextEpisode = statusFilter === 'watching' && !query.trim();
  // check + thumb + title + status + progress + files, plus optional columns.
  $: columnCount = 6 + (showSeason ? 1 : 0) + (showNextEpisode ? 1 : 0);

  onMount(() => {
    void load();
    void loadStats();
    void loadCalendar();
    // The column shows days and hours, so a minute is fine-grained enough.
    clockTimer = setInterval(() => { nowSec = Math.floor(Date.now() / 1000); }, 60_000);
    calendarTimer = setInterval(() => { void loadCalendar(); }, CALENDAR_REFRESH_MS);
  });

  onDestroy(() => {
    if (confirmDeleteTimer) clearTimeout(confirmDeleteTimer);
    if (clockTimer) clearInterval(clockTimer);
    if (calendarTimer) clearInterval(calendarTimer);
  });
</script>

<div class="library-view">
  <div class="lib-header">
  <div class="controls">
    <input
      type="text"
      class="search"
      placeholder="Search library…"
      bind:value={query}
      on:input={debouncedReload}
      aria-label="Search library"
    />
    <button class="view-toggle" on:click={() => viewMode = viewMode === 'table' ? 'grid' : 'table'} aria-label="Toggle view">
      {#if viewMode === 'table'}
        <LayoutGrid size={14} /> Grid
      {:else}
        <List size={14} /> Table
      {/if}
    </button>
    {#if viewMode === 'table'}
      <button class="view-toggle" on:click={() => compact = !compact} aria-pressed={compact} title="Toggle compact list density">
        {compact ? '≣ Comfortable' : '≡ Compact'}
      </button>
    {/if}
    {#if statusFilter === 'plan_to_watch'}
      <button
        class="view-toggle"
        on:click={() => groupBySeasonPref = !groupBySeasonPref}
        aria-pressed={groupBySeasonPref}
        title="Group Plan to Watch by season"
      >
        ⊞ Group by season
      </button>
    {/if}
  </div>
  <div class="status-tabs">
    {#each statusOptions as opt}
      <button
        type="button"
        class="status-tab"
        class:active={statusFilter === opt.value}
        class:dragover={dragEntry !== null && dragEntry.status !== opt.value}
        on:dragover={(e) => { e.preventDefault(); if (e.dataTransfer) e.dataTransfer.dropEffect = 'move'; }}
        on:drop={(e) => { e.preventDefault(); handleDrop(opt.value); }}
        on:click={() => selectStatus(opt.value)}
      >
        {opt.label}{#if stats}<span class="tab-count"> ({tabCount(opt.value)})</span>{/if}
      </button>
    {/each}
  </div>
  </div>

  {#if error}
    <div class="message error" role="alert">
      <p>{error}</p>
      <button type="button" class="retry" on:click={load}>Retry</button>
    </div>
  {/if}

  {#if viewMode === 'grid'}
    <div class="poster-grid">
      {#each displayRows as row (row.kind === 'group' ? `g:${row.group.key}` : `e:${row.entry.anime_id}`)}
        {#if row.kind === 'group'}
          <button
            type="button"
            class="group-band"
            class:is-marked={row.group.chip !== null}
            aria-expanded={!collapsedSeasons[row.group.key]}
            on:click={() => toggleGroup(row.group.key)}
          >
            <span class="chev" class:collapsed={collapsedSeasons[row.group.key]} aria-hidden="true">
              <ChevronDown size={13} />
            </span>
            <span class="group-name">{row.group.label}</span>
            <span class="group-count">{row.group.entries.length}</span>
            {#if row.group.chip}<span class="next-chip">{row.group.chip}</span>{/if}
          </button>
        {:else}
          {@const entry = row.entry}
          <div class="poster-card"
            tabindex="0"
            role="button"
            aria-label={`${entry.title}, ${entry.status}`}
            on:click={() => handleRowActivate(entry)}
            on:keydown={(e) => e.key === 'Enter' && handleRowActivate(entry)}
            on:contextmenu={(e) => openContextMenu(e, entry)}
          >
            <div class="poster-check">
              <input type="checkbox" checked={selectedIds.has(entry.anime_id)} on:change={() => toggleSelect(entry.anime_id)} on:click|stopPropagation aria-label={`Select ${entry.title}`} />
            </div>
            {#if entry.image_url}
              <img class="poster-thumb" src={entry.image_url} alt={entry.title} loading="lazy" />
            {:else}
              <div class="poster-thumb placeholder"></div>
            {/if}
            <div class="poster-info">
              <p class="poster-title" class:has-new={hasNewEpisode(entry)}>{entry.title}</p>
              {#if entry.status === 'unlisted'}
                <span class="no-status" aria-label="No list status">—</span>
              {:else}
                <span class="badge">{formatStatus(entry.status)}</span>
              {/if}
              <div class="progress-wrap poster-progress">
                <div class="progress-bar" style="width: {progressPct(entry)}%"></div>
                <div class="progress-inner">
                  <button class="progress-btn" on:click|stopPropagation={() => handleDecrement(entry)} aria-label="Decrease">&minus;</button>
                  <span class="progress-text">{entry.watched_episodes} / {totalLabel(entry)}</span>
                  <button class="progress-btn" on:click|stopPropagation={() => handleIncrement(entry)} aria-label="Increase">+</button>
                </div>
              </div>
              {#if showNextEpisode}
                {@const na = nextAiring.get(entry.anime_id)}
                {#if na}
                  <span class="airing-in" class:soon={(na.airing_at ?? 0) - nowSec < 86400}>
                    {formatAiringCountdown((na.airing_at ?? 0) - nowSec)}
                  </span>
                {/if}
              {/if}
              {#if episodeFilesMap.has(entry.anime_id)}
                <div class="ep-download-bar">
                  {#each Array(Math.min(effectiveCount(entry), 50)) as _, i}
                    {@const ep = i + 1}
                    {@const hasFile = episodeFilesMap.get(entry.anime_id)?.some(f => (f.episode ?? 0) === ep)}
                    {@const watched = ep <= entry.watched_episodes}
                    <button
                      type="button"
                      class="ep-segment"
                      class:downloaded={hasFile}
                      class:watched={watched}
                      disabled={!hasFile}
                      title={hasFile ? `Ep ${ep} - Downloaded` : `Ep ${ep}`}
                      aria-label={hasFile ? `Play episode ${ep}` : `Episode ${ep}`}
                      on:click|stopPropagation={() => playEpisode(entry.anime_id, ep)}
                      style="cursor: {hasFile ? 'pointer' : 'default'}"
                    ></button>
                  {/each}
                  {#if effectiveCount(entry) > 50}
                    <span class="ep-more">+{effectiveCount(entry) - 50}</span>
                  {/if}
                </div>
              {/if}
            </div>
          </div>
        {/if}
      {/each}
    </div>
  {:else}
    <div class="table-wrap">
      <table class:compact>
        <thead>
          <tr>
            <th class="col-check" scope="col">
              <input type="checkbox" on:change={toggleSelectAll} checked={allSelected} aria-label="Select all" />
            </th>
            <th class="col-thumb" scope="col">
              <span class="sr-only">Thumbnail</span>
            </th>
            <th
              class="col-title"
              scope="col"
              aria-sort={sortKey === 'title'
                ? sortDir === 'asc'
                  ? 'ascending'
                  : 'descending'
                : 'none'}
            >
              <button
                type="button"
                class="sort-btn"
                aria-label="Sort by title"
                on:click={() => setSort('title')}
              >
                Title
                {#if sortKey === 'title'}
                  <span aria-hidden="true" class="sort-arrow">
                    {#if sortDir === 'asc'}<ChevronUp size={13} />{:else}<ChevronDown size={13} />{/if}
                  </span>
                {/if}
              </button>
            </th>
            <th
              class="col-status"
              scope="col"
              aria-sort={sortKey === 'status'
                ? sortDir === 'asc'
                  ? 'ascending'
                  : 'descending'
                : 'none'}
            >
              <button
                type="button"
                class="sort-btn"
                aria-label="Sort by status"
                on:click={() => setSort('status')}
              >
                Status
                {#if sortKey === 'status'}
                  <span aria-hidden="true" class="sort-arrow">
                    {#if sortDir === 'asc'}<ChevronUp size={13} />{:else}<ChevronDown size={13} />{/if}
                  </span>
                {/if}
              </button>
            </th>
            {#if showSeason}
              <th
                class="col-season"
                scope="col"
                aria-sort={sortKey === 'season' ? (sortDir === 'asc' ? 'ascending' : 'descending') : 'none'}
              >
                <button
                  type="button"
                  class="sort-btn"
                  aria-label="Sort by season"
                  on:click={() => setSort('season')}
                >
                  Season
                  {#if sortKey === 'season'}
                    <span aria-hidden="true" class="sort-arrow">
                      {#if sortDir === 'asc'}<ChevronUp size={13} />{:else}<ChevronDown size={13} />{/if}
                    </span>
                  {/if}
                </button>
              </th>
            {/if}
            {#if showNextEpisode}
              <th
                class="col-airing"
                scope="col"
                aria-sort={sortKey === 'next_airing' ? (sortDir === 'asc' ? 'ascending' : 'descending') : 'none'}
              >
                <button
                  type="button"
                  class="sort-btn"
                  aria-label="Sort by next episode"
                  on:click={() => setSort('next_airing')}
                >
                  Next Episode
                  {#if sortKey === 'next_airing'}
                    <span aria-hidden="true" class="sort-arrow">
                      {#if sortDir === 'asc'}<ChevronUp size={13} />{:else}<ChevronDown size={13} />{/if}
                    </span>
                  {/if}
                </button>
              </th>
            {/if}
            <th
              class="col-progress"
              scope="col"
              aria-sort={sortKey === 'progress'
                ? sortDir === 'asc'
                  ? 'ascending'
                  : 'descending'
                : 'none'}
            >
              <button
                type="button"
                class="sort-btn"
                aria-label="Sort by progress"
                on:click={() => setSort('progress')}
              >
                Progress
                {#if sortKey === 'progress'}
                  <span aria-hidden="true" class="sort-arrow">
                    {#if sortDir === 'asc'}<ChevronUp size={13} />{:else}<ChevronDown size={13} />{/if}
                  </span>
                {/if}
              </button>
            </th>
            <th class="col-files" scope="col">
              <span class="sr-only">Files</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {#if loading}
            {#each Array.from({ length: 5 }) as _, i (i)}
              <tr class="skeleton-row">
                <td></td>
                <td><div class="skeleton-thumb"></div></td>
                <td><div class="skeleton-line"></div></td>
                <td><div class="skeleton-badge"></div></td>
                <td><div class="skeleton-line short"></div></td>
              </tr>
            {/each}
          {:else if sortedEntries.length === 0}
            <tr class="empty-row">
              <td colspan={columnCount}>
                <p class="empty">No anime found.</p>
              </td>
            </tr>
          {:else}
            {#each displayRows as row (row.kind === 'group' ? `g:${row.group.key}` : `e:${row.entry.anime_id}`)}
              {#if row.kind === 'group'}
                <tr class="group-row" class:is-marked={row.group.chip !== null}>
                  <td colspan={columnCount}>
                    <button
                      type="button"
                      class="group-btn"
                      aria-expanded={!collapsedSeasons[row.group.key]}
                      on:click={() => toggleGroup(row.group.key)}
                    >
                      <span class="chev" class:collapsed={collapsedSeasons[row.group.key]} aria-hidden="true">
                        <ChevronDown size={13} />
                      </span>
                      <span class="group-name">{row.group.label}</span>
                      <span class="group-count">{row.group.entries.length}</span>
                      {#if row.group.chip}<span class="next-chip">{row.group.chip}</span>{/if}
                    </button>
                  </td>
                </tr>
              {:else}
                {@const entry = row.entry}
                <tr
                  class="data-row"
                  draggable="true"
                  tabindex="0"
                  on:click={() => handleRowActivate(entry)}
                  on:keydown={(e) => onRowKeydown(e, entry)}
                  on:contextmenu={(e) => openContextMenu(e, entry)}
                  on:dragstart={(e) => handleDragStart(e, entry)}
                  on:dragend={() => dragEntry = null}
                >
                  <td class="col-check">
                    <input type="checkbox" checked={selectedIds.has(entry.anime_id)} on:change={() => toggleSelect(entry.anime_id)} on:click|stopPropagation />
                  </td>
                  <td>
                    {#if entry.image_url}
                      <img
                        class="thumb"
                        src={entry.image_url}
                        alt=""
                        width="24"
                        height="24"
                        loading="lazy"
                      />
                    {:else}
                      <div class="thumb fallback" aria-hidden="true"></div>
                    {/if}
                  </td>
                  <td class="title-cell" class:has-new={hasNewEpisode(entry)}>{entry.title}</td>
                  <td>
                    {#if entry.status === 'unlisted'}
                      <span class="no-status" aria-label="No list status">—</span>
                    {:else}
                      <span class="badge">{formatStatus(entry.status)}</span>
                    {/if}
                  </td>
                  {#if showSeason}
                    <td class="col-season season-cell">{formatSeason(entry.season, entry.season_year)}</td>
                  {/if}
                  {#if showNextEpisode}
                    {@const na = nextAiring.get(entry.anime_id)}
                    <td class="col-airing airing-cell">
                      {#if na}
                        <span class="airing-in" class:soon={(na.airing_at ?? 0) - nowSec < 86400}>
                          {formatAiringCountdown((na.airing_at ?? 0) - nowSec)}
                        </span>
                      {:else}
                        <span class="no-status">—</span>
                      {/if}
                    </td>
                  {/if}
                  <td class="num-cell progress-cell" class:completed={entry.watched_episodes > 0 && entry.episode_count != null && entry.watched_episodes >= entry.episode_count}>
                    <div class="progress-wrap">
                      <div class="progress-bar" style="width: {progressPct(entry)}%"></div>
                      <div class="progress-inner">
                        <button class="progress-btn" on:click|stopPropagation={() => handleDecrement(entry)} aria-label="Decrease">&minus;</button>
                        <span class="progress-text">{entry.watched_episodes} / {totalLabel(entry)}</span>
                        <button class="progress-btn" on:click|stopPropagation={() => handleIncrement(entry)} aria-label="Increase">+</button>
                      </div>
                    </div>
                    {#if episodeFilesMap.has(entry.anime_id)}
                      <div class="ep-download-bar">
                        {#each Array(Math.min(effectiveCount(entry), 50)) as _, i}
                          {@const ep = i + 1}
                          {@const hasFile = episodeFilesMap.get(entry.anime_id)?.some(f => (f.episode ?? 0) === ep)}
                          {@const watched = ep <= entry.watched_episodes}
                          <button
                            type="button"
                            class="ep-segment"
                            class:downloaded={hasFile}
                            class:watched={watched}
                            disabled={!hasFile}
                            title={hasFile ? `Ep ${ep} - Downloaded` : `Ep ${ep}`}
                            aria-label={hasFile ? `Play episode ${ep}` : `Episode ${ep}`}
                            on:click|stopPropagation={() => playEpisode(entry.anime_id, ep)}
                            style="cursor: {hasFile ? 'pointer' : 'default'}"
                          ></button>
                        {/each}
                        {#if effectiveCount(entry) > 50}
                          <span class="ep-more">+{effectiveCount(entry) - 50}</span>
                        {/if}
                      </div>
                    {/if}
                  </td>
                  <td class="col-files">
                    {#if episodeFilesMap.has(entry.anime_id)}
                      <button class="play-inline-btn" on:click|stopPropagation={() => {
                        const files = episodeFilesMap.get(entry.anime_id);
                        if (!files || files.length === 0) return;
                        const nextEp = files.find(f => (f.episode ?? 0) > entry.watched_episodes);
                        const target = nextEp ?? files[0];
                        if (target) openEpisodeFile(target.file_path);
                      }} title="Play next episode">&#9654;</button>
                    {/if}
                  </td>
                </tr>
              {/if}
            {/each}
          {/if}
        </tbody>
      </table>
    </div>
    {#if selectedIds.size > 0}
      <div class="batch-bar">
        <span class="batch-count">{selectedIds.size} selected</span>
        <button class="action-btn" on:click={batchSetStatus('watching')}>Watching</button>
        <button class="action-btn" on:click={batchSetStatus('completed')}>Completed</button>
        <button class="action-btn" on:click={batchSetStatus('on_hold')}>On Hold</button>
        <button class="action-btn" on:click={batchSetStatus('dropped')}>Dropped</button>
        <button class="action-btn" on:click={batchSetStatus('plan_to_watch')}>Plan to Watch</button>
        <button class="action-btn" on:click={batchIncrementProgress}>+1 Ep</button>
        {#if confirmingDelete}
          <button class="action-btn danger" on:click={confirmBatchDelete}>Confirm delete {selectedIds.size}</button>
          <button class="action-btn" on:click={cancelBatchDelete}>Cancel</button>
        {:else}
          <button class="action-btn danger" on:click={armBatchDelete}>Delete</button>
        {/if}
      </div>
    {/if}
  {/if}

  {#if ctxMenu}
    <div class="ctx-backdrop" on:click={closeContextMenu} on:contextmenu|preventDefault={closeContextMenu} role="presentation"></div>
    <div class="ctx-menu" style="left: {ctxMenu.x}px; top: {ctxMenu.y}px;" role="menu">
      <button class="ctx-item" role="menuitem" disabled={ctxNextEp() === null} on:click={() => playCtxEp(ctxNextEp())}>
        <Play size={13} /> Play next{#if ctxNextEp() !== null} <span class="ctx-dim">Ep {ctxNextEp()}</span>{/if}
      </button>
      <button class="ctx-item" role="menuitem" disabled={ctxPrevEp() === null} on:click={() => playCtxEp(ctxPrevEp())}>
        <ChevronLeft size={13} /> Play previous{#if ctxPrevEp() !== null} <span class="ctx-dim">Ep {ctxPrevEp()}</span>{/if}
      </button>

      <div class="ctx-sub">
        <button class="ctx-item has-sub" role="menuitem" disabled={ctxFiles().length === 0}>Play episode <ChevronRight size={13} class="ctx-arrow" /></button>
        {#if ctxFiles().length > 0}
          <div class="ctx-submenu">
            {#each ctxFiles() as f (f.path)}
              <button class="ctx-item" role="menuitem" on:click={() => playCtxPath(f.path)}>Episode {f.ep}</button>
            {/each}
          </div>
        {/if}
      </div>

      <div class="ctx-sep"></div>
      <button class="ctx-item" role="menuitem" disabled={ctxFiles().length === 0} on:click={ctxOpenFolder}><FolderOpen size={13} /> Open folder</button>
      <button class="ctx-item" role="menuitem" on:click={ctxRescan}><RotateCw size={13} /> Rescan episodes</button>
      <button class="ctx-item danger" role="menuitem" on:click={ctxDelete}><Trash2 size={13} /> Delete from library</button>
    </div>
  {/if}
</div>

<svelte:window on:keydown={(e) => e.key === 'Escape' && closeContextMenu()} />

<style>
  .library-view {
    display: grid;
    gap: 1rem;
  }

  .lib-header {
    position: sticky;
    top: -1.5rem;
    z-index: 6;
    display: grid;
    gap: 0.75rem;
    padding: 1.5rem 0 0.5rem;
    margin: -1.5rem 0 -0.25rem;
    background: var(--color-bg, #0a0d14);
  }

  .controls {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
    align-items: center;
  }

  .search {
    border-radius: var(--radius-card);
    color: var(--color-text);
    padding: 0.6rem 0.9rem;
    font-family: var(--font-ui);
    font-size: 0.9rem;
    outline: none;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
    min-width: 16rem;
    flex: 1 1 16rem;
  }

  .search:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 3px rgba(var(--color-accent-rgb), 0.25);
  }

  .status-tabs {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
    padding: 0.25rem 0;
  }

  .status-tab {
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
    border-radius: 999px;
    padding: 0.35rem 0.8rem;
    background: transparent;
    color: var(--color-muted);
    font-family: var(--font-ui);
    font-size: 0.82rem;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
    white-space: nowrap;
  }

  .status-tab:hover {
    background: rgba(var(--color-accent-rgb), 0.08);
    color: var(--color-text);
  }

  .status-tab.active {
    background: rgba(var(--color-accent-rgb), 0.18);
    color: var(--color-accent);
    border-color: rgba(var(--color-accent-rgb), 0.35);
  }

  .status-tab.dragover {
    background: rgba(var(--color-accent-rgb), 0.25);
    border-color: var(--color-accent);
  }

  .progress-cell {
    white-space: nowrap;
  }

  .progress-wrap {
    position: relative;
    width: 100%;
    height: 1.55rem;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.06);
    overflow: hidden;
  }

  .progress-bar {
    position: absolute;
    left: 0;
    top: 0;
    height: 100%;
    background: rgba(var(--color-accent-rgb), 0.25);
    border-radius: 4px;
    transition: width 0.3s ease;
  }

  .progress-cell.completed .progress-bar {
    background: rgba(var(--color-success-rgb), 0.25);
  }

  .progress-inner {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.35rem;
    height: 100%;
    padding: 0 0.3rem;
  }

  .progress-text {
    font-size: 0.8rem;
    font-weight: 500;
    min-width: 3rem;
    text-align: center;
  }

  .progress-btn {
    border: 1px solid rgba(var(--color-accent-rgb), 0.2);
    border-radius: 4px;
    background: rgba(var(--color-accent-rgb), 0.06);
    color: var(--color-muted);
    width: 1.4rem;
    height: 1.4rem;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    font-size: 0.85rem;
    line-height: 1;
    padding: 0;
    transition: background 0.12s, color 0.12s;
  }

  .progress-btn:hover {
    background: rgba(var(--color-accent-rgb), 0.2);
    color: var(--color-text);
  }

  .table-wrap {
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
    border-radius: var(--radius-card);
    background: rgba(255, 255, 255, 0.04);
    overflow-x: auto;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    min-width: 640px;
    font-size: 0.9rem;
  }

  thead th {
    text-align: left;
    padding: 0.75rem 1rem;
    color: var(--color-muted);
    font-weight: 600;
    font-size: 0.78rem;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    border-bottom: 1px solid rgba(var(--color-accent-rgb), 0.18);
    white-space: nowrap;
  }

  .sort-btn {
    all: unset;
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    cursor: pointer;
    color: inherit;
    font: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
    border-radius: 6px;
    padding: 0.15rem 0.3rem;
    margin: -0.15rem -0.3rem;
  }

  .sort-btn:focus-visible {
    outline: 2px solid var(--color-accent);
    outline-offset: 2px;
  }

  .sort-arrow {
    display: inline-flex;
    align-items: center;
  }

  tbody td {
    padding: 0.6rem 1rem;
    vertical-align: middle;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  /* Compact / dense list mode — fits many more rows on screen. */
  table.compact { font-size: 0.82rem; }
  table.compact thead th { padding: 0.4rem 0.7rem; }
  table.compact tbody td { padding: 0.12rem 0.7rem; }
  table.compact .thumb,
  table.compact .thumb.fallback { width: 18px; height: 18px; }
  table.compact .badge { padding: 0.04rem 0.4rem; font-size: 0.66rem; }
  table.compact .progress-wrap { height: 1.15rem; }
  table.compact .progress-text { font-size: 0.72rem; min-width: 2.4rem; }
  table.compact .progress-btn { width: 1.05rem; height: 1.05rem; font-size: 0.72rem; }
  table.compact .ep-download-bar { height: 0.25rem; margin-top: 0.1rem; }
  table.compact .ep-segment:hover { transform: scaleY(1.5); }
  table.compact .play-inline-btn { padding: 0.05rem 0.35rem; }

  .data-row {
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .data-row:hover,
  .data-row:focus {
    background: rgba(var(--color-accent-rgb), 0.08);
  }

  .data-row:focus-visible {
    outline: 2px solid var(--color-accent);
    outline-offset: -2px;
  }

  .group-row td {
    padding: 0;
    background: var(--color-surface-raised);
    border-bottom: 1px solid rgba(var(--color-accent-rgb), 0.14);
  }

  .group-btn {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    width: 100%;
    padding: 0.5rem 0.7rem;
    background: transparent;
    border: 0;
    border-left: 3px solid transparent;
    color: var(--color-text);
    font-family: var(--font-ui);
    font-size: 0.85rem;
    text-align: left;
    cursor: pointer;
  }

  .group-btn:hover { background: rgba(var(--color-accent-rgb), 0.07); }
  .group-btn:focus-visible { outline: 2px solid var(--color-accent); outline-offset: -2px; }
  .group-row.is-marked .group-btn { border-left-color: var(--color-accent); }

  .chev {
    display: inline-flex;
    color: var(--color-muted);
    transition: transform 0.16s ease;
  }
  .chev.collapsed { transform: rotate(-90deg); }

  .group-name { font-weight: 650; }
  .group-count { color: var(--color-muted); font-size: 0.78rem; font-variant-numeric: tabular-nums; }

  .next-chip {
    margin-left: auto;
    font-size: 0.62rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--color-accent);
    background: rgba(var(--color-accent-rgb), 0.14);
    border: 1px solid rgba(var(--color-accent-rgb), 0.3);
    border-radius: 999px;
    padding: 0.1rem 0.5rem;
  }

  @media (prefers-reduced-motion: reduce) {
    .chev { transition: none; }
  }

  .thumb {
    width: 24px;
    height: 24px;
    border-radius: 6px;
    object-fit: cover;
    display: block;
    background: rgba(255, 255, 255, 0.08);
  }

  .thumb.fallback {
    background: rgba(255, 255, 255, 0.12);
  }

  .title-cell {
    font-weight: 500;
    color: var(--color-text);
    max-width: 20rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
    display: inline-block;
    background: rgba(var(--color-accent-rgb), 0.12);
    color: var(--color-accent);
    border-radius: 999px;
    padding: 0.15rem 0.55rem;
    font-size: 0.75rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .no-status {
    color: var(--color-muted);
    opacity: 0.5;
  }

  .num-cell {
    font-variant-numeric: tabular-nums;
    color: var(--color-muted);
  }

  .col-season { white-space: nowrap; }
  .season-cell { color: var(--color-muted); white-space: nowrap; }

  .airing-cell { white-space: nowrap; }

  .airing-in {
    display: block;
    color: var(--color-text);
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
  }

  .airing-in.soon { color: var(--color-accent); font-weight: 650; }

  table.compact .airing-in { font-size: 0.72rem; }

  .empty-row td {
    text-align: center;
    padding: 2.5rem 1rem;
  }

  .empty {
    color: var(--color-muted);
    margin: 0;
  }

  .message {
    border: 1px solid rgba(var(--color-error-rgb), 0.35);
    border-radius: var(--radius-card);
    background: rgba(var(--color-error-rgb), 0.08);
    padding: 1rem 1.25rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .message p {
    margin: 0;
    color: var(--color-error);
  }

  .retry {
    border: 1px solid rgba(var(--color-error-rgb), 0.45);
    border-radius: 999px;
    background: rgba(var(--color-error-rgb), 0.14);
    color: var(--color-error);
    padding: 0.5rem 1rem;
    cursor: pointer;
    font-family: var(--font-ui);
    font-weight: 600;
    font-size: 0.85rem;
  }

  .retry:hover {
    background: rgba(var(--color-error-rgb), 0.22);
  }

  .skeleton-row td {
    padding: 0.7rem 1rem;
  }

  .skeleton-thumb,
  .skeleton-line,
  .skeleton-badge {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    animation: pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite;
  }

  .skeleton-thumb {
    width: 24px;
    height: 24px;
  }

  .skeleton-line {
    height: 0.9rem;
    width: 60%;
  }

  .skeleton-line.short {
    width: 30%;
  }

  .skeleton-badge {
    height: 1.1rem;
    width: 4rem;
    border-radius: 999px;
  }

  .view-toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
    border-radius: 999px;
    padding: 0.45rem 0.8rem;
    background: rgba(var(--color-accent-rgb), 0.06);
    color: var(--color-muted);
    cursor: pointer;
    font-size: 0.82rem;
    white-space: nowrap;
  }

  .view-toggle:hover {
    background: rgba(var(--color-accent-rgb), 0.15);
    color: var(--color-text);
  }

  .poster-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr));
    gap: 1rem;
  }

  .poster-card {
    position: relative;
    border: 1px solid rgba(var(--color-accent-rgb), 0.1);
    border-radius: 10px;
    overflow: hidden;
    background: rgba(255, 255, 255, 0.03);
    cursor: pointer;
    transition: border-color 0.15s, transform 0.15s;
  }

  .poster-card:hover {
    border-color: rgba(var(--color-accent-rgb), 0.3);
    transform: translateY(-2px);
  }

  .group-band {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 0.55rem;
    width: 100%;
    padding: 0.45rem 0.7rem;
    margin-top: 0.35rem;
    background: var(--color-surface-raised);
    border: 1px solid rgba(var(--color-accent-rgb), 0.14);
    border-left: 3px solid transparent;
    border-radius: 8px;
    color: var(--color-text);
    font-family: var(--font-ui);
    font-size: 0.85rem;
    text-align: left;
    cursor: pointer;
  }

  .group-band:first-child { margin-top: 0; }
  .group-band:hover { background: rgba(var(--color-accent-rgb), 0.1); }
  .group-band:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }
  .group-band.is-marked { border-left-color: var(--color-accent); }

  .poster-thumb {
    width: 100%;
    aspect-ratio: 3/4;
    object-fit: cover;
    display: block;
  }

  .poster-thumb.placeholder {
    background: rgba(var(--color-accent-rgb), 0.08);
  }

  .poster-info {
    padding: 0.6rem;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .poster-title {
    font-size: 0.85rem;
    font-weight: 600;
    line-height: 1.3;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .poster-progress {
    height: 1.5rem;
  }

  .poster-progress .progress-text {
    font-size: 0.72rem;
    min-width: 2.5rem;
  }

  .poster-progress .progress-btn {
    width: 1.25rem;
    height: 1.25rem;
    font-size: 0.8rem;
  }

  .tab-count {
    opacity: 0.65;
    font-variant-numeric: tabular-nums;
  }

  .ctx-backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
  }

  .ctx-menu {
    position: fixed;
    z-index: 41;
    min-width: 11rem;
    max-width: 16rem;
    background: rgba(16, 21, 32, 0.98);
    border: 1px solid rgba(var(--color-accent-rgb), 0.25);
    border-radius: 10px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    padding: 0.35rem;
    display: grid;
    gap: 0.15rem;
  }

  .ctx-item {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    width: 100%;
    text-align: left;
    border: none;
    background: transparent;
    color: var(--color-text);
    font-family: var(--font-ui);
    font-size: 0.82rem;
    padding: 0.4rem 0.55rem;
    border-radius: 6px;
    cursor: pointer;
    white-space: nowrap;
  }

  .ctx-item:hover:not(:disabled) {
    background: rgba(var(--color-accent-rgb), 0.15);
  }

  .ctx-item:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .ctx-item.danger { color: var(--color-danger-text); }
  .ctx-item.danger:hover:not(:disabled) { background: rgba(var(--color-danger-rgb), 0.15); }

  .ctx-dim { color: var(--color-muted); font-size: 0.75rem; }

  .ctx-sep {
    height: 1px;
    background: rgba(var(--color-accent-rgb), 0.15);
    margin: 0.25rem 0.3rem;
  }

  .ctx-sub { position: relative; }

  .ctx-submenu {
    display: none;
    position: absolute;
    left: 100%;
    top: 0;
    margin-left: 2px;
    min-width: 8rem;
    max-height: 18rem;
    overflow-y: auto;
    background: rgba(16, 21, 32, 0.99);
    border: 1px solid rgba(var(--color-accent-rgb), 0.25);
    border-radius: 10px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    padding: 0.35rem;
    gap: 0.1rem;
  }

  .ctx-sub:hover .ctx-submenu,
  .ctx-submenu:hover {
    display: grid;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.4;
    }
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border-width: 0;
  }

  .col-check {
    width: 2rem;
    text-align: center;
  }

  .col-check input {
    accent-color: var(--color-accent);
  }

  .batch-bar {
    position: sticky;
    bottom: 0;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 1rem;
    background: rgba(10,13,20,0.95);
    border: 1px solid rgba(var(--color-accent-rgb),0.2);
    border-radius: 10px;
    margin-top: 0.5rem;
    backdrop-filter: blur(8px);
    z-index: 10;
  }

  .batch-count {
    font-size: 0.82rem;
    color: var(--color-muted);
    margin-right: 0.5rem;
  }

  .action-btn {
    border: 1px solid rgba(var(--color-accent-rgb), 0.2);
    border-radius: 999px;
    padding: 0.35rem 0.7rem;
    background: rgba(var(--color-accent-rgb), 0.08);
    color: var(--color-text);
    font-family: var(--font-ui);
    font-size: 0.78rem;
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.12s;
  }

  .action-btn:hover {
    background: rgba(var(--color-accent-rgb), 0.2);
  }

  .action-btn.danger {
    border-color: rgba(var(--color-danger-rgb), 0.4);
    background: rgba(var(--color-danger-rgb), 0.12);
    color: var(--color-danger-text);
  }

  .action-btn.danger:hover {
    background: rgba(var(--color-danger-rgb), 0.24);
  }

  .col-files {
    width: 2rem;
    text-align: center;
  }

  .play-inline-btn {
    border: none;
    background: rgba(var(--color-accent-rgb),0.15);
    color: var(--color-accent);
    cursor: pointer;
    border-radius: 4px;
    padding: 0.1rem 0.4rem;
    font-size: 0.75rem;
    line-height: 1.4;
  }

  .play-inline-btn:hover {
    background: rgba(var(--color-accent-rgb),0.3);
  }

  .poster-check {
    position: absolute;
    top: 0.35rem;
    left: 0.35rem;
    z-index: 2;
  }

  .poster-check input {
    accent-color: var(--color-accent);
  }

  .ep-download-bar {
    display: flex;
    gap: 1px;
    height: 0.45rem;
    margin-top: 0.2rem;
    align-items: center;
  }

  .ep-segment {
    flex: 1;
    min-width: 2px;
    height: 100%;
    border-radius: 1px;
    background: rgba(255, 255, 255, 0.08);
    transition: background 0.15s;
    border: 0;
    padding: 0;
    font: inherit;
    appearance: none;
    -webkit-appearance: none;
  }

  .ep-segment.downloaded {
    background: rgba(var(--color-success-rgb), 0.4);
  }

  .ep-segment.watched.downloaded {
    background: rgba(var(--color-accent-rgb), 0.5);
  }

  .ep-segment.watched:not(.downloaded) {
    background: rgba(var(--color-accent-rgb), 0.25);
  }

  .ep-segment:hover {
    transform: scaleY(1.8);
  }

  .ep-more {
    font-size: 0.65rem;
    color: var(--color-muted);
    margin-left: 0.3rem;
    white-space: nowrap;
  }

  .title-cell.has-new {
    color: var(--color-accent);
    font-weight: 600;
  }

  .poster-title.has-new {
    color: var(--color-accent);
  }

  /* .lib-header's sticky offset cancels out .content's padding (App.svelte) so the
     header sits flush with the viewport edge instead of leaving a gap above it.
     .content's padding shrinks at this breakpoint, so the offset must match. */
  @media (max-width: 768px) {
    .lib-header {
      top: -1rem;
      padding: 1rem 0 0.5rem;
      margin: -1rem 0 -0.25rem;
    }
  }
</style>
