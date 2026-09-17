const siteUrl = "https://dotcfg.pxxl.click";

const creator = {
  "@type": "Person",
  name: "Spectra010s",
  url: "https://github.com/Spectra010s",
};

type SchemaNode = {
  "@type": string;
  [key: string]: unknown;
};

const software = {
  "@type": "SoftwareApplication",
  name: "dotcfg",
  applicationCategory: "DeveloperApplication",
  operatingSystem: "Cross-platform",
  softwareVersion: "0.2.0",
  description: "Flexible config management for Rust applications.",
  programmingLanguage: "Rust",
  author: creator,
  creator,
  codeRepository: "https://github.com/Spectra010s/dotcfg",
  downloadUrl: "https://crates.io/crates/dotcfg",
  sameAs: [
    "https://github.com/Spectra010s/dotcfg",
    "https://crates.io/crates/dotcfg",
    "https://docs.rs/dotcfg",
  ],
};

const absoluteUrl = (path: string) =>
  path === "/" ? siteUrl : `${siteUrl}${path.startsWith("/") ? path : `/${path}`}`;

export const createSchema = (nodes: SchemaNode[]) => ({
  "@context": "https://schema.org",
  "@graph": nodes,
});

export const createBreadcrumbSchema = (title: string, path: string) => ({
  "@type": "BreadcrumbList",
  itemListElement: [
    {
      "@type": "ListItem",
      position: 1,
      name: "Home",
      item: siteUrl,
    },
    {
      "@type": "ListItem",
      position: 2,
      name: title,
      item: absoluteUrl(path),
    },
  ],
});

export const createHomeSchema = (description: string) =>
  createSchema([
    {
      "@type": "WebSite",
      name: "dotcfg documentation",
      url: siteUrl,
      description,
      creator,
      about: software,
    },
    software,
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
      name: title,
      headline: title,
      description,
      url: absoluteUrl(path),
      author: creator,
      isPartOf: {
        "@type": "WebSite",
        name: "dotcfg documentation",
        url: siteUrl,
      },
      about: software,
    },
    createBreadcrumbSchema(title, path),
  ]);
