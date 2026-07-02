<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getEngineStatus, getLaunchOnStartup, getSetting, setLaunchOnStartup, setSetting, type EngineStatus, connectSonarr, disconnectSonarr, getSonarrStatus, importSonarrSeries, testSonarrConnection, type SonarrStatus, type SonarrImportReport, getLibraryFolders, setLibraryFolders, scanLibraryFolders, type LibraryScanReport } from './api';
  import {
    discoverV1Data, previewMigration, runMigration,
    backupDatabase, restoreDatabase, exportDatabase, importDatabase,
    type V1DataPaths, type MigrationReport,
  } from './api';
  import AniListConnect from './AniListConnect.svelte';
  import SyncStatus from './SyncStatus.svelte';

  type Tab = 'general' | 'tracking' | 'library' | 'anilist' | 'sonarr' | 'migration' | 'about';
  let activeTab: Tab = 'general';

  let startupEnabled = false;
  let startupLoading = false;
  let startupError: string | null = null;
  let startupSaveState: 'idle' | 'saving' | 'saved' = 'idle';
  let startupSaveTimer: ReturnType<typeof setTimeout> | null = null;

  let trackingEnabled = true;
  let trackingLoading = false;
  let trackingError: string | null = null;
  let trackingSaveState: 'idle' | 'saving' | 'saved' = 'idle';
  let trackingSaveTimer: ReturnType<typeof setTimeout> | null = null;

  let engineStatus: EngineStatus | null = null;
  let engineLoading = false;
  let engineError: string | null = null;

  // Sonarr state
  let sonarrStatus: SonarrStatus | null = null;
  let sonarrStatusLoading = false;
  let sonarrStatusError: string | null = null;

  let sonarrUrl = '';
  let sonarrApiKey = '';

  let sonarrConnecting = false;
  let sonarrConnectionError: string | null = null;

  let sonarrTesting = false;
  let sonarrTestResult: 'success' | 'failure' | null = null;

  let sonarrImporting = false;
  let sonarrImportReport: SonarrImportReport | null = null;
  let sonarrImportError: string | null = null;

  // Library state
  let libraryFolders: string[] = [];
  let libraryFoldersInput = '';
  let libraryFoldersLoading = false;
  let libraryFoldersError: string | null = null;
  let libraryScanning = false;
  let libraryScanReport: LibraryScanReport | null = null;

  // Migration state
  let migrationDiscovering = false;
  let migrationDataPaths: V1DataPaths | null = null;
  let migrationDiscoverError: string | null = null;

  let migrationPreviewing = false;
  let migrationPreviewReport: MigrationReport | null = null;
  let migrationPreviewError: string | null = null;

  let migrationRunning = false;
  let migrationStrategy: string = 'Skip';
  let migrationRunReport: MigrationReport | null = null;
  let migrationRunError: string | null = null;

  let migrationBackingUp = false;
  let migrationBackupPath: string | null = null;
  let migrationBackupError: string | null = null;

  let migrationRestorePath: string = '';
  let migrationRestoring = false;
  let migrationRestoreResult: string | null = null;
  let migrationRestoreError: string | null = null;

  let migrationExporting = false;
  let migrationExportedJson: string | null = null;
  let migrationExportError: string | null = null;

  let migrationImportJson: string = '';
  let migrationImporting = false;
  let migrationImportReport: MigrationReport | null = null;
  let migrationImportError: string | null = null;

  async function loadStartup() {
    startupLoading = true;
    startupError = null;
    try {
      startupEnabled = await getLaunchOnStartup();
    } catch (e) {
      startupError = e instanceof Error ? e.message : String(e);
    } finally {
      startupLoading = false;
    }
  }

  async function handleStartupToggle() {
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
      startupError = e instanceof Error ? e.message : String(e);
      startupEnabled = !next;
    }
  }

  async function loadTracking() {
    trackingLoading = true;
    trackingError = null;
    try {
      const val = await getSetting<boolean>('tracking.enabled');
      trackingEnabled = val ?? true;
    } catch (e) {
      trackingError = e instanceof Error ? e.message : String(e);
    } finally {
      trackingLoading = false;
    }
  }

  async function handleToggle() {
    const next = !trackingEnabled;
    trackingEnabled = next;
    trackingSaveState = 'saving';
    if (trackingSaveTimer) clearTimeout(trackingSaveTimer);
    try {
      await setSetting('tracking.enabled', next);
      trackingSaveState = 'saved';
      trackingSaveTimer = setTimeout(() => (trackingSaveState = 'idle'), 1500);
    } catch (e) {
      trackingSaveState = 'idle';
      trackingError = e instanceof Error ? e.message : String(e);
      trackingEnabled = !next;
    }
  }

  async function loadEngineStatus() {
    engineLoading = true;
    engineError = null;
    try {
      engineStatus = await getEngineStatus();
    } catch (e) {
      engineError = e instanceof Error ? e.message : String(e);
    } finally {
      engineLoading = false;
    }
  }

  async function loadSonarrStatus() {
    sonarrStatusLoading = true;
    sonarrStatusError = null;
    try {
      sonarrStatus = await getSonarrStatus();
    } catch (e) {
      sonarrStatusError = e instanceof Error ? e.message : String(e);
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
      sonarrConnectionError = e instanceof Error ? e.message : String(e);
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
      sonarrConnectionError = e instanceof Error ? e.message : String(e);
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
      sonarrConnectionError = e instanceof Error ? e.message : String(e);
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
    } catch (e) {
      sonarrImportError = e instanceof Error ? e.message : String(e);
    } finally {
      sonarrImporting = false;
    }
  }

  async function loadLibraryFolders() {
    libraryFoldersLoading = true;
    libraryFoldersError = null;
    try { libraryFolders = await getLibraryFolders(); }
    catch(e) { libraryFoldersError = e instanceof Error ? e.message : String(e); }
    finally { libraryFoldersLoading = false; }
  }

  async function handleAddFolder() {
    if (!libraryFoldersInput.trim()) return;
    const newFolders = [...libraryFolders, libraryFoldersInput.trim()];
    try {
      await setLibraryFolders(newFolders);
      libraryFolders = newFolders;
      libraryFoldersInput = '';
    } catch(e) {
      libraryFoldersError = e instanceof Error ? e.message : String(e);
    }
  }

  async function handleRemoveFolder(index: number) {
    const newFolders = libraryFolders.filter((_, i) => i !== index);
    try {
      await setLibraryFolders(newFolders);
      libraryFolders = newFolders;
    } catch(e) {
      libraryFoldersError = e instanceof Error ? e.message : String(e);
    }
  }

  async function handleScanFolders() {
    libraryScanning = true;
    libraryScanReport = null;
    try { libraryScanReport = await scanLibraryFolders(); }
    catch(e) { libraryFoldersError = e instanceof Error ? e.message : String(e); }
    finally { libraryScanning = false; }
  }

  async function handleDiscover() {
    migrationDiscovering = true; migrationDiscoverError = null;
    try { migrationDataPaths = await discoverV1Data(); }
    catch (e) { migrationDiscoverError = e instanceof Error ? e.message : String(e); }
    finally { migrationDiscovering = false; }
  }

  async function handlePreview() {
    migrationPreviewing = true; migrationPreviewError = null; migrationPreviewReport = null;
    try { migrationPreviewReport = await previewMigration(); }
    catch (e) { migrationPreviewError = e instanceof Error ? e.message : String(e); }
    finally { migrationPreviewing = false; }
  }

  async function handleRunMigration() {
    migrationRunning = true; migrationRunError = null; migrationRunReport = null;
    try { migrationRunReport = await runMigration(migrationStrategy); }
    catch (e) { migrationRunError = e instanceof Error ? e.message : String(e); }
    finally { migrationRunning = false; }
  }

  async function handleBackup() {
    migrationBackingUp = true; migrationBackupError = null;
    try { migrationBackupPath = await backupDatabase(); }
    catch (e) { migrationBackupError = e instanceof Error ? e.message : String(e); }
    finally { migrationBackingUp = false; }
  }

  async function handleRestore() {
    if (!migrationRestorePath.trim()) return;
    migrationRestoring = true; migrationRestoreError = null; migrationRestoreResult = null;
    try { migrationRestoreResult = await restoreDatabase(migrationRestorePath); }
    catch (e) { migrationRestoreError = e instanceof Error ? e.message : String(e); }
    finally { migrationRestoring = false; }
  }

  async function handleExport() {
    migrationExporting = true; migrationExportError = null;
    try { migrationExportedJson = await exportDatabase(); }
    catch (e) { migrationExportError = e instanceof Error ? e.message : String(e); }
    finally { migrationExporting = false; }
  }

  async function handleImport() {
    if (!migrationImportJson.trim()) return;
    migrationImporting = true; migrationImportError = null; migrationImportReport = null;
    try { migrationImportReport = await importDatabase(migrationImportJson); }
    catch (e) { migrationImportError = e instanceof Error ? e.message : String(e); }
    finally { migrationImporting = false; }
  }

  onMount(() => {
    loadStartup();
    loadTracking();
    loadLibraryFolders();
    loadEngineStatus();
    loadSonarrStatus();
  });

  onDestroy(() => {
    if (startupSaveTimer) clearTimeout(startupSaveTimer);
    if (trackingSaveTimer) clearTimeout(trackingSaveTimer);
  });
</script>

<div class="settings-view">
  <nav class="tab-bar" role="tablist" aria-label="Settings sections">
    {#each [{id: 'general', label: 'General'}, {id: 'tracking', label: 'Tracking'}, {id: 'library', label: 'Library'}, {id: 'anilist', label: 'AniList'}, {id: 'sonarr', label: 'Sonarr'}, {id: 'migration', label: 'Migration'}, {id: 'about', label: 'About'}] as tab}
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
  </nav>

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
            <div class="error-row">
              <p class="error">{startupError}</p>
              <button type="button" class="btn-retry" on:click={loadStartup}>Retry</button>
            </div>
          {:else}
            <div class="toggle-row">
              <span class="label">Launch AniVault when Windows starts</span>
              <button
                type="button"
                class="toggle-btn"
                class:active={startupEnabled}
                aria-pressed={startupEnabled}
                on:click={handleStartupToggle}
              >
                {startupEnabled ? 'Enabled' : 'Disabled'}
              </button>
            </div>
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
            <div class="error-row">
              <p class="error">{trackingError}</p>
              <button type="button" class="btn-retry" on:click={loadTracking}>Retry</button>
            </div>
          {:else}
            <div class="toggle-row">
              <span class="label">Enable tracking</span>
              <button
                type="button"
                role="switch"
                aria-checked={trackingEnabled}
                class="switch"
                on:click={handleToggle}
              >
                <span class="switch-thumb" />
              </button>
            </div>
            <p class="hint">Automatically detect and record playback progress.</p>
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
            <div class="error-row"><p class="error">{libraryFoldersError}</p><button class="btn-retry" on:click={loadLibraryFolders}>Retry</button></div>
          {:else}
            {#if libraryFolders.length === 0}
              <p class="muted">No folders configured. Add a folder below.</p>
            {:else}
              <ul class="folder-list">
                {#each libraryFolders as folder, i}
                  <li class="folder-item">
                    <span class="folder-path">{folder}</span>
                    <button class="btn-remove" on:click={() => handleRemoveFolder(i)} aria-label="Remove">✕</button>
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

            {#if libraryScanReport}
              <div class="import-report">
                <p>Found {libraryScanReport.found} video files, indexed {libraryScanReport.indexed} new entries.</p>
              </div>
            {/if}
          {/if}
        </section>
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
            <div class="error-row">
              <p class="error">{sonarrStatusError}</p>
              <button type="button" class="btn-retry" on:click={loadSonarrStatus}>Retry</button>
            </div>
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
              <p class="error">{sonarrConnectionError}</p>
            {/if}

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
              <p class="error">{sonarrImportError}</p>
            {/if}
          </section>
        {:else}
          <!-- Disconnected state -->
          <section class="card">
            <h3>Sonarr Connection</h3>

            {#if sonarrConnectionError}
              <p class="error">{sonarrConnectionError}</p>
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
            {:else if sonarrTestResult === 'failure'}
              <p class="error">Connection failed. Check URL and API key.</p>
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
            <p class="error">{migrationDiscoverError}</p>
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
            <p class="error">{migrationPreviewError}</p>
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
            <label class="form-label">Duplicate strategy</label>
            <select class="form-input" bind:value={migrationStrategy}>
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
            <p class="error">{migrationRunError}</p>
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
            <p class="error">{migrationBackupError}</p>
          {/if}
          {#if migrationBackupPath}
            <p class="success-msg">Backup created: {migrationBackupPath}</p>
          {/if}

          <div class="form-group" style="margin-top: 1rem;">
            <label class="form-label">Restore from backup path</label>
            <input class="form-input" type="text" bind:value={migrationRestorePath} placeholder="Path to backup file" />
          </div>
          <div class="form-actions">
            <button class="action-btn danger" on:click={handleRestore} disabled={migrationRestoring || !migrationRestorePath.trim()}>
              {migrationRestoring ? 'Restoring…' : 'Restore (requires restart)'}
            </button>
          </div>
          {#if migrationRestoreError}
            <p class="error">{migrationRestoreError}</p>
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
          <p class="hint">Export your library as JSON, or import from a previous export.</p>
          <div class="form-actions">
            <button class="action-btn outline" on:click={handleExport} disabled={migrationExporting}>
              {migrationExporting ? 'Exporting…' : 'Export as JSON'}
            </button>
          </div>
          {#if migrationExportError}
            <p class="error">{migrationExportError}</p>
          {/if}
          {#if migrationExportedJson}
            <details class="export-details">
              <summary>Exported JSON ({migrationExportedJson.length} chars)</summary>
              <pre class="export-pre">{migrationExportedJson.slice(0, 2000)}{migrationExportedJson.length > 2000 ? '...' : ''}</pre>
            </details>
          {/if}

          <div class="form-group" style="margin-top: 1rem;">
            <label class="form-label">Import from JSON</label>
            <textarea class="form-input" bind:value={migrationImportJson} placeholder="Paste exported JSON here" rows={3}></textarea>
          </div>
          <div class="form-actions">
            <button class="action-btn" on:click={handleImport} disabled={migrationImporting || !migrationImportJson.trim()}>
              {migrationImporting ? 'Importing…' : 'Import JSON'}
            </button>
          </div>
          {#if migrationImportError}
            <p class="error">{migrationImportError}</p>
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
            <div class="error-row">
              <p class="error">{engineError}</p>
              <button type="button" class="btn-retry" on:click={loadEngineStatus}>Retry</button>
            </div>
          {:else}
            <dl class="info-list">
              <div><dt>App</dt><dd>Taiga</dd></div>
              <div><dt>Version</dt><dd>0.1.0</dd></div>
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
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
    border-radius: 4px;
  }

  .panels {
    padding-top: 1.25rem;
  }

  .card {
    border: 1px solid rgba(143, 183, 255, 0.18);
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

  .error {
    color: var(--color-error);
    font-size: 0.85rem;
    margin: 0;
  }

  .error-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .btn-retry {
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 999px;
    padding: 0.45rem 0.9rem;
    font-size: 0.78rem;
    cursor: pointer;
    background: rgba(143, 183, 255, 0.18);
    color: #e9eefc;
    font-family: inherit;
  }

  .btn-retry:hover {
    background: rgba(143, 183, 255, 0.28);
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
    background: rgba(143, 183, 255, 0.35);
    border-color: rgba(143, 183, 255, 0.5);
  }

  .switch:focus-visible {
    outline: 2px solid rgba(143, 183, 255, 0.5);
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

  .toggle-btn {
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 999px;
    padding: 0.45rem 0.9rem;
    font-size: 0.78rem;
    cursor: pointer;
    background: rgba(143, 183, 255, 0.18);
    color: #e9eefc;
    font-family: inherit;
    transition: background 0.15s ease, border-color 0.15s ease;
  }

  .toggle-btn:hover {
    background: rgba(143, 183, 255, 0.28);
  }

  .toggle-btn.active {
    background: rgba(143, 183, 255, 0.45);
    border-color: rgba(143, 183, 255, 0.6);
  }

  .toggle-btn:focus-visible {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
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
    color: #7ee87e;
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
    border: 1px solid rgba(143, 183, 255, 0.15);
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

  .sonarr-actions {
    display: flex;
    gap: 0.75rem;
    margin-top: 0.75rem;
  }

  .action-btn {
    border: 1px solid rgba(143, 183, 255, 0.35);
    border-radius: 999px;
    padding: 0.45rem 0.9rem;
    background: rgba(143, 183, 255, 0.12);
    color: #e9eefc;
    cursor: pointer;
    font-size: 0.85rem;
    white-space: nowrap;
  }

  .action-btn:hover:not(:disabled) {
    background: rgba(143, 183, 255, 0.22);
  }

  .action-btn:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .action-btn.danger {
    border-color: rgba(255, 130, 130, 0.4);
    background: rgba(255, 130, 130, 0.1);
    color: #ffb0b0;
  }

  .action-btn.danger:hover {
    background: rgba(255, 130, 130, 0.2);
  }

  .action-btn.outline {
    background: transparent;
    border-color: rgba(143, 183, 255, 0.35);
  }

  .action-btn.outline:hover {
    background: rgba(143, 183, 255, 0.12);
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
    border: 1px solid rgba(143, 183, 255, 0.25);
    border-radius: 8px;
    padding: 0.55rem 0.7rem;
    background: rgba(255, 255, 255, 0.06);
    color: var(--color-text);
    font-size: 0.9rem;
  }

  .form-input:focus {
    outline: 2px solid rgba(143, 183, 255, 0.4);
    outline-offset: 1px;
  }

  .form-actions {
    display: flex;
    gap: 0.75rem;
    margin-top: 0.5rem;
  }

  .import-report {
    margin-top: 0.75rem;
    padding: 0.6rem 0.9rem;
    border: 1px solid rgba(143, 183, 255, 0.2);
    border-radius: 10px;
    background: rgba(143, 183, 255, 0.06);
    font-size: 0.82rem;
    color: #c8d2e0;
  }

  .success-msg {
    margin-top: 0.5rem;
    color: #7ee87e;
    font-size: 0.82rem;
  }

  .migration-path-list {
    margin-top: 0.75rem;
    padding: 0.6rem 0.9rem;
    border: 1px solid rgba(143, 183, 255, 0.2);
    border-radius: 10px;
    background: rgba(143, 183, 255, 0.06);
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
    color: var(--color-warning, #f0c040);
    font-weight: 600;
  }

  .warning-list {
    margin: 0.25rem 0 0 1.2rem;
    font-size: 0.78rem;
    color: var(--color-warning, #f0c040);
  }

  .warning-list li {
    margin-bottom: 0.15rem;
  }

  .export-details {
    margin-top: 0.75rem;
    font-size: 0.82rem;
    color: var(--color-muted);
  }

  .export-details summary {
    cursor: pointer;
  }

  .export-pre {
    margin-top: 0.5rem;
    padding: 0.6rem;
    border: 1px solid rgba(143, 183, 255, 0.2);
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.3);
    font-family: monospace;
    font-size: 0.72rem;
    color: var(--color-muted);
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 16rem;
    overflow-y: auto;
  }

  textarea.form-input {
    resize: vertical;
    min-height: 3.5rem;
    font-family: monospace;
    font-size: 0.78rem;
  }

  .folder-list { list-style: none; padding: 0; margin: 0.5rem 0; }
  .folder-item { display: flex; align-items: center; justify-content: space-between; padding: 0.4rem 0.6rem; border: 1px solid rgba(143,183,255,0.1); border-radius: 6px; margin-bottom: 0.25rem; background: rgba(255,255,255,0.03); font-size: 0.82rem; }
  .folder-path { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--color-muted); }
  .btn-remove { border: none; background: transparent; color: #ff9d9d; cursor: pointer; font-size: 0.85rem; padding: 0 0.3rem; }
</style>
