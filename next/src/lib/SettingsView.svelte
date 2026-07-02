<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getEngineStatus, getLaunchOnStartup, getSetting, setLaunchOnStartup, setSetting, type EngineStatus, connectSonarr, disconnectSonarr, getSonarrStatus, importSonarrSeries, type SonarrStatus, type SonarrImportReport } from './api';
  import AniListConnect from './AniListConnect.svelte';
  import SyncStatus from './SyncStatus.svelte';

  type Tab = 'general' | 'tracking' | 'anilist' | 'sonarr' | 'about';
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
    try {
      await connectSonarr(sonarrUrl, sonarrApiKey);
      sonarrTestResult = 'success';
      // Clean up test connection
      await disconnectSonarr();
      await loadSonarrStatus();
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

  onMount(() => {
    loadStartup();
    loadTracking();
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
    {#each [{id: 'general', label: 'General'}, {id: 'tracking', label: 'Tracking'}, {id: 'anilist', label: 'AniList'}, {id: 'sonarr', label: 'Sonarr'}, {id: 'about', label: 'About'}] as tab}
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
</style>
