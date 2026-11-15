<script lang="ts">
  // Episode tiles: click one to play it. `compact` is the sidebar strip, a
  // single sideways-scrolling row that keeps the playing/next tile in view.
  import { createEventDispatcher } from 'svelte';
  import { fileName, stateLabel, type EpisodeTile } from './episodePlayback';

  export let tiles: EpisodeTile[] = [];
  export let compact = false;

  const dispatch = createEventDispatcher<{ play: EpisodeTile }>();

  function tooltip(t: EpisodeTile): string {
    const label = stateLabel(t.state);
    const head = `Episode ${t.episode}${label ? ` · ${label}` : ''}`;
    return t.file ? `${head}\n${fileName(t.file.file_path)}` : head;
  }

  function play(t: EpisodeTile) {
    if (t.file) dispatch('play', t);
  }

  // The episode the strip centres on. Keyed on its number, not the tile
  // array: the status poll rebuilds the array every two seconds, and
  // re-centring on each poll would fight a user scrolling the strip.
  $: focusEpisode = compact ? tiles.find((t) => t.state === 'playing' || t.state === 'next')?.episode ?? null : null;

  function centreOn(node: HTMLElement, episode: number | null) {
    const centre = () => requestAnimationFrame(() => {
      const el = node.querySelector<HTMLElement>('.playing, .next');
      if (el) node.scrollLeft = Math.max(0, el.offsetLeft - node.clientWidth / 2 + el.clientWidth / 2);
    });
    if (episode != null) centre();
    return {
      update(next: number | null) {
        if (next != null) centre();
      },
    };
  }
</script>

<div class="eps" class:compact use:centreOn={focusEpisode} role="group" aria-label="Episodes">
  {#each tiles as t (t.season * 10000 + t.episode)}
    <button
      type="button"
      class="ep {t.state}"
      title={tooltip(t)}
      aria-label={`Play episode ${t.episode}${stateLabel(t.state) ? `, ${stateLabel(t.state)}` : ''}`}
      aria-current={t.state === 'playing' ? 'true' : undefined}
      disabled={!t.file}
      on:click={() => play(t)}
    >
      <span class="num">{t.episode}</span>
      {#if !compact && (t.state === 'playing' || t.state === 'next' || t.state === 'missing')}
        <small>{stateLabel(t.state)}</small>
      {/if}
    </button>
  {/each}
</div>

<style>
  .eps {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(58px, 1fr));
    gap: 0.45rem;
  }

  .eps.compact {
    position: relative;
    display: flex;
    gap: 0.25rem;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .eps.compact::-webkit-scrollbar { display: none; }

  .ep {
    position: relative;
    aspect-ratio: 1.25;
    border-radius: 12px;
    border: 1px solid transparent;
    background: var(--color-surface-raised);
    color: var(--color-text);
    display: grid;
    place-items: center;
    font-weight: 700;
    font-size: 1rem;
    font-variant-numeric: tabular-nums;
    cursor: pointer;
    padding: 0;
  }

  .compact .ep {
    flex: 0 0 28px;
    aspect-ratio: 1;
    border-radius: 7px;
    font-size: 0.7rem;
  }

  .ep small {
    position: absolute;
    bottom: 4px;
    font-size: 0.56rem;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--color-muted);
  }

  .ep:hover:not(:disabled):not(.playing) { background: rgba(var(--color-accent-rgb), 0.16); }
  .ep:focus-visible { outline: 2px solid var(--color-accent); outline-offset: 2px; }

  .ep.watched { background: rgba(255, 255, 255, 0.03); color: var(--color-muted); }
  .ep.watched::after {
    content: '✓';
    position: absolute;
    top: 3px;
    right: 6px;
    font-size: 0.65rem;
    color: var(--color-success);
  }
  .compact .ep.watched::after { display: none; }

  .ep.playing { background: var(--color-accent); color: var(--color-bg); }
  .ep.playing small { color: var(--color-bg); }

  .ep.next { border-color: var(--color-accent); }
  .ep.next small { color: var(--color-accent); }

  .ep.missing {
    background: transparent;
    border: 1px dashed rgba(154, 166, 184, 0.35);
    color: rgba(154, 166, 184, 0.55);
    cursor: not-allowed;
  }

  @media (prefers-reduced-motion: no-preference) {
    .ep.playing { animation: ep-pulse 2.4s ease-in-out infinite; }
    @keyframes ep-pulse {
      0%, 100% { box-shadow: 0 0 0 0 rgba(var(--color-accent-rgb), 0.45); }
      50% { box-shadow: 0 0 0 5px rgba(var(--color-accent-rgb), 0); }
    }
  }
</style>
