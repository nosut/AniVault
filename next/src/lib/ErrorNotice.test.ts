// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { flushSync } from 'svelte';
import { createClassComponent } from 'svelte/legacy';
import ErrorNotice from './ErrorNotice.svelte';

function mount(props: { message: string; details: string; actionLabel?: string | null }) {
  const component = createClassComponent({ component: ErrorNotice, target: document.getElementById('app')!, props });
  const action = vi.fn();
  const dismiss = vi.fn();
  component.$on('action', action);
  component.$on('dismiss', dismiss);
  return { action, dismiss };
}

const button = (label: string) =>
  [...document.querySelectorAll('button')].find((b) => b.textContent?.trim() === label || b.getAttribute('aria-label') === label);

describe('ErrorNotice', () => {
  beforeEach(() => { document.body.innerHTML = '<div id="app"></div>'; });

  it('shows the plain message and keeps the technical details hidden until asked', () => {
    mount({ message: 'A change could not be saved.', actionLabel: 'Try again', details: 'SqliteError: database is locked' });
    expect(document.body.textContent).toContain('A change could not be saved.');
    expect(document.body.textContent).not.toContain('database is locked');

    button('Show details')!.click();
    flushSync();
    expect(document.body.textContent).toContain('database is locked');
    expect(button('Hide details')).toBeTruthy();
  });

  it('raises the action and dismiss events', () => {
    const { action, dismiss } = mount({ message: 'm', actionLabel: 'Try again', details: 'd' });
    button('Try again')!.click();
    button('Dismiss')!.click();
    expect(action).toHaveBeenCalledOnce();
    expect(dismiss).toHaveBeenCalledOnce();
  });

  it('copies the details for a bug report', async () => {
    const writeText = vi.fn(async () => {});
    Object.assign(navigator, { clipboard: { writeText } });
    mount({ message: 'm', details: 'SqliteError: database is locked' });
    button('Show details')!.click();
    flushSync();
    button('Copy details')!.click();
    expect(writeText).toHaveBeenCalledWith('SqliteError: database is locked');
  });

  it('omits the action button when there is nothing to do', () => {
    mount({ message: 'm', details: 'd' });
    expect(button('Try again')).toBeUndefined();
  });
});
