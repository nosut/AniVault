<script lang="ts">
  import bannerUrl from './assets/banner.png';
  import NowPlaying from './lib/now-playing.svelte';
  import { startOAuth, completeOAuth, getOAuthStatus } from './lib/api';
  import type { OAuthStatus } from './lib/api';
  import { getSyncStatus } from './lib/api';
  import type { SyncStatus } from './lib/api';
  import { getLibraryAnime } from './lib/api';
  import type { LibraryEntry } from './lib/api';

  const navItems = ['Home', 'Library', 'Watching', 'Calendar', 'Sync', 'Integrations', 'Settings'];
  let activeTab = $state('Home');

  let oauthStatus: OAuthStatus = $state({ authenticated: false, username: null });
  let oauthLoading: boolean = $state(false);
  let oauthMessage: string | null = $state(null);
  let syncPending: number = $state(0);
  let library: LibraryEntry[] = $state([]);

  async function loadLibrary() {
    try { library = await getLibraryAnime(); } catch { /* empty */ }
  }
  $effect(() => { if (activeTab === 'Library') loadLibrary(); });

  async function refreshSyncStatus() {
    try {
      const s = await getSyncStatus();
      syncPending = s.pending;
    } catch { /* ignore */ }
  }

  setInterval(refreshSyncStatus, 10_000);
  refreshSyncStatus();

  async function refreshOAuthStatus() {
    try {
      oauthStatus = await getOAuthStatus();
      oauthMessage = null;
    } catch {
      /* ignore */
    }
  }

  async function handleStartOAuth() {
    oauthLoading = true;
    oauthMessage = null;
    try {
      await startOAuth();
      oauthMessage = 'Browser opened. Authorize AniList, then click Complete below.';
    } catch (e) {
      oauthMessage = `Error: ${e}`;
    }
    oauthLoading = false;
  }

  async function handleCompleteOAuth() {
    oauthLoading = true;
    oauthMessage = null;
    try {
      oauthStatus = await completeOAuth();
      oauthMessage = oauthStatus.authenticated
        ? `Connected as ${oauthStatus.username ?? 'unknown'}.`
        : 'Authentication failed.';
    } catch (e) {
      oauthMessage = `Error: ${e}`;
    }
    oauthLoading = false;
  }

  refreshOAuthStatus();
</script>

<main class="shell">
  <aside class="rail" aria-label="Main navigation">
    <div class="brand">AniVault</div>
    <div class="sync-dot" class:amber={syncPending > 0} title={syncPending ? `${syncPending} pending sync` : 'Sync up to date'}>
      <span class="dot"></span>
      {#if syncPending > 0}
        <span class="count">{syncPending}</span>
      {/if}
    </div>
    {#each navItems as item}
      <button class:active={item === activeTab} onclick={() => activeTab = item}>{item}</button>
    {/each}
  </aside>

  {#if activeTab === 'Home'}
  <section class="home">
    <img class="banner" src={bannerUrl} alt="AniVault" />
    <NowPlaying />
    <p class="eyebrow">Foundation build</p>
    <h1>Your premium dark anime vault.</h1>
    <div class="card">
      <span>AniVault Preview</span>
      <strong>Engine scaffold ready for storage, migration, sync, Sonarr integration, and future tracking workflows.</strong>
    </div>
  </section>
  {:else if activeTab === 'Settings'}
  <section class="home">
    <p class="eyebrow">Settings</p>
    <div class="card">
      <span>AniList OAuth</span>
      <p class="oauth-status">
        {#if oauthStatus.authenticated}
          Connected as <strong>{oauthStatus.username ?? 'unknown'}</strong>.
        {:else}
          Not connected.
        {/if}
      </p>
      <div class="oauth-actions">
        {#if !oauthStatus.authenticated}
          <button class="oauth-btn" onclick={handleStartOAuth} disabled={oauthLoading}>
            {oauthLoading ? 'Opening browser...' : 'Connect AniList'}
          </button>
          <button class="oauth-btn" onclick={handleCompleteOAuth} disabled={oauthLoading}>
            Complete Authorization
          </button>
        {:else}
          <button class="oauth-btn" onclick={refreshOAuthStatus}>Refresh</button>
        {/if}
      </div>
      {#if oauthMessage}
        <p class="oauth-msg">{oauthMessage}</p>
      {/if}
    </div>
  </section>
  {:else if activeTab === 'Library'}
  <section class="home">
    <p class="eyebrow">Library</p>
    {#if library.length === 0}
      <div class="card"><span>No anime in library yet. Play a file to auto-add.</span></div>
    {:else}
      <div class="library-list">
        {#each library as entry}
          <div class="lib-row">
            <span class="lib-title">{entry.title}</span>
            <span class="lib-status status-{entry.status}">{entry.status.replace('_', ' ')}</span>
            <span class="lib-ep">Ep {entry.watched_episodes}{#if entry.episode_count} / {entry.episode_count}{/if}</span>
          </div>
        {/each}
      </div>
    {/if}
  </section>
  {:else}
  <section class="home">
    <p class="eyebrow">{activeTab}</p>
    <h1>Coming soon.</h1>
  </section>
  {/if}
</main>

<style>
  .shell {
    display: grid;
    grid-template-columns: 16rem 1fr;
    min-height: 100vh;
  }

  .rail {
    border-right: 1px solid rgb(255 255 255 / 8%);
    background: rgb(10 13 20 / 72%);
    padding: 1.5rem;
    backdrop-filter: blur(24px);
  }

  .brand {
    font-weight: 800;
    letter-spacing: -0.04em;
    margin-bottom: 1rem;
  }

  .sync-dot {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-bottom: 1.5rem;
    font-size: 0.72rem;
    color: var(--color-muted);
  }

  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #22c55e;
  }

  .sync-dot.amber .dot {
    background: #f59e0b;
  }

  .count {
    color: var(--color-text);
  }

  button {
    display: block;
    width: 100%;
    border: 0;
    border-radius: 999px;
    margin: 0.25rem 0;
    padding: 0.8rem 1rem;
    text-align: left;
    color: var(--color-muted);
    background: transparent;
  }

  button.active,
  button:hover {
    color: var(--color-text);
    background: rgb(255 255 255 / 8%);
  }

  .home {
    padding: 4rem;
  }

  .eyebrow {
    color: var(--color-accent);
    text-transform: uppercase;
    letter-spacing: 0.16em;
    font-size: 0.78rem;
    font-weight: 800;
  }

  h1 {
    max-width: 54rem;
    font-size: clamp(3rem, 7vw, 6rem);
    line-height: 0.94;
    letter-spacing: -0.08em;
  }

  .card {
    display: grid;
    gap: 0.5rem;
    max-width: 34rem;
    border: 1px solid rgb(255 255 255 / 10%);
    border-radius: var(--radius-card);
    background: linear-gradient(145deg, rgb(255 255 255 / 12%), rgb(255 255 255 / 4%));
    box-shadow: var(--shadow-card);
    padding: 1.5rem;
  }

  .card span {
    color: var(--color-muted);
  }

  .banner {
    display: block;
    width: min(34rem, 100%);
    height: auto;
    margin-bottom: 2rem;
    border-radius: var(--radius-card);
    box-shadow: var(--shadow-card);
  }

  .oauth-status {
    color: var(--color-muted);
    font-size: 0.9rem;
    margin: 0.5rem 0;
  }

  .oauth-status strong {
    color: var(--color-text);
  }

  .oauth-actions {
    display: flex;
    gap: 0.75rem;
    margin: 1rem 0;
  }

  .oauth-btn {
    border: 1px solid rgb(255 255 255 / 14%);
    border-radius: 999px;
    padding: 0.5rem 1.2rem;
    color: var(--color-text);
    background: rgb(255 255 255 / 6%);
    cursor: pointer;
    font-size: 0.82rem;
    transition: background 0.15s;
  }

  .oauth-btn:hover:not(:disabled) {
    background: rgb(255 255 255 / 12%);
  }

  .oauth-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .oauth-msg {
    color: var(--color-accent);
    font-size: 0.82rem;
    margin-top: 0.5rem;
  }

  .library-list {
    max-width: 42rem;
  }

  .lib-row {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.6rem 0;
    border-bottom: 1px solid rgb(255 255 255 / 6%);
    font-size: 0.9rem;
  }

  .lib-title {
    flex: 1;
    color: var(--color-text);
  }

  .lib-status {
    text-transform: capitalize;
    font-size: 0.72rem;
    padding: 0.15rem 0.5rem;
    border-radius: 999px;
    background: rgb(255 255 255 / 6%);
    color: var(--color-muted);
  }

  .status-watching { color: #22c55e; }
  .status-completed { color: #60a5fa; }
  .status-on_hold, .status-onhold { color: #f59e0b; }
  .status-dropped { color: #ef4444; }

  .lib-ep {
    color: var(--color-muted);
    font-size: 0.78rem;
    min-width: 5rem;
    text-align: right;
  }
</style>
