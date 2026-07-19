import sitemap from '@astrojs/sitemap';
import wasm from 'vite-plugin-wasm';
import { defineConfig } from 'astro/config';

export default defineConfig({
  site: 'https://brooke.largespiky.club',
  integrations: [sitemap()],
  vite: {
    plugins: [wasm()],
  },
});
