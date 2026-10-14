<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getEngineStatus, getLaunchOnStartup, getStartInTray, setStartInTray, getSetting, setLaunchOnStartup, setSetting, setTrackingEnabled, type EngineStatus, connectSonarr, disconnectSonarr, getSonarrStatus, importSonarrSeries, testSonarrConnection, type SonarrStatus, type SonarrImportReport, getLibraryFolders, setLibraryFolders, scanLibraryFolders, type LibraryScanReport, type EngineEvent } from './api';
  import {
    discoverV1Data, previewMigration, runMigration,
    backupDatabase, restoreDatabase, exportDatabaseToFile, importDatabaseFromFile,
    type V1DataPaths, type MigrationReport,
  } from './api';
  import { getVersion } from '@tauri-apps/api/app';
  import { X } from 'lucide-svelte';
  import AniListConnect from './AniListConnect.svelte';
  import SyncStatus from './SyncStatus.svelte';
  import FileManager from './FileManager.svelte';
  import SonarrRemap from './SonarrRemap.svelte';
  import { listSonarrSeries, type SonarrSeriesListRow } from './api';
  import { loadStartPage, saveStartPage, START_PAGE_OPTIONS } from './startPage';
  import { selectedWantedTags, toggleWantedTag, wantedTagRows, type WantedTagRow } from './sonarrUi';
  import { CLEANUP_FILES_KEY, CLEANUP_RESUME_KEY, CLEANUP_SONARR_KEY, CLEANUP_STATUSES_KEY, CLEANUP_STATUS_OPTIONS, DEFAULT_CLEANUP_SETTINGS, loadCleanupSettings, type CleanupSettings } from './dropCleanup';
  import { listSonarrTags } from './api';
  import ErrorNotice from './ErrorNotice.svelte';
  import { explainError, explainScanErrors, type ExplainedError } from './errors';

  // Every error slot on this page: plain words, raw details, and the retry
  // for whatever failed.
  type ActionError = ExplainedError & { retry?: () => void };
  const fail = (summary: string, e: unknown, retry?: () => void): ActionError =>
    ({ ...explainError(summary, e), retry });

  export let events: EngineEvent[] = [];

  let startPage = loadStartPage();

  // Real app version from the Tauri bundle, so About never drifts from the build.
  let appVersion = '';
  getVersion().then((v) => { appVersion = v; }).catch(() => {});

  type Tab = 'general' | 'tracking' | 'library' | 'files' | 'anilist' | 'sonarr' | 'migration' | 'about';
  let activeTab: Tab = 'general';

  let startupEnabled = false;
  let startupLoading = false;
  let startupError: ActionError | null = null;
  let startupSaveError: ActionError | null = null;
  let startupSaveState: 'idle' | 'saving' | 'saved' = 'idle';
  let startupSaveTimer: ReturnType<typeof setTimeout> | null = null;
  let startInTray = false;

  let trackingEnabled = true;
  let trackingLoading = false;
  let trackingError: ActionError | null = null;
  let trackingSaveError: ActionError | null = null;
  let trackingSaveState: 'idle' | 'saving' | 'saved' = 'idle';
  let trackingSaveTimer: ReturnType<typeof setTimeout> | null = null;

  let upNextToast = true;
  let upNextNotify = true;
  let upNextMinMinutes = 5;
  let upNextError: ActionError | null = null;

  let engineStatus: EngineStatus | null = null;
  let engineLoading = false;
  let engineError: ActionError | null = null;

  // Sonarr state
  let sonarrStatus: SonarrStatus | null = null;
  let sonarrStatusLoading = false;
  let sonarrStatusError: ActionError | null = null;

  let sonarrUrl = '';
  // Comma-separated Sonarr tag labels that filter which series are imported.
  let sonarrTagRows: WantedTagRow[] = [];
  let sonarrTagsLoading = false;
  let sonarrWantedTagsState: 'idle' | 'saved' = 'idle';
  let sonarrWantedTagsError: ActionError | null = null;
  let sonarrWantedTagsTimer: ReturnType<typeof setTimeout> | null = null;
  let sonarrApiKey = '';

  let sonarrConnecting = false;
  let sonarrConnectionError: ActionError | null = null;

  let sonarrTesting = false;
  let sonarrTestResult: 'success' | 'failure' | null = null;

  let sonarrImporting = false;
  let sonarrImportReport: SonarrImportReport | null = null;
  let sonarrImportError: ActionError | null = null;

  // Sonarr series mapping list
  let sonarrSeries: SonarrSeriesListRow[] = [];
  let sonarrSeriesLoading = false;
  let sonarrSeriesError: ActionError | null = null;
  let sonarrSeriesFilter: 'all' | 'unmapped' = 'all';
  let sonarrSeriesSearch = '';

  async function loadSonarrSeries() {
    sonarrSeriesLoading = true;
    sonarrSeriesError = null;
    try { sonarrSeries = await listSonarrSeries(); }
    catch (e) {
      sonarrSeries = [];
      sonarrSeriesError = fail("Your Sonarr series list couldn't be loaded.", e, loadSonarrSeries);
    }
    finally { sonarrSeriesLoading = false; }
  }

  // Update just the affected row in place so the list doesn't scroll back to top.
  function onSonarrRemapped(sonarrId: number, detail: { animeId: number | null; title: string | null }) {
    sonarrSeries = sonarrSeries.map((s) =>
      s.sonarr_id === sonarrId
        ? { ...s, anime_id: detail.animeId, anime_title: detail.title }
        : s,
    );
  }

  $: sonarrSeriesFiltered = sonarrSeries.filter((s) => {
    if (sonarrSeriesFilter === 'unmapped' && s.anime_id != null) return false;
    if (sonarrSeriesSearch.trim()) {
      const q = sonarrSeriesSearch.trim().toLowerCase();
      if (!`${s.title} ${s.anime_title ?? ''}`.toLowerCase().includes(q)) return false;
    }
    return true;
  });
  $: sonarrUnmappedCount = sonarrSeries.filter((s) => s.anime_id == null).length;

  // Library state
  let libraryFolders: string[] = [];
  let libraryFoldersInput = '';
  let libraryFoldersLoading = false;
  let libraryFoldersError: ActionError | null = null;
  let libraryScanning = false;
  let libraryScanReport: LibraryScanReport | null = null;
  let libraryScanError: ActionError | null = null;

  // Migration state
  let migrationDiscovering = false;
  let migrationDataPaths: V1DataPaths | null = null;
  let migrationDiscoverError: ActionError | null = null;

  let migrationPreviewing = false;
  let migrationPreviewReport: MigrationReport | null = null;
  let migrationPreviewError: ActionError | null = null;

  let migrationRunning = false;
  let migrationStrategy: 'Skip' | 'Merge' = 'Skip';
  let migrationRunReport: MigrationReport | null = null;
  let migrationRunError: ActionError | null = null;

  let migrationBackingUp = false;
  let migrationBackupPath: string | null = null;
  let migrationBackupError: ActionError | null = null;

  let migrationRestorePath: string = '';
  let migrationRestoring = false;
  let migrationRestoreResult: string | null = null;
  let migrationRestoreError: ActionError | null = null;

  let migrationExporting = false;
  let migrationExportedPath: string | null = null;
  let migrationExportError: ActionError | null = null;

  let migrationImporting = false;
  let migrationImportReport: MigrationReport | null = null;
  let migrationImportError: ActionError | null = null;

  async function loadStartup() {
    startupLoading = true;
    startupError = null;
    try {
      startupEnabled = await getLaunchOnStartup();
      startInTray = await getStartInTray();
    } catch (e) {
      startupError = fail("Your startup settings couldn't be loaded.", e, loadStartup);
    } finally {
      startupLoading = false;
    }
  }

  async function handleStartupToggle() {
    startupSaveError = null;
    const next = !startupEnabled;
    startupEnabled = next;
    startupSaveState = 'saving';
    if (startupSaveTimer) clearTimeout(startupSaveTimer);
    try {
      await setLaunchOnStartup(next);
      startupSaveState = 'saved';
      startupSaveTimer = setTimeout(() => (startupSaveState = 'idle'), 1500);
    } catch (e) {
      startupSaveState = 'idle';
      startupSaveError = fail(next ? "AniVault couldn't be set to start with Windows." : "AniVault couldn't be removed from Windows startup.", e, handleStartupToggle);
      startupEnabled = !next;
    }
  }

  async function handleStartInTrayToggle() {
    startupSaveError = null;
    const next = !startInTray;
    startInTray = next;
    startupSaveState = 'saving';
    if (startupSaveTimer) clearTimeout(startupSaveTimer);
    try {
      await setStartInTray(next);
      startupSaveState = 'saved';
      startupSaveTimer = setTimeout(() => (startupSaveState = 'idle'), 1500);
    } catch (e) {
      startupSaveState = 'idle';
      startupSaveError = fail("The start-in-tray setting couldn't be saved.", e, handleStartInTrayToggle);
      startInTray = !next;
    }
  }

  async function loadTracking() {
    trackingLoading = true;
    trackingError = null;
    try {
      const val = await getSetting<boolean>('tracking.enabled');
      trackingEnabled = val ?? true;
    } catch (e) {
      trackingError = fail("Your tracking setting couldn't be loaded.", e, loadTracking);
    } finally {
      trackingLoading = false;
    }
  }

  async function handleToggle() {
    trackingSaveError = null;
    const next = !trackingEnabled;
    trackingEnabled = next;
    trackingSaveState = 'saving';
    if (trackingSaveTimer) clearTimeout(trackingSaveTimer);
    try {
      await setTrackingEnabled(next);
      trackingSaveState = 'saved';
      trackingSaveTimer = setTimeout(() => (trackingSaveState = 'idle'), 1500);
    } catch (e) {
      trackingSaveState = 'idle';
      trackingSaveError = fail(next ? "Tracking couldn't be turned on." : "Tracking couldn't be turned off.", e, handleToggle);
      trackingEnabled = !next;
    }
  }

  async function loadUpNext() {
    upNextError = null;
    try {
      upNextToast = (await getSetting<boolean>('up_next_toast_enabled')) ?? true;
      upNextNotify = (await getSetting<boolean>('up_next_notification_enabled')) ?? true;
      upNextMinMinutes = (await getSetting<number>('up_next_min_watch_minutes')) ?? 5;
    } catch (e) {
      upNextError = fail("Your Up next settings couldn't be loaded, so the defaults are shown.", e, loadUpNext);
    }
  }

  async function toggleUpNextToast() {
    upNextToast = !upNextToast;
    try { await setSetting('up_next_toast_enabled', upNextToast); upNextError = null; }
    catch (e) {
      upNextToast = !upNextToast;
      upNextError = fail("That Up next setting couldn't be saved, so it was switched back.", e, toggleUpNextToast);
    }
  }

  async function toggleUpNextNotify() {
    upNextNotify = !upNextNotify;
    try { await setSetting('up_next_notification_enabled', upNextNotify); upNextError = null; }
    catch (e) {
      upNextNotify = !upNextNotify;
      upNextError = fail("That Up next setting couldn't be saved, so it was switched back.", e, toggleUpNextNotify);
    }
  }

  // Clamped here as well as in the input so a typed-in value can't disable the
  // gate by accident. 0 means "prompt however briefly the episode played".
  // The input is bound one-way, so Svelte only repaints it when the variable
  // changes — typing 70 over a stored 60 clamps back to 60, nothing changes, and
  // the rejected 70 would stay on screen. Write the accepted value back by hand.
  async function commitUpNextMinMinutes(input: HTMLInputElement) {
    const previous = upNextMinMinutes;
    const value = input.valueAsNumber;
    const clamped = Math.min(60, Math.max(0, Math.round(Number.isFinite(value) ? value : 5)));
    upNextMinMinutes = clamped;
    input.value = String(clamped);
    try { await setSetting('up_next_min_watch_minutes', clamped); upNextError = null; }
    catch (e) {
      upNextMinMinutes = previous;
      input.value = String(previous);
      upNextError = fail(`The minimum watch time couldn't be saved, so it's back to ${previous} minutes. Enter it again to retry.`, e);
    }
  }

  let cleanup: CleanupSettings = { ...DEFAULT_CLEANUP_SETTINGS };
  let cleanupError: ActionError | null = null;

  async function loadCleanup() {
    cleanupError = null;
    try { cleanup = await loadCleanupSettings(); }
    catch (e) { cleanupError = fail("Your clean-up settings couldn't be loaded, so the defaults are shown.", e, loadCleanup); }
  }

  const cleanupSaveFailed = (e: unknown, retry: () => void) =>
    (cleanupError = fail("That clean-up setting couldn't be saved, so it was switched back.", e, retry));

  async function toggleCleanupSonarr() {
    cleanup = { ...cleanup, offerSonarr: !cleanup.offerSonarr };
    try { await setSetting(CLEANUP_SONARR_KEY, cleanup.offerSonarr); cleanupError = null; }
    catch (e) { cleanup = { ...cleanup, offerSonarr: !cleanup.offerSonarr }; cleanupSaveFailed(e, toggleCleanupSonarr); }
  }

  async function toggleCleanupFiles() {
    cleanup = { ...cleanup, offerFiles: !cleanup.offerFiles };
    try { await setSetting(CLEANUP_FILES_KEY, cleanup.offerFiles); cleanupError = null; }
    catch (e) { cleanup = { ...cleanup, offerFiles: !cleanup.offerFiles }; cleanupSaveFailed(e, toggleCleanupFiles); }
  }

  async function toggleCleanupResume() {
    cleanup = { ...cleanup, offerResume: !cleanup.offerResume };
    try { await setSetting(CLEANUP_RESUME_KEY, cleanup.offerResume); cleanupError = null; }
    catch (e) { cleanup = { ...cleanup, offerResume: !cleanup.offerResume }; cleanupSaveFailed(e, toggleCleanupResume); }
  }

  async function toggleCleanupStatus(status: string) {
    const previous = cleanup.statuses;
    const statuses = previous.includes(status) ? previous.filter((s) => s !== status) : [...previous, status];
    cleanup = { ...cleanup, statuses };
    try { await setSetting(CLEANUP_STATUSES_KEY, statuses); cleanupError = null; }
    catch (e) { cleanup = { ...cleanup, statuses: previous }; cleanupSaveFailed(e, () => toggleCleanupStatus(status)); }
  }

  async function loadEngineStatus() {
    engineLoading = true;
    engineError = null;
    try {
      engineStatus = await getEngineStatus();
    } catch (e) {
      engineError = fail("AniVault's database status couldn't be checked.", e, loadEngineStatus);
    } finally {
      engineLoading = false;
    }
  }

  async function loadSonarrWantedTags() {
    sonarrTagsLoading = true;
    sonarrWantedTagsError = null;
    try {
      const [tags, saved] = await Promise.all([
        listSonarrTags(),
        getSetting<string[]>('sonarr.wanted_tags'),
      ]);
      sonarrTagRows = wantedTagRows(tags, saved);
    } catch (e) {
      sonarrWantedTagsError = fail("Your Sonarr tags couldn't be loaded.", e, loadSonarrWantedTags);
    } finally {
      sonarrTagsLoading = false;
    }
  }

  // Saves on every toggle. Nothing checked stores [], which imports every series.
  async function toggleSonarrTag(label: string) {
    const previous = sonarrTagRows;
    sonarrTagRows = toggleWantedTag(sonarrTagRows, label);
    sonarrWantedTagsError = null;
    try {
      await setSetting('sonarr.wanted_tags', selectedWantedTags(sonarrTagRows));
      sonarrWantedTagsState = 'saved';
      if (sonarrWantedTagsTimer) clearTimeout(sonarrWantedTagsTimer);
      sonarrWantedTagsTimer = setTimeout(() => (sonarrWantedTagsState = 'idle'), 1500);
    } catch (e) {
      sonarrTagRows = previous;
      sonarrWantedTagsError = fail("That tag choice couldn't be saved, so it was switched back.", e, () => toggleSonarrTag(label));
    }
  }

  async function loadSonarrStatus() {
    sonarrStatusLoading = true;
    sonarrStatusError = null;
    try {
      sonarrStatus = await getSonarrStatus();
      if (sonarrStatus.connected) void loadSonarrWantedTags();
    } catch (e) {
      sonarrStatusError = fail("AniVault couldn't check the Sonarr connection.", e, loadSonarrStatus);
    } finally {
      sonarrStatusLoading = false;
    }
  }

  async function handleConnectSonarr() {
    sonarrConnecting = true;
    sonarrConnectionError = null;
    try {
      await connectSonarr(sonarrUrl, sonarrApiKey);
      sonarrApiKey = '';
      await loadSonarrStatus();
    } catch (e) {
      sonarrConnectionError = fail("AniVault couldn't connect to Sonarr.", e, handleConnectSonarr);
    } finally {
      sonarrConnecting = false;
    }
  }

  async function handleDisconnectSonarr() {
    sonarrConnecting = true;
    try {
      await disconnectSonarr();
      sonarrStatus = null;
    } catch (e) {
      sonarrConnectionError = fail("Sonarr couldn't be disconnected.", e, handleDisconnectSonarr);
    } finally {
      sonarrConnecting = false;
    }
  }

  async function handleTestConnection() {
    sonarrTesting = true;
    sonarrTestResult = null;
    sonarrConnectionError = null;
    try {
      await testSonarrConnection(sonarrUrl, sonarrApiKey);
      sonarrTestResult = 'success';
    } catch (e) {
      sonarrTestResult = 'failure';
      sonarrConnectionError = fail("The connection test didn't succeed.", e, handleTestConnection);
    } finally {
      sonarrTesting = false;
    }
  }

  async function handleImportSonarr() {
    sonarrImporting = true;
    sonarrImportError = null;
    sonarrImportReport = null;
    try {
      sonarrImportReport = await importSonarrSeries();
      await loadSonarrStatus();
      await loadSonarrSeries();
    } catch (e) {
      sonarrImportError = fail("Your Sonarr series couldn't be imported.", e, handleImportSonarr);
    } finally {
      sonarrImporting = false;
    }
  }

  async function loadLibraryFolders() {
    libraryFoldersLoading = true;
    libraryFoldersError = null;
    try { libraryFolders = await getLibraryFolders(); }
    catch(e) { libraryFoldersError = fail("Your library folders couldn't be loaded.", e, loadLibraryFolders); }
    finally { libraryFoldersLoading = false; }
  }

  async function handleAddFolder() {
    if (!libraryFoldersInput.trim()) return;
    libraryFoldersError = null;
    const newFolders = [...libraryFolders, libraryFoldersInput.trim()];
    try {
      await setLibraryFolders(newFolders);
      libraryFolders = newFolders;
      libraryFoldersInput = '';
    } catch(e) {
      libraryFoldersError = fail("That folder couldn't be added.", e, handleAddFolder);
    }
  }

  async function handleRemoveFolder(index: number) {
    const newFolders = libraryFolders.filter((_, i) => i !== index);
    libraryFoldersError = null;
    try {
      await setLibraryFolders(newFolders);
      libraryFolders = newFolders;
    } catch(e) {
      libraryFoldersError = fail("That folder couldn't be removed.", e, () => handleRemoveFolder(index));
    }
  }

  async function handleScanFolders() {
    libraryScanning = true;
    libraryScanReport = null;
    libraryScanError = null;
    try { libraryScanReport = await scanLibraryFolders(); }
    catch(e) { libraryScanError = fail("The library scan didn't finish.", e, handleScanFolders); }
    finally { libraryScanning = false; }
  }

  async function handleDiscover() {
    migrationDiscovering = true; migrationDiscoverError = null;
    try { migrationDataPaths = await discoverV1Data(); }
    catch (e) { migrationDiscoverError = fail("AniVault couldn't look for data from the old version.", e, handleDiscover); }
    finally { migrationDiscovering = false; }
  }

  async function handlePreview() {
    migrationPreviewing = true; migrationPreviewError = null; migrationPreviewReport = null;
    try { migrationPreviewReport = await previewMigration(); }
    catch (e) { migrationPreviewError = fail("The import preview couldn't be made.", e, handlePreview); }
    finally { migrationPreviewing = false; }
  }

  async function handleRunMigration() {
    migrationRunning = true; migrationRunError = null; migrationRunReport = null;
    try { migrationRunReport = await runMigration(migrationStrategy); }
    catch (e) { migrationRunError = fail("The import from the old version didn't finish.", e, handleRunMigration); }
    finally { migrationRunning = false; }
  }

  async function handleBackup() {
    migrationBackingUp = true; migrationBackupError = null;
    try { migrationBackupPath = await backupDatabase(); }
    catch (e) { migrationBackupError = fail("The backup couldn't be made.", e, handleBackup); }
    finally { migrationBackingUp = false; }
  }

  async function handleRestore() {
    if (!migrationRestorePath.trim()) return;
    migrationRestoring = true; migrationRestoreError = null; migrationRestoreResult = null;
    try { migrationRestoreResult = await restoreDatabase(migrationRestorePath); }
    catch (e) { migrationRestoreError = fail("The backup couldn't be restored.", e, handleRestore); }
    finally { migrationRestoring = false; }
  }

  async function handleExport() {
    migrationExporting = true; migrationExportError = null; migrationExportedPath = null;
    try { migrationExportedPath = await exportDatabaseToFile(); }
    catch (e) { migrationExportError = fail("Your data couldn't be exported.", e, handleExport); }
    finally { migrationExporting = false; }
  }

  async function handleImport() {
    migrationImporting = true; migrationImportError = null; migrationImportReport = null;
    try { migrationImportReport = await importDatabaseFromFile(); }
    catch (e) { migrationImportError = fail("That file couldn't be imported.", e, handleImport); }
    finally { migrationImporting = false; }
  }

  onMount(() => {
    loadStartup();
    loadTracking();
    loadUpNext();
    loadCleanup();
    loadLibraryFolders();
    loadEngineStatus();
    loadSonarrStatus();
    loadSonarrSeries();
  });

  onDestroy(() => {
    if (startupSaveTimer) clearTimeout(startupSaveTimer);
    if (trackingSaveTimer) clearTimeout(trackingSaveTimer);
  });
</script>

<div class="settings-view">
  <div class="tab-bar" role="tablist" aria-label="Settings sections">
    {#each [{id: 'general', label: 'General'}, {id: 'tracking', label: 'Tracking'}, {id: 'library', label: 'Library'}, {id: 'files', label: 'Files'}, {id: 'anilist', label: 'AniList'}, {id: 'sonarr', label: 'Sonarr'}, {id: 'migration', label: 'Migration'}, {id: 'about', label: 'About'}] as tab}
      <button
        type="button"
        role="tab"
        id="tab-{tab.id}"
        class="tab"
        class:active={activeTab === tab.id}
        aria-selected={activeTab === tab.id}
        aria-controls="panel-{tab.id}"
        on:click={() => (activeTab = tab.id as Tab)}
      >
        {tab.label}
      </button>
    {/each}
  </div>

  <div class="panels">
    {#if activeTab === 'general'}
      <div class="panel" role="tabpanel" id="panel-general" aria-labelledby="tab-general">
        <section class="card">
          <div class="section-header">
            <h3>Startup</h3>
            {#if startupSaveState === 'saving'}
              <span class="save-state">Saving…</span>
            {:else if startupSaveState === 'saved'}
              <span class="save-state saved">Saved</span>
            {/if}
          </div>

          {#if startupLoading}
            <p class="muted">Loading…</p>
          {:else if startupError}
            <ErrorNotice compact {...startupError} on:dismiss={() => (startupError = null)} />
          {:else}
            {#if startupSaveError}
              <ErrorNotice compact {...startupSaveError} on:dismiss={() => (startupSaveError = null)} />
            {/if}
            <div class="toggle-row">
              <span class="label">Launch AniVault when Windows starts</span>
              <button
                type="button"
                role="switch"
                aria-checked={startupEnabled}
                aria-label="Launch AniVault when Windows starts"
                class="switch"
                on:click={handleStartupToggle}
              >
                <span class="switch-thumb"></span>
              </button>
            </div>
            <div class="toggle-row">
              <span class="label">Start minimized to the system tray</span>
              <button
                type="button"
                role="switch"
                aria-checked={startInTray}
                aria-label="Start minimized to the system tray"
                class="switch"
                on:click={handleStartInTrayToggle}
              >
                <span class="switch-thumb"></span>
              </button>
            </div>
            <p class="hint">When enabled, launching on Windows startup opens AniVault directly to the tray (no window). Open it any time from the tray icon.</p>
            <div class="toggle-row">
              <span class="label">Open to page</span>
              <select class="start-page-select" bind:value={startPage} on:change={() => saveStartPage(startPage)}>
                {#each START_PAGE_OPTIONS as opt}
                  <option value={opt.value}>{opt.label}</option>
                {/each}
              </select>
            </div>
            <p class="hint">Which page AniVault shows when it opens.</p>
          {/if}
        </section>
      </div>
    {/if}

    {#if activeTab === 'tracking'}
      <div id="panel-tracking" role="tabpanel" class="panel">
        <section class="card">
          <div class="section-header">
            <h3>Tracking</h3>
            {#if trackingSaveState === 'saving'}
              <span class="save-state">Saving…</span>
            {:else if trackingSaveState === 'saved'}
              <span class="save-state saved">Saved</span>
            {/if}
          </div>

          {#if trackingLoading}
            <p class="muted">Loading…</p>
          {:else if trackingError}
            <ErrorNotice compact {...trackingError} on:dismiss={() => (trackingError = null)} />
          {:else}
            {#if trackingSaveError}
              <ErrorNotice compact {...trackingSaveError} on:dismiss={() => (trackingSaveError = null)} />
            {/if}
            <div class="toggle-row">
              <span class="label">Enable tracking</span>
              <button
                type="button"
                role="switch"
                aria-checked={trackingEnabled}
                aria-label="Enable tracking"
                class="switch"
                on:click={handleToggle}
              >
                <span class="switch-thumb"></span>
              </button>
            </div>
            <p class="hint">Automatically detect and record playback progress.</p>
          {/if}

          <!-- These two toggles must stay reachable regardless of tracking-status
               load state (loading/error) — the Up Next prompt itself fires
               independent of that status. -->
          <div class="toggle-row">
            <span class="label">Show an in-app "Up Next" prompt when the next episode is ready</span>
            <button type="button" role="switch" aria-checked={upNextToast} aria-label={'Show an in-app "Up Next" prompt when the next episode is ready'} class="switch" on:click={toggleUpNextToast}>
              <span class="switch-thumb"></span>
            </button>
          </div>
          <div class="toggle-row">
            <span class="label">Also send a Windows notification for "Up Next"</span>
            <button type="button" role="switch" aria-checked={upNextNotify} aria-label={'Also send a Windows notification for "Up Next"'} class="switch" on:click={toggleUpNextNotify}>
              <span class="switch-thumb"></span>
            </button>
          </div>
          <div class="toggle-row">
            <label class="label" for="up-next-min-minutes">Only prompt after an episode played for at least (minutes)</label>
            <input
              id="up-next-min-minutes"
              class="form-input up-next-minutes"
              type="number"
              min="0"
              max="60"
              step="1"
              value={upNextMinMinutes}
              on:change={(e) => commitUpNextMinMinutes(e.currentTarget)}
            />
          </div>
          <p class="hint">Set 0 to prompt however briefly the episode played.</p>
          {#if upNextError}
            <ErrorNotice compact {...upNextError} on:dismiss={() => (upNextError = null)} />
          {/if}
        </section>
      </div>
    {/if}

    {#if activeTab === 'library'}
      <div class="panel" role="tabpanel" id="panel-library">
        <section class="card">
          <div class="section-header"><h3>Library Folders</h3></div>
          <p class="hint">Add folders containing your anime video files. AniVault will scan them and link episodes to your library.</p>

          {#if libraryFoldersLoading && libraryFolders.length === 0}
            <p class="muted">Loading…</p>
          {:else if libraryFoldersError && libraryFolders.length === 0}
            <ErrorNotice compact {...libraryFoldersError} on:dismiss={() => (libraryFoldersError = null)} />
          {:else}
            {#if libraryFolders.length === 0}
              <p class="muted">No folders configured. Add a folder below.</p>
            {:else}
              <ul class="folder-list">
                {#each libraryFolders as folder, i}
                  <li class="folder-item">
                    <span class="folder-path">{folder}</span>
                    <button class="btn-remove" on:click={() => handleRemoveFolder(i)} aria-label="Remove"><X size={13} /></button>
                  </li>
                {/each}
              </ul>
            {/if}

            <div class="form-actions" style="margin-top: 0.75rem;">
              <input class="form-input" type="text" bind:value={libraryFoldersInput} placeholder="C:\Users\...\Anime" on:keydown={(e) => e.key === 'Enter' && handleAddFolder()} />
              <button class="action-btn" on:click={handleAddFolder} disabled={!libraryFoldersInput.trim()}>Add</button>
            </div>

            <div class="form-actions" style="margin-top: 0.5rem;">
              <button class="action-btn" on:click={handleScanFolders} disabled={libraryScanning || libraryFolders.length === 0}>
                {libraryScanning ? 'Scanning…' : 'Scan Folders'}
              </button>
            </div>

            {#if libraryFoldersError && libraryFolders.length > 0}
              <ErrorNotice compact {...libraryFoldersError} on:dismiss={() => (libraryFoldersError = null)} />
            {/if}
            {#if libraryScanError}
              <ErrorNotice compact {...libraryScanError} on:dismiss={() => (libraryScanError = null)} />
            {/if}
            {#if libraryScanReport}
              <div class="import-report">
                <p>Found {libraryScanReport.found} video files. Indexed {libraryScanReport.indexed} new.</p>
                {#if libraryScanReport.skipped > 0}
                  <p>Skipped {libraryScanReport.skipped} already-indexed files.</p>
                {/if}
                {#if libraryScanReport.errors.length > 0}
                  {@const scanProblems = explainScanErrors(libraryScanReport.errors)}
                  <ErrorNotice compact message={scanProblems.message} items={scanProblems.items} details={scanProblems.details} retry={handleScanFolders} on:dismiss={() => libraryScanReport && (libraryScanReport = { ...libraryScanReport, errors: [] })} />
                {/if}
                {#if libraryScanReport.found === 0 && libraryScanReport.errors.length === 0}
                  <p class="muted">No video files found in configured folders. Make sure the folders contain .mkv, .mp4, .avi, or other video files in their directory tree.</p>
                {/if}
              </div>
            {/if}
          {/if}
        </section>

        <section class="card">
          <div class="section-header"><h3>Dropping and Resuming Shows</h3></div>
          <p class="hint">Moving a show to one of the statuses below asks what to clean up, and moving it back to Watching or Planning asks what to restore. Nothing changes until you choose.</p>
          <div class="toggle-row">
            <span class="label">Offer to unmonitor or remove the show in Sonarr</span>
            <button type="button" role="switch" aria-checked={cleanup.offerSonarr} aria-label="Offer to unmonitor or remove the show in Sonarr" class="switch" on:click={toggleCleanupSonarr}>
              <span class="switch-thumb"></span>
            </button>
          </div>
          <div class="toggle-row">
            <span class="label">Offer to delete the show's episode files</span>
            <button type="button" role="switch" aria-checked={cleanup.offerFiles} aria-label="Offer to delete the show's episode files" class="switch" on:click={toggleCleanupFiles}>
              <span class="switch-thumb"></span>
            </button>
          </div>
          <fieldset class="form-group">
            <legend class="form-label">Ask when a show moves to</legend>
            <ul class="tag-list">
              {#each CLEANUP_STATUS_OPTIONS as opt (opt.value)}
                <li>
                  <label class="tag-option">
                    <input type="checkbox" checked={cleanup.statuses.includes(opt.value)} on:change={() => toggleCleanupStatus(opt.value)} />
                    <span class="tag-label">{opt.label}</span>
                  </label>
                </li>
              {/each}
            </ul>
          </fieldset>
          <div class="toggle-row">
            <span class="label">When a show moves from one of these back to Watching or Planning, offer to monitor it in Sonarr again or add it back</span>
            <button type="button" role="switch" aria-checked={cleanup.offerResume} aria-label="Offer to monitor or re-add the show in Sonarr when you pick it back up" class="switch" on:click={toggleCleanupResume}>
              <span class="switch-thumb"></span>
            </button>
          </div>
          {#if cleanupError}
            <ErrorNotice compact {...cleanupError} on:dismiss={() => (cleanupError = null)} />
          {/if}
        </section>
      </div>
    {/if}

    {#if activeTab === 'files'}
      <div class="panel" role="tabpanel" id="panel-files" aria-labelledby="tab-files">
        <FileManager {events} />
      </div>
    {/if}

    {#if activeTab === 'anilist'}
      <div id="panel-anilist" role="tabpanel" class="panel">
        <div class="anilist-grid">
          <AniListConnect />
          <SyncStatus />
        </div>
      </div>
    {/if}

    {#if activeTab === 'sonarr'}
      <div class="panel" role="tabpanel" id="panel-sonarr" aria-labelledby="tab-sonarr">

        {#if sonarrStatusLoading && !sonarrStatus}
          <section class="card">
            <p class="muted">Loading…</p>
          </section>
        {:else if sonarrStatusError && !sonarrStatus}
          <section class="card">
            <ErrorNotice compact {...sonarrStatusError} on:dismiss={() => (sonarrStatusError = null)} />
          </section>
        {:else if sonarrStatus?.connected}
          <!-- Connected state -->
          <section class="card">
            <div class="section-header">
              <h3>Sonarr Connection</h3>
              <span class="connected-badge">Connected ✓</span>
            </div>

            <div class="sonarr-meta">
              <div class="sonarr-stat">
                <span class="sonarr-stat-value">{sonarrStatus.series_count}</span>
                <span class="sonarr-stat-label">series imported</span>
              </div>
              <div class="sonarr-stat">
                <span class="sonarr-stat-value">{sonarrStatus.mapped_count}</span>
                <span class="sonarr-stat-label">mapped to anime</span>
              </div>
              {#if sonarrStatus.last_sync_at}
                <div class="sonarr-stat">
                  <span class="sonarr-stat-label">Last synced</span>
                  <span class="sonarr-stat-value muted">{new Date(sonarrStatus.last_sync_at * 1000).toLocaleString()}</span>
                </div>
              {/if}
            </div>

            {#if sonarrConnectionError}
              <ErrorNotice compact {...sonarrConnectionError} on:dismiss={() => (sonarrConnectionError = null)} />
            {/if}

            <fieldset class="form-group tag-filter">
              <legend class="form-label">
                Import only series tagged
                {#if sonarrWantedTagsState === 'saved'}<span class="save-state">Saved ✓</span>{/if}
              </legend>
              {#if sonarrTagsLoading && sonarrTagRows.length === 0}
                <p class="muted">Loading Sonarr tags…</p>
              {:else if sonarrWantedTagsError && sonarrTagRows.length === 0}
                <ErrorNotice compact {...sonarrWantedTagsError} on:dismiss={() => (sonarrWantedTagsError = null)} />
              {:else if sonarrTagRows.length === 0}
                <p class="muted">This Sonarr has no tags, so every series is imported.</p>
              {:else}
                <ul class="tag-list">
                  {#each sonarrTagRows as row (row.label)}
                    <li>
                      <label class="tag-option" class:missing={row.missing}>
                        <input type="checkbox" checked={row.checked} on:change={() => toggleSonarrTag(row.label)} />
                        <span class="tag-label">{row.label}</span>
                        {#if row.missing}
                          <span class="tag-count">not found in Sonarr</span>
                        {:else}
                          <span class="tag-count">{row.seriesCount} series</span>
                        {/if}
                      </label>
                    </li>
                  {/each}
                </ul>
                {#if sonarrWantedTagsError}
                  <ErrorNotice compact {...sonarrWantedTagsError} on:dismiss={() => (sonarrWantedTagsError = null)} />
                {/if}
              {/if}
              <p class="hint">Only series carrying at least one checked tag are imported. With nothing checked, every series is imported.</p>
            </fieldset>

            <div class="sonarr-actions">
              <button
                type="button"
                class="action-btn"
                on:click={handleImportSonarr}
                disabled={sonarrImporting}
              >
                {sonarrImporting ? 'Importing…' : 'Import Series'}
              </button>
              <button
                type="button"
                class="action-btn danger"
                on:click={handleDisconnectSonarr}
                disabled={sonarrConnecting}
              >
                Disconnect
              </button>
            </div>

            {#if sonarrImportReport}
              <div class="import-report">
                <p>Imported {sonarrImportReport.imported} series: {sonarrImportReport.auto_mapped} auto-mapped, {sonarrImportReport.unmapped} need manual mapping.</p>
              </div>
            {/if}
            {#if sonarrImportError}
              <ErrorNotice compact {...sonarrImportError} on:dismiss={() => (sonarrImportError = null)} />
            {/if}
          </section>

          <section class="card">
            <div class="section-header">
              <h3>Series Mapping</h3>
              <span class="save-state">{sonarrUnmappedCount} unmapped</span>
            </div>
            <p class="hint">Map Sonarr series to anime in your library. Unmapped series won't show Sonarr info on their detail page or in the calendar.</p>

            <div class="form-actions" style="margin-bottom: 0.5rem;">
              <input class="form-input" type="text" placeholder="Search series…" bind:value={sonarrSeriesSearch} style="flex:1;" />
              <button class="action-btn outline" class:active={sonarrSeriesFilter === 'all'} on:click={() => (sonarrSeriesFilter = 'all')}>All</button>
              <button class="action-btn outline" class:active={sonarrSeriesFilter === 'unmapped'} on:click={() => (sonarrSeriesFilter = 'unmapped')}>Unmapped</button>
            </div>

            {#if sonarrSeriesLoading}
              <p class="muted">Loading…</p>
            {:else if sonarrSeriesError}
              <ErrorNotice compact {...sonarrSeriesError} on:dismiss={() => (sonarrSeriesError = null)} />
            {:else if sonarrSeries.length === 0}
              <p class="muted">No series imported yet. Click Import Series above.</p>
            {:else}
              <ul class="sonarr-series-list">
                {#each sonarrSeriesFiltered.slice(0, 300) as s (s.sonarr_id)}
                  <li class="sonarr-series-row">
                    <span class="ss-title" title={s.title}>{s.title}</span>
                    {#if s.anime_id != null}
                      <span class="ss-map mapped">→ {s.anime_title ?? `#${s.anime_id}`}</span>
                    {:else}
                      <span class="ss-map unmapped">Unmapped</span>
                    {/if}
                    <SonarrRemap sonarrId={s.sonarr_id} currentAnimeId={s.anime_id} on:changed={(e) => onSonarrRemapped(s.sonarr_id, e.detail)} />
                  </li>
                {/each}
              </ul>
              {#if sonarrSeriesFiltered.length > 300}
                <p class="muted">Showing first 300 of {sonarrSeriesFiltered.length}. Refine with search.</p>
              {/if}
            {/if}
          </section>
        {:else}
          <!-- Disconnected state -->
          <section class="card">
            <h3>Sonarr Connection</h3>

            {#if sonarrConnectionError}
              <ErrorNotice compact {...sonarrConnectionError} on:dismiss={() => (sonarrConnectionError = null)} />
            {/if}

            <div class="form-group">
              <label class="form-label" for="sonarr-url">URL</label>
              <input
                id="sonarr-url"
                class="form-input"
                type="text"
                bind:value={sonarrUrl}
                placeholder="http://localhost:8989"
                disabled={sonarrConnecting}
              />
            </div>

            <div class="form-group">
              <label class="form-label" for="sonarr-apikey">API Key</label>
              <input
                id="sonarr-apikey"
                class="form-input"
                type="password"
                bind:value={sonarrApiKey}
                placeholder="Your Sonarr API key"
                disabled={sonarrConnecting}
              />
            </div>

            <div class="form-actions">
              <button
                type="button"
                class="action-btn outline"
                on:click={handleTestConnection}
                disabled={sonarrConnecting || sonarrTesting || !sonarrUrl || !sonarrApiKey}
              >
                {sonarrTesting ? 'Testing…' : 'Test Connection'}
              </button>
              <button
                type="button"
                class="action-btn"
                on:click={handleConnectSonarr}
                disabled={sonarrConnecting || sonarrTesting || !sonarrUrl || !sonarrApiKey}
              >
                {sonarrConnecting ? 'Connecting…' : 'Connect'}
              </button>
            </div>

            {#if sonarrTestResult === 'success'}
              <p class="success-msg">Connection successful!</p>
            {/if}
          </section>
        {/if}
      </div>
    {/if}

    {#if activeTab === 'migration'}
      <div class="panel" role="tabpanel" id="panel-migration" aria-labelledby="tab-migration">

        <!-- 1. Discover Section -->
        <section class="card">
          <div class="section-header">
            <h3>Find Taiga v1 Data</h3>
          </div>
          <p class="hint">Scan common locations for old Taiga v1 data files.</p>
          <div class="form-actions">
            <button class="action-btn" on:click={handleDiscover} disabled={migrationDiscovering}>
              {migrationDiscovering ? 'Scanning…' : 'Discover v1 Data'}
            </button>
          </div>
          {#if migrationDiscoverError}
            <ErrorNotice compact {...migrationDiscoverError} on:dismiss={() => (migrationDiscoverError = null)} />
          {/if}
          {#if migrationDataPaths}
            <div class="migration-path-list">
              <p class="path-item"><span class="path-label">SQLite DB:</span> {migrationDataPaths.sqlite_path ?? 'not found'}</p>
              <p class="path-item"><span class="path-label">History XML:</span> {migrationDataPaths.history_xml_path ?? 'not found'}</p>
              <p class="path-item"><span class="path-label">Data Dir:</span> {migrationDataPaths.data_dir ?? 'unknown'}</p>
              {#if !migrationDataPaths.found}
                <p class="muted">No v1 data detected at known locations.</p>
              {/if}
            </div>
          {/if}
        </section>

        <!-- 2. Dry Run Preview -->
        <section class="card">
          <div class="section-header">
            <h3>Preview Import</h3>
          </div>
          <p class="hint">See what will be imported before applying changes.</p>
          <div class="form-actions">
            <button class="action-btn" on:click={handlePreview} disabled={migrationPreviewing || !migrationDataPaths?.found}>
              {migrationPreviewing ? 'Analyzing…' : 'Dry Run'}
            </button>
          </div>
          {#if migrationPreviewError}
            <ErrorNotice compact {...migrationPreviewError} on:dismiss={() => (migrationPreviewError = null)} />
          {/if}
          {#if migrationPreviewReport}
            <div class="import-report">
              <p><strong>Anime:</strong> {migrationPreviewReport.imported_anime} to import, {migrationPreviewReport.skipped_anime} skipped</p>
              <p><strong>Entries:</strong> {migrationPreviewReport.imported_entries} to import, {migrationPreviewReport.skipped_entries} skipped</p>
              <p><strong>History:</strong> {migrationPreviewReport.imported_history} entries</p>
              {#if migrationPreviewReport.warnings.length > 0}
                <p class="warnings-title">Warnings ({migrationPreviewReport.warnings.length})</p>
                <ul class="warning-list">
                  {#each migrationPreviewReport.warnings as w}
                    <li>[{w.source}] {w.message}</li>
                  {/each}
                </ul>
              {/if}
            </div>
          {/if}
        </section>

        <!-- 3. Run Migration -->
        <section class="card">
          <div class="section-header">
            <h3>Run Migration</h3>
          </div>
          <p class="hint">Import v1 data into AniVault. A backup is created automatically.</p>
          <div class="form-group">
            <label class="form-label" for="migration-strategy">Duplicate strategy</label>
            <select id="migration-strategy" class="form-input" bind:value={migrationStrategy}>
              <option value="Skip">Skip existing</option>
              <option value="Merge">Merge (update metadata)</option>
            </select>
          </div>
          <div class="form-actions">
            <button class="action-btn" on:click={handleRunMigration} disabled={migrationRunning || !migrationDataPaths?.found}>
              {migrationRunning ? 'Importing…' : 'Import v1 Data'}
            </button>
          </div>
          {#if migrationRunError}
            <ErrorNotice compact {...migrationRunError} on:dismiss={() => (migrationRunError = null)} />
          {/if}
          {#if migrationRunReport}
            <div class="import-report">
              <p>Imported: {migrationRunReport.imported_anime} anime, {migrationRunReport.imported_entries} entries, {migrationRunReport.imported_history} history</p>
              {#if migrationRunReport.skipped_anime > 0 || migrationRunReport.skipped_entries > 0}
                <p>Skipped: {migrationRunReport.skipped_anime} anime, {migrationRunReport.skipped_entries} entries</p>
              {/if}
            </div>
          {/if}
        </section>

        <!-- 4. Backup / Restore -->
        <section class="card">
          <div class="section-header">
            <h3>Backup & Restore</h3>
          </div>
          <p class="hint">Create a timestamped backup of your current database.</p>
          <div class="form-actions">
            <button class="action-btn outline" on:click={handleBackup} disabled={migrationBackingUp}>
              {migrationBackingUp ? 'Backing up…' : 'Backup Now'}
            </button>
          </div>
          {#if migrationBackupError}
            <ErrorNotice compact {...migrationBackupError} on:dismiss={() => (migrationBackupError = null)} />
          {/if}
          {#if migrationBackupPath}
            <p class="success-msg">Backup created: {migrationBackupPath}</p>
          {/if}

          <div class="form-group" style="margin-top: 1rem;">
            <label class="form-label" for="migration-restore">Restore from backup path</label>
            <input id="migration-restore" class="form-input" type="text" bind:value={migrationRestorePath} placeholder="Path to backup file" />
          </div>
          <div class="form-actions">
            <button class="action-btn danger" on:click={handleRestore} disabled={migrationRestoring || !migrationRestorePath.trim()}>
              {migrationRestoring ? 'Restoring…' : 'Restore (requires restart)'}
            </button>
          </div>
          {#if migrationRestoreError}
            <ErrorNotice compact {...migrationRestoreError} on:dismiss={() => (migrationRestoreError = null)} />
          {/if}
          {#if migrationRestoreResult}
            <p class="success-msg">{migrationRestoreResult}</p>
          {/if}
        </section>

        <!-- 5. Export / Import -->
        <section class="card">
          <div class="section-header">
            <h3>Export & Import</h3>
          </div>
          <p class="hint">Export your library to a JSON file, or import a file from a previous export.</p>
          <div class="form-actions">
            <button class="action-btn outline" on:click={handleExport} disabled={migrationExporting}>
              {migrationExporting ? 'Exporting…' : 'Export to file…'}
            </button>
            <button class="action-btn" on:click={handleImport} disabled={migrationImporting}>
              {migrationImporting ? 'Importing…' : 'Import from file…'}
            </button>
          </div>
          {#if migrationExportError}
            <ErrorNotice compact {...migrationExportError} on:dismiss={() => (migrationExportError = null)} />
          {/if}
          {#if migrationExportedPath}
            <p class="hint">Saved to {migrationExportedPath}</p>
          {/if}
          {#if migrationImportError}
            <ErrorNotice compact {...migrationImportError} on:dismiss={() => (migrationImportError = null)} />
          {/if}
          {#if migrationImportReport}
            <div class="import-report">
              <p>Imported: {migrationImportReport.imported_anime} anime, {migrationImportReport.imported_entries} entries</p>
            </div>
          {/if}
        </section>
      </div>
    {/if}

    {#if activeTab === 'about'}
      <div id="panel-about" role="tabpanel" class="panel">
        <section class="card">
          <h3>About</h3>

          {#if engineLoading}
            <p class="muted">Loading…</p>
          {:else if engineError}
            <ErrorNotice compact {...engineError} on:dismiss={() => (engineError = null)} />
          {:else}
            <dl class="info-list">
              <div><dt>App</dt><dd>AniVault</dd></div>
              <div><dt>Version</dt><dd>{appVersion || '—'}</dd></div>
              <div><dt>Database path</dt><dd>{engineStatus?.database_path ?? '—'}</dd></div>
              <div><dt>Migrations</dt><dd>{engineStatus?.migration_count ?? '—'}</dd></div>
            </dl>
          {/if}
        </section>
      </div>
    {/if}
  </div>
</div>

<style>
  .settings-view {
    max-width: 720px;
    margin: 0 auto;
    padding: 1.5rem;
  }

  .tab-bar {
    display: flex;
    gap: 0.25rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .tab {
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--color-muted);
    padding: 0.75rem 1rem;
    font-size: 0.85rem;
    font-family: inherit;
    cursor: pointer;
    transition: color 0.15s ease;
  }

  .tab:hover {
    color: var(--color-text);
  }

  .tab.active {
    color: var(--color-accent);
    border-bottom-color: var(--color-accent);
  }

  .tab:focus-visible {
    outline: 2px solid rgba(var(--color-accent-rgb), 0.5);
    outline-offset: 2px;
    border-radius: 4px;
  }

  .panels {
    padding-top: 1.25rem;
  }

  .card {
    border: 1px solid rgba(var(--color-accent-rgb), 0.18);
    border-radius: var(--radius-card);
    padding: 1.25rem;
    background: rgba(255, 255, 255, 0.04);
    display: grid;
    gap: 0.75rem;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  h3 {
    margin: 0;
    font-size: 1rem;
    font-weight: 700;
    color: var(--color-text);
  }

  .save-state {
    font-size: 0.78rem;
    color: var(--color-muted);
  }

  .save-state.saved {
    color: var(--color-accent);
  }

  .muted {
    color: var(--color-muted);
    font-size: 0.85rem;
    margin: 0;
  }

  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  .label {
    font-size: 0.9rem;
    color: var(--color-text);
  }

  .start-page-select {
    border: 1px solid rgba(var(--color-accent-rgb), 0.2);
    border-radius: 999px;
    padding: 0.4rem 0.8rem;
    background: rgba(255, 255, 255, 0.06);
    color: var(--color-text);
    font-size: 0.85rem;
  }
  .start-page-select option { background: #141820; }

  .switch {
    position: relative;
    width: 44px;
    height: 24px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.15);
    border: 1px solid rgba(255, 255, 255, 0.2);
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
  }

  .switch[aria-checked='true'] {
    background: rgba(var(--color-accent-rgb), 0.35);
    border-color: rgba(var(--color-accent-rgb), 0.5);
  }

  .switch:focus-visible {
    outline: 2px solid rgba(var(--color-accent-rgb), 0.5);
    outline-offset: 2px;
  }

  .switch-thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #fff;
    transition: transform 0.2s ease;
    pointer-events: none;
  }

  .switch[aria-checked='true'] .switch-thumb {
    transform: translateX(20px);
  }

  .hint {
    margin: 0;
    font-size: 0.78rem;
    color: var(--color-muted);
  }

  .anilist-grid {
    display: grid;
    gap: 1rem;
  }

  .info-list {
    display: grid;
    gap: 0.75rem;
    margin: 0;
  }

  .info-list div {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 1rem;
  }

  .info-list dt {
    color: var(--color-muted);
    font-size: 0.85rem;
  }

  .info-list dd {
    margin: 0;
    font-weight: 500;
    font-size: 0.9rem;
    word-break: break-all;
    text-align: right;
  }

  .connected-badge {
    font-size: 0.78rem;
    color: var(--color-success);
    font-weight: 600;
  }

  .sonarr-meta {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 0.75rem;
    margin-top: 0.5rem;
  }

  .sonarr-stat {
    display: grid;
    gap: 0.15rem;
    padding: 0.5rem 0.75rem;
    border: 1px solid rgba(var(--color-accent-rgb), 0.15);
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.03);
  }

  .sonarr-stat-value {
    font-size: 1.1rem;
    font-weight: 700;
    color: var(--color-text);
  }

  .sonarr-stat-value.muted {
    font-size: 0.78rem;
    font-weight: 400;
    color: var(--color-muted);
  }

  .sonarr-stat-label {
    font-size: 0.72rem;
    color: var(--color-muted);
  }

  .tag-filter {
    border: none;
    padding: 0;
    margin: 0 0 0.5rem;
  }

  .tag-list {
    list-style: none;
    margin: 0.35rem 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(12rem, 1fr));
    gap: 0.35rem 1rem;
  }

  .tag-option {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    font-size: 0.88rem;
  }

  .tag-option.missing .tag-label {
    text-decoration: line-through;
    opacity: 0.7;
  }

  .tag-count {
    color: var(--color-muted);
    font-size: 0.78rem;
    margin-left: auto;
  }

  .sonarr-actions {
    display: flex;
    gap: 0.75rem;
    margin-top: 0.75rem;
  }

  .action-btn {
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    border-radius: 999px;
    padding: 0.45rem 0.9rem;
    background: rgba(var(--color-accent-rgb), 0.12);
    color: #e9eefc;
    cursor: pointer;
    font-size: 0.85rem;
    white-space: nowrap;
  }

  .action-btn:hover:not(:disabled) {
    background: rgba(var(--color-accent-rgb), 0.22);
  }

  .action-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .action-btn.danger {
    border-color: rgba(var(--color-danger-rgb), 0.4);
    background: rgba(var(--color-danger-rgb), 0.1);
    color: var(--color-danger-text);
  }

  .action-btn.danger:hover {
    background: rgba(var(--color-danger-rgb), 0.2);
  }

  .action-btn.outline {
    background: transparent;
    border-color: rgba(var(--color-accent-rgb), 0.35);
  }

  .action-btn.outline:hover {
    background: rgba(var(--color-accent-rgb), 0.12);
  }

  .form-group {
    display: grid;
    gap: 0.4rem;
    margin-bottom: 0.75rem;
  }

  .form-label {
    font-size: 0.82rem;
    color: var(--color-muted);
  }

  .form-input {
    border: 1px solid rgba(var(--color-accent-rgb), 0.25);
    border-radius: 8px;
    padding: 0.55rem 0.7rem;
    background: rgba(255, 255, 255, 0.06);
    color: var(--color-text);
    font-size: 0.9rem;
  }

  /* Solid dark popup background for the <select> variant — avoids
     white-on-white option text against the system default. */
  .form-input option {
    background: #141820;
    color: var(--color-text);
  }

  .form-input:focus {
    outline: 2px solid rgba(var(--color-accent-rgb), 0.4);
    outline-offset: 1px;
  }

  .up-next-minutes {
    width: 5rem;
    text-align: right;
  }

  .form-actions {
    display: flex;
    gap: 0.75rem;
    margin-top: 0.5rem;
  }

  .import-report {
    margin-top: 0.75rem;
    padding: 0.6rem 0.9rem;
    border: 1px solid rgba(var(--color-accent-rgb), 0.2);
    border-radius: 10px;
    background: rgba(var(--color-accent-rgb), 0.06);
    font-size: 0.82rem;
    color: #c8d2e0;
  }

  .success-msg {
    margin-top: 0.5rem;
    color: var(--color-success);
    font-size: 0.82rem;
  }

  .migration-path-list {
    margin-top: 0.75rem;
    padding: 0.6rem 0.9rem;
    border: 1px solid rgba(var(--color-accent-rgb), 0.2);
    border-radius: 10px;
    background: rgba(var(--color-accent-rgb), 0.06);
    font-size: 0.82rem;
  }

  .path-item {
    color: #c8d2e0;
    margin-bottom: 0.25rem;
  }

  .path-label {
    color: var(--color-muted);
    width: 7rem;
    display: inline-block;
  }

  .warnings-title {
    margin-top: 0.5rem;
    color: var(--color-warning, var(--color-warning));
    font-weight: 600;
  }

  .warning-list {
    margin: 0.25rem 0 0 1.2rem;
    font-size: 0.78rem;
    color: var(--color-warning, var(--color-warning));
  }

  .warning-list li {
    margin-bottom: 0.15rem;
  }

  .action-btn.outline.active {
    background: rgba(var(--color-accent-rgb), 0.28);
    border-color: rgba(var(--color-accent-rgb), 0.6);
    color: #e9eefc;
  }

  .sonarr-series-list { list-style: none; padding: 0; margin: 0; display: grid; gap: 0.3rem; max-height: 28rem; overflow-y: auto; }
  .sonarr-series-row {
    display: grid;
    grid-template-columns: 1fr auto auto;
    gap: 0.6rem;
    align-items: center;
    padding: 0.4rem 0.6rem;
    border: 1px solid rgba(var(--color-accent-rgb),0.1);
    border-radius: 6px;
    background: rgba(255,255,255,0.03);
    font-size: 0.82rem;
  }
  .ss-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--color-text); }
  .ss-map { font-size: 0.75rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 12rem; }
  .ss-map.mapped { color: var(--color-accent); }
  .ss-map.unmapped { color: var(--color-warning); }

  .folder-list { list-style: none; padding: 0; margin: 0.5rem 0; }
  .folder-item { display: flex; align-items: center; justify-content: space-between; padding: 0.4rem 0.6rem; border: 1px solid rgba(var(--color-accent-rgb),0.1); border-radius: 6px; margin-bottom: 0.25rem; background: rgba(255,255,255,0.03); font-size: 0.82rem; }
  .folder-path { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--color-muted); }
  .btn-remove { display: inline-flex; align-items: center; border: none; background: transparent; color: var(--color-error); cursor: pointer; padding: 0 0.3rem; }
</style>
