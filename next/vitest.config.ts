import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  // Svelte 5 ships separate client/server builds; tests mount components in
  // jsdom, so resolve the browser build.
  resolve: {
    conditions: ['browser'],
  },
  test: {
    globals: true,
    environment: 'node',
  },
});
