/**
 * Keydown handler for a non-button element with `role="button"`: Enter or
 * Space activates it, like a real button. Space only counts when the element
 * itself has focus, so a control nested inside it keeps its own Space
 * behaviour, and its default (scrolling the page) is suppressed.
 */
export function activateOnKey(activate: () => void): (event: KeyboardEvent) => void {
  return (event) => {
    if (event.key === 'Enter') {
      activate();
    } else if (event.key === ' ' && event.target === event.currentTarget) {
      event.preventDefault();
      activate();
    }
  };
}
