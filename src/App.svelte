<script lang="ts">
  const navItems = ['Home', 'Library', 'Watching', 'Calendar', 'Sync', 'Integrations', 'Settings'];
</script>

<main class="shell">
  <aside class="rail" aria-label="Main navigation">
    <div class="brand">Taiga Next</div>
    {#each navItems as item}
      <button class:active={item === 'Home'}>{item}</button>
    {/each}
  </aside>

  <section class="home">
    <p class="eyebrow">Foundation build</p>
    <h1>Premium dark anime library, local-first engine.</h1>
    <div class="card">
      <span>Phase 0</span>
      <strong>Engine scaffold ready for storage, migration, sync, and Sonarr integration.</strong>
    </div>
  </section>
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
</style>
