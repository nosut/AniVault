import './styles/tokens.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { setupTray } from './lib/tray';

const app = mount(App, {
  target: document.getElementById('app') as HTMLElement,
});

setupTray();

export default app;
