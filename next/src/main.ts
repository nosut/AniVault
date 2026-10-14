import './styles/tokens.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { initPrefs } from './lib/prefs';
import { reportProblem } from './lib/notify';

// Last line of defence: a failure no code caught would otherwise do nothing
// visible. These are bugs, so the details keep the stack for a report.
const UNEXPECTED = "Something in AniVault didn't work as expected.";
const withStack = (e: unknown) => (e instanceof Error && e.stack ? e.stack : e);
window.addEventListener('unhandledrejection', (event) => reportProblem(UNEXPECTED, withStack(event.reason)));
window.addEventListener('error', (event) => {
  // Harmless browser layout notice, not a failure.
  if (/ResizeObserver loop/i.test(event.message)) return;
  reportProblem(UNEXPECTED, withStack(event.error ?? event.message));
});

// Views read their saved preferences synchronously while they initialise, so
// the store has to be loaded before anything mounts. initPrefs never rejects:
// a failure is reported through prefsError and the views fall back to defaults.
void initPrefs().finally(() => {
  mount(App, { target: document.getElementById('app') as HTMLElement });
});
