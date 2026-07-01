import './styles/tokens.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { setupTray } from './lib/tray';
import { getCurrentWindow } from '@tauri-apps/api/window';

const app = mount(App, {
  target: document.getElementById('app') as HTMLElement,
});

const appWindow = getCurrentWindow();

// Minimize to tray: hide instead of close
appWindow.onCloseRequested(async (event) => {
  event.preventDefault();
  await appWindow.hide();
});

setupTray();

export default app;
