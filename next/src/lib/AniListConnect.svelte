<script lang="ts">
  import { storeAniListToken, disconnectAniList, importAniListLibrary } from './api';

  let clientId = '';
  let connected = false;
  let loading = false;
  let error: string | null = null;
  let importReport: { imported: number; merged: number; skipped: number } | null = null;

  async function handleConnect() {
    if (!clientId.trim()) return;
    error = null;
    try {
      await storeAniListToken(clientId.trim());
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
      clientId = '';
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
    <div class="connect-row">
      <input
        type="text"
        placeholder="Client ID / token"
        bind:value={clientId}
        disabled={loading}
      />
      <button type="button" class="btn-primary" on:click={handleConnect} disabled={!clientId.trim() || loading}>
        Connect
      </button>
    </div>
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

  .connect-row {
    display: flex;
    gap: 0.5rem;
    align-items: center;
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
