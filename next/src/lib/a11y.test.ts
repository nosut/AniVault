// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest';
import { activateOnKey } from './a11y';

function press(key: string, target?: HTMLElement) {
  const card = document.createElement('div');
  const inner = document.createElement('button');
  card.appendChild(inner);
  const handler = vi.fn();
  card.addEventListener('keydown', activateOnKey(handler));
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
  (target === undefined ? card : inner).dispatchEvent(event);
  return { handler, event };
}

describe('activateOnKey', () => {
  it('activates on Enter', () => {
    expect(press('Enter').handler).toHaveBeenCalledTimes(1);
  });

  it('activates on Space and stops the page from scrolling', () => {
    const { handler, event } = press(' ');
    expect(handler).toHaveBeenCalledTimes(1);
    expect(event.defaultPrevented).toBe(true);
  });

  it('ignores other keys', () => {
    expect(press('a').handler).not.toHaveBeenCalled();
    expect(press('Tab').handler).not.toHaveBeenCalled();
  });

  it('leaves Space alone when it comes from a control inside the card', () => {
    const { handler, event } = press(' ', document.createElement('span'));
    expect(handler).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(false);
  });
});
