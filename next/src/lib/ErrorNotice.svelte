<script lang="ts">
  // A problem told in plain words first: what happened and what it means, one
  // button that fixes it, and the raw error kept behind "Show details" with a
  // copy button for bug reports. The technical text never leads.
  import { createEventDispatcher } from 'svelte';

  export let message: string;
  export let details: string;
  export let actionLabel: string | null = null;

  const dispatch = createEventDispatcher<{ action: null; dismiss: null }>();

  let showDetails = false;
  let copied = false;

  async function copyDetails() {
    try {
      await navigator.clipboard.writeText(details);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch { /* clipboard unavailable: the text is still selectable */ }
  }
</script>

<div class="error-notice" role="alert">
  <div class="row">
    <div class="body">
      <span class="message">{message}</span>
      <div class="buttons">
        {#if actionLabel}
          <button type="button" class="action" on:click={() => dispatch('action')}>{actionLabel}</button>
        {/if}
        <button type="button" class="toggle" aria-expanded={showDetails} on:click={() => (showDetails = !showDetails)}>
          {showDetails ? 'Hide details' : 'Show details'}
        </button>
      </div>
    </div>
    <button type="button" class="dismiss" aria-label="Dismiss" on:click={() => dispatch('dismiss')}>×</button>
  </div>
  {#if showDetails}
    <div class="details">
      <pre>{details}</pre>
      <button type="button" class="toggle" on:click={copyDetails}>{copied ? 'Copied' : 'Copy details'}</button>
    </div>
  {/if}
</div>

<style>
  /* Above the Library and Collection sticky headers (z-index 6), whose
     negative top margin would otherwise slide over the notice's lower edge. */
  .error-notice {
    position: relative;
    z-index: 7;
    margin-bottom: 1rem;
    padding: 0.5rem 0.9rem;
    border: 1px solid rgba(var(--color-warning-rgb), 0.45);
    border-radius: 10px;
    background: rgba(var(--color-warning-rgb), 0.12);
    font-size: 0.85rem;
    color: var(--color-text);
  }

  .row {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
  }

  /* The message and its buttons wrap; the dismiss button stays top right. */
  .body {
    flex: 1;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.5rem 0.75rem;
  }

  .message { flex: 1 1 18rem; }

  .buttons {
    display: flex;
    gap: 0.5rem;
  }

  .action, .toggle {
    border: 1px solid rgba(var(--color-warning-rgb), 0.45);
    border-radius: 999px;
    padding: 0.25rem 0.75rem;
    background: rgba(var(--color-warning-rgb), 0.18);
    color: var(--color-text);
    font-size: 0.78rem;
    cursor: pointer;
  }
  .toggle { background: transparent; }
  .action:hover, .toggle:hover { background: rgba(var(--color-warning-rgb), 0.3); }

  .dismiss {
    border: none;
    background: transparent;
    color: var(--color-muted);
    font-size: 1rem;
    cursor: pointer;
    padding: 0 0.3rem;
  }
  .dismiss:hover { color: var(--color-text); }

  .details {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    margin-top: 0.5rem;
  }

  pre {
    flex: 1;
    margin: 0;
    padding: 0.5rem 0.6rem;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.25);
    color: var(--color-muted);
    font-size: 0.75rem;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
  }
</style>
