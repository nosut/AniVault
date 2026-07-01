<script lang="ts">
  import bannerUrl from './assets/banner.png';
  import NowPlaying from './lib/now-playing.svelte';

  const navItems = ['Home', 'Library', 'Watching', 'Calendar', 'Sync', 'Integrations', 'Settings'];
  let activeTab = $state('Home');
</script>

<main class="shell">
  <aside class="rail" aria-label="Main navigation">
    <div class="brand">AniVault</div>
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
    margin-bottom: 2rem;
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
</style>
