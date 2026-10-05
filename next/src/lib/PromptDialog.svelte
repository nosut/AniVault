<script lang="ts">
  // Centered modal shared by the status-change prompts: backdrop, heading,
  // and Escape/backdrop to close unless `busy`.
  import { createEventDispatcher } from 'svelte';

  export let eyebrow: string;
  export let heading: string;
  export let busy = false;

  const dispatch = createEventDispatcher<{ close: void }>();
  const headingId = `prompt-${Math.random().toString(36).slice(2)}`;

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !busy) dispatch('close');
  }
</script>

<svelte:window on:keydown={onKeydown} />

<div class="backdrop" role="presentation" on:click={() => !busy && dispatch('close')}></div>
<div class="dialog" role="dialog" aria-modal="true" aria-labelledby={headingId}>
  <span class="eyebrow">{eyebrow}</span>
  <h2 id={headingId}>{heading}</h2>
  <slot />
</div>

<style>
  .backdrop { position: fixed; inset: 0; z-index: 60; background: rgba(0, 0, 0, 0.55); }
  .dialog {
    position: fixed; z-index: 61; left: 50%; top: 50%; transform: translate(-50%, -50%);
    width: min(28rem, calc(100vw - 2rem)); max-height: calc(100vh - 2rem); overflow-y: auto;
    display: flex; flex-direction: column; gap: 0.75rem;
    padding: 1.1rem 1.25rem; border-radius: 12px;
    border: 1px solid rgba(var(--color-accent-rgb), 0.35);
    background: var(--color-surface); box-shadow: 0 16px 40px rgba(0, 0, 0, 0.55);
  }
  .eyebrow { font-size: 0.66rem; font-weight: 800; letter-spacing: 0.12em; text-transform: uppercase; color: var(--color-accent); }
  h2 { margin: 0; font-size: 1.05rem; color: var(--color-text); }

  /* Shared by the dialogs' slotted content. */
  .dialog :global(fieldset) { border: 1px solid rgba(255, 255, 255, 0.08); border-radius: 8px; padding: 0.6rem 0.75rem; margin: 0; display: flex; flex-direction: column; gap: 0.4rem; }
  .dialog :global(legend) { padding: 0 0.3rem; font-size: 0.78rem; color: var(--color-muted); }
  .dialog :global(label) { display: flex; align-items: center; gap: 0.5rem; font-size: 0.85rem; cursor: pointer; }
  .dialog :global(label.nested) { margin-left: 1.5rem; }
  .dialog :global(.hint) { margin: 0; font-size: 0.72rem; color: var(--color-muted); }
  .dialog :global(.warning) { margin: 0; font-size: 0.78rem; color: var(--color-warning); }
  .dialog :global(.error) { margin: 0; font-size: 0.82rem; color: var(--color-warning); }
  .dialog :global(.errors) { margin: 0; padding-left: 1.1rem; font-size: 0.78rem; color: var(--color-muted); word-break: break-all; }
  .dialog :global(.shows) { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.3rem; max-height: 10rem; overflow-y: auto; }
  .dialog :global(.shows li) { display: flex; justify-content: space-between; align-items: center; gap: 0.75rem; font-size: 0.82rem; }
  .dialog :global(.show-title) { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dialog :global(.show-meta) { color: var(--color-muted); white-space: nowrap; }
  .dialog :global(.actions) { display: flex; justify-content: flex-end; gap: 0.5rem; }
  .dialog :global(.actions button), .dialog :global(button.pill) {
    border: 1px solid rgba(255, 255, 255, 0.12); border-radius: 999px; padding: 0.4rem 0.95rem;
    background: transparent; color: var(--color-text); font-size: 0.82rem; cursor: pointer;
  }
  .dialog :global(button.pill) { padding: 0.25rem 0.7rem; font-size: 0.75rem; white-space: nowrap; }
  .dialog :global(.actions button.primary) { border-color: rgba(var(--color-accent-rgb), 0.35); background: rgba(var(--color-accent-rgb), 0.18); }
  .dialog :global(.actions button:disabled), .dialog :global(button.pill:disabled) { opacity: 0.5; cursor: default; }
</style>
