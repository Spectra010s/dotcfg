const siteUrl = "https://dotcfg.pxxl.click";

export const creator = {
  "@type": "Person",
  "@id": "https://github.com/Spectra010s#person",
  name: "Spectra010s",
  url: "https://github.com/Spectra010s",
};

type SchemaNode = {
  "@type": string;
  [key: string]: unknown;
};

export const software = {
  "@type": "SoftwareApplication",
  "@id": `${siteUrl}/#software`,
  name: "dotcfg",
  applicationCategory: "DeveloperApplication",
  operatingSystem: "Cross-platform",
  softwareVersion: "0.3.0",
  description: "Flexible config management for Rust applications.",
  programmingLanguage: "Rust",
  author: { "@id": creator["@id"] },
  creator: { "@id": creator["@id"] },
  codeRepository: "https://github.com/Spectra010s/dotcfg",
  downloadUrl: "https://crates.io/crates/dotcfg",
  sameAs: [
    "https://github.com/Spectra010s/dotcfg",
    "https://crates.io/crates/dotcfg",
    "https://docs.rs/dotcfg",
  ],
};

const website = {
  "@type": "WebSite",
  "@id": `${siteUrl}/#website`,
  name: "dotcfg documentation",
  url: siteUrl,
  creator: { "@id": creator["@id"] },
  about: { "@id": software["@id"] },
};

export const absoluteUrl = (path: string) =>
  path === "/" ? siteUrl : `${siteUrl}${path.startsWith("/") ? path : `/${path}`}`;

export const createSchema = (nodes: SchemaNode[]) => ({
  "@context": "https://schema.org",
  "@graph": nodes,
});

export const createBreadcrumbSchema = (
  title: string,
  path: string,
  parent?: { name: string; path: string },
) => ({
  "@type": "BreadcrumbList",
  "@id": `${absoluteUrl(path)}#breadcrumb`,
  itemListElement: [
    {
      "@type": "ListItem",
      position: 1,
      name: "Home",
      item: siteUrl,
    },
    ...(parent
      ? [{
          "@type": "ListItem",
          position: 2,
          name: parent.name,
          item: absoluteUrl(parent.path),
        }]
      : []),
    {
      "@type": "ListItem",
      position: parent ? 3 : 2,
      name: title,
      item: absoluteUrl(path),
    },
  ],
});

export const createHomeSchema = (description: string) =>
  createSchema([
    { ...website, description },
    software,
    creator,
  ]);

export const createDocsSchema = ({
  title,
  description,
  path,
}: {
  title: string;
  description: string;
  path: string;
}) =>
  createSchema([
    {
      "@type": "TechArticle",
      "@id": `${absoluteUrl(path)}#article`,
      name: title,
      headline: title,
      description,
      url: absoluteUrl(path),
      author: { "@id": creator["@id"] },
      isPartOf: { "@id": website["@id"] },
      about: { "@id": software["@id"] },
      breadcrumb: { "@id": `${absoluteUrl(path)}#breadcrumb` },
    },
    createBreadcrumbSchema(title, path),
    website,
    software,
    creator,
  ]);

export const createBlogIndexSchema = ({
  description,
  posts,
}: {
  description: string;
  posts: Array<{ title: string; path: string }>;
}) => {
  const path = "/blog";
  return createSchema([
    {
      "@type": "Blog",
      "@id": `${absoluteUrl(path)}#blog`,
      name: "dotcfg blog",
      description,
      url: absoluteUrl(path),
      author: { "@id": creator["@id"] },
      publisher: { "@id": creator["@id"] },
      isPartOf: { "@id": website["@id"] },
      about: { "@id": software["@id"] },
      blogPost: posts.map((post) => ({
        "@type": "BlogPosting",
        "@id": `${absoluteUrl(post.path)}#article`,
        headline: post.title,
        url: absoluteUrl(post.path),
      })),
    },
    createBreadcrumbSchema("Blog", path),
    website,
    software,
    creator,
  ]);
};

export const createBlogPostSchema = ({
  title,
  description,
  path,
  publishedAt,
  modifiedAt,
  tags = [],
  version,
}: {
  title: string;
  description: string;
  path: string;
  publishedAt: Date;
  modifiedAt?: Date;
  tags?: string[];
  version?: string;
}) => {
  const url = absoluteUrl(path);
  const article = {
    "@type": "BlogPosting",
    "@id": `${url}#article`,
    mainEntityOfPage: {
      "@type": "WebPage",
      "@id": url,
    },
    headline: title,
    name: title,
    description,
    url,
    datePublished: publishedAt.toISOString(),
    ...(modifiedAt ? { dateModified: modifiedAt.toISOString() } : {}),
    author: { "@id": creator["@id"] },
    publisher: { "@id": creator["@id"] },
    isPartOf: { "@id": `${siteUrl}/blog#blog` },
    about: { "@id": software["@id"] },
    programmingLanguage: "Rust",
    ...(tags.length ? { keywords: tags.join(", ") } : {}),
    ...(version ? { version } : {}),
    breadcrumb: { "@id": `${url}#breadcrumb` },
  };

  return createSchema([
    article,
    createBreadcrumbSchema(title, path, { name: "Blog", path: "/blog" }),
    {
      "@type": "Blog",
      "@id": `${siteUrl}/blog#blog`,
      name: "dotcfg blog",
      url: `${siteUrl}/blog`,
      isPartOf: { "@id": website["@id"] },
      about: { "@id": software["@id"] },
    },
    website,
    software,
    creator,
  ]);
};
