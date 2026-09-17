import rss from "@astrojs/rss";
import { getCollection } from "astro:content";
import type { APIContext } from "astro";

export async function GET(context: APIContext) {
  const posts = await getCollection("blog", ({ data }) => !data.draft);
  return rss({
    title: "dotcfg blog",
    description: "Articles about dotcfg, configuration management in Rust, and the design decisions behind the crate.",
    site: context.site?.href ?? "https://dotcfg.pxxl.click",
    items: posts
      .sort((a, b) => b.data.publishedAt.getTime() - a.data.publishedAt.getTime())
      .map((post) => ({
        title: post.data.title,
        description: post.data.description,
        pubDate: post.data.publishedAt,
        link: `/blog/${post.id}`,
      })),
  });
}
