import './styles/tokens.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { initPrefs } from './lib/prefs';

// Views read their saved preferences synchronously while they initialise, so
// the store has to be loaded before anything mounts. initPrefs never rejects:
// a failure is reported through prefsError and the views fall back to defaults.
void initPrefs().finally(() => {
  mount(App, { target: document.getElementById('app') as HTMLElement });
});
