<script lang="ts">
  import { storeAniListToken, disconnectAniList, importAniListLibrary, connectAniListOauth } from './api';

  // OAuth state
  let clientId = '';
  let clientSecret = '';
  let oauthConnecting = false;
  let oauthError: string | null = null;

  // Manual token state
  let manualToken = '';
  let connected = false;
  let loading = false;
  let error: string | null = null;
  let importReport: { imported: number; merged: number; skipped: number } | null = null;

  async function handleOAuthConnect() {
    if (!clientId.trim() || !clientSecret.trim()) return;
    oauthConnecting = true;
    oauthError = null;
    try {
      await connectAniListOauth(clientId.trim(), clientSecret.trim());
      // Clear fields on success
      clientId = '';
      clientSecret = '';
      connected = true;
    } catch (e) {
      oauthError = e instanceof Error ? e.message : String(e);
    } finally {
      oauthConnecting = false;
    }
  }

  async function handleConnect() {
    if (!manualToken.trim()) return;
    error = null;
    try {
      await storeAniListToken(manualToken.trim());
      connected = true;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      connected = false;
    }
  }

  async function handleDisconnect() {
    error = null;
    try {
      await disconnectAniList();
      connected = false;
      manualToken = '';
      importReport = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function handleImport() {
    loading = true;
    error = null;
    importReport = null;
    try {
      importReport = await importAniListLibrary();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }
</script>

<section class="anilist-card" aria-label="AniList connection">
  <p class="eyebrow">AniList</p>

  {#if error}
    <p class="error" aria-live="polite">{error}</p>
  {/if}

  {#if !connected}
    {#if oauthError}
      <p class="error" role="alert">{oauthError}</p>
    {/if}

    <div class="oauth-section">
      <h4 class="oauth-heading">Connect with AniList</h4>
      <p class="help-text">
        Enter your app's Client ID and Client Secret from <a href="https://anilist.co/settings/developer" target="_blank" rel="noopener">AniList Developer Settings</a>.
      </p>
      <p class="help-text redirect-note">
        <strong>Required:</strong> In your AniList app settings, set <strong>Redirect URI</strong> to exactly: <code>http://127.0.0.1:35789</code>
      </p>

      <div class="form-group">
        <label class="input-label" for="client-id">Client ID</label>
        <input id="client-id" class="form-input" type="text" bind:value={clientId} placeholder="e.g. 12345" />
      </div>
      <div class="form-group">
        <label class="input-label" for="client-secret">Client Secret</label>
        <input id="client-secret" class="form-input" type="password" bind:value={clientSecret} placeholder="Secret from AniList" />
      </div>

      <button class="action-btn" on:click={handleOAuthConnect} disabled={oauthConnecting || !clientId.trim() || !clientSecret.trim()}>
        {oauthConnecting ? 'Opening browser…' : 'Connect with AniList'}
      </button>
    </div>

    <details class="manual-section">
      <summary><span class="manual-summary">Manual token (alternative)</span></summary>
      <div class="connect-row">
        <label for="anilist-token" class="input-label">AniList Access Token</label>
        <input
          id="anilist-token"
          type="text"
          placeholder="Access Token"
          bind:value={manualToken}
          disabled={loading}
        />
        <button type="button" class="btn-primary" on:click={handleConnect} disabled={!manualToken.trim() || loading}>
          Connect
        </button>
      </div>
      <p class="help-text">
        Get your token at <a href="https://anilist.co/settings/developer" target="_blank" rel="noopener noreferrer">https://anilist.co/settings/developer</a> → create a client → copy the token.
        The token is NOT your Client ID. It's a long alphanumeric string generated when you create an application.
      </p>
    </details>
  {:else}
    <div class="connected-row">
      <span class="status">Connected</span>
      <button type="button" class="btn-import" on:click={handleImport} disabled={loading}>
        {loading ? 'Importing…' : 'Import Library'}
      </button>
      <button type="button" class="btn-disconnect" on:click={handleDisconnect} disabled={loading}>
        Disconnect
      </button>
    </div>

    {#if importReport}
      <dl class="report">
        <div><dt>Imported</dt><dd>{importReport.imported}</dd></div>
        <div><dt>Merged</dt><dd>{importReport.merged}</dd></div>
        <div><dt>Skipped</dt><dd>{importReport.skipped}</dd></div>
      </dl>
    {/if}
  {/if}
</section>

<style>
  .anilist-card {
    border: 1px solid rgba(143, 183, 255, 0.18);
    border-radius: var(--radius-card);
    padding: 1.25rem;
    background: rgba(255, 255, 255, 0.04);
    display: grid;
    gap: 0.75rem;
  }

  .eyebrow {
    color: var(--color-accent);
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.78rem;
    font-weight: 800;
  }

  .error {
    color: var(--color-error, #ff9d9d);
    font-size: 0.82rem;
  }

  .oauth-section {
    display: grid;
    gap: 0.6rem;
  }

  .oauth-heading {
    margin: 0;
    font-size: 0.85rem;
    font-weight: 700;
  }

  .form-group {
    display: grid;
    gap: 0.25rem;
  }

  .input-label {
    display: block;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--color-text);
    margin-bottom: 0.25rem;
  }

  .form-input {
    width: 100%;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 999px;
    padding: 0.55rem 1rem;
    color: var(--color-text);
    font-size: 0.85rem;
    box-sizing: border-box;
  }

  .form-input::placeholder {
    color: var(--color-muted);
  }

  .form-input:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  .action-btn {
    border-radius: 999px;
    padding: 0.55rem 1rem;
    font-size: 0.78rem;
    cursor: pointer;
    border: 1px solid rgba(143, 183, 255, 0.35);
    background: rgba(143, 183, 255, 0.18);
    color: #e9eefc;
    white-space: nowrap;
  }

  .action-btn:hover:not(:disabled) {
    background: rgba(143, 183, 255, 0.28);
  }

  .action-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .manual-section {
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    padding-top: 0.75rem;
  }

  .manual-summary {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--color-muted);
    cursor: pointer;
  }

  .manual-section[open] .manual-summary {
    margin-bottom: 0.5rem;
    display: inline-block;
  }

  .connect-row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex-wrap: wrap;
    margin-top: 0.5rem;
  }

  .help-text {
    font-size: 0.78rem;
    color: var(--color-muted);
    margin: 0;
    line-height: 1.5;
  }

  .help-text a {
    color: var(--color-accent);
  }

  input {
    flex: 1;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 999px;
    padding: 0.55rem 1rem;
    color: var(--color-text);
    font-size: 0.85rem;
  }

  input::placeholder {
    color: var(--color-muted);
  }

  input:focus {
    outline: 2px solid rgba(143, 183, 255, 0.5);
    outline-offset: 2px;
  }

  button {
    border-radius: 999px;
    padding: 0.55rem 1rem;
    font-size: 0.78rem;
    cursor: pointer;
    border: 1px solid rgba(143, 183, 255, 0.35);
    white-space: nowrap;
  }

  button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .btn-primary {
    background: rgba(143, 183, 255, 0.18);
    color: #e9eefc;
  }

  .btn-primary:hover:not(:disabled) {
    background: rgba(143, 183, 255, 0.28);
  }

  .connected-row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .status {
    color: var(--color-accent);
    font-size: 0.85rem;
    font-weight: 700;
    margin-right: 0.5rem;
  }

  .btn-import {
    background: rgba(143, 183, 255, 0.18);
    color: #e9eefc;
  }

  .btn-import:hover:not(:disabled) {
    background: rgba(143, 183, 255, 0.28);
  }

  .btn-disconnect {
    background: rgba(255, 157, 157, 0.15);
    border-color: rgba(255, 157, 157, 0.35);
    color: #ff9d9d;
  }

  .btn-disconnect:hover:not(:disabled) {
    background: rgba(255, 157, 157, 0.25);
  }

  .report {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 1rem;
    margin: 0;
  }

  .report div {
    display: grid;
    gap: 0.25rem;
  }

  .report dt {
    color: var(--color-muted);
    font-size: 0.78rem;
  }

  .report dd {
    margin: 0;
    font-weight: 700;
  }
</style>
