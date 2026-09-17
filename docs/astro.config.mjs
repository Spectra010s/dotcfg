import { defineConfig } from "astro/config";
import mdx from "@astrojs/mdx";
import sitemap from "@astrojs/sitemap";

export default defineConfig({
  site: "https://dotcfg.pxxl.click",
  integrations: [mdx(), sitemap()],
});
