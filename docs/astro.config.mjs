import { defineConfig } from 'astro/config';
import ogImages from './integrations/og-images.mjs';
import mdx from '@astrojs/mdx';
import sitemap from '@astrojs/sitemap';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  site: 'https://dotcfg.spectra010s.com',
  integrations: [mdx(), sitemap(), ogImages()],
  vite: {
    plugins: [tailwindcss()],
  },
});
