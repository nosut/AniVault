import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

// Used by svelte-check (and IDE tooling) to preprocess TypeScript in
// components; the vite build configures the plugin separately.
export default {
  preprocess: vitePreprocess(),
};
