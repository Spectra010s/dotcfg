import { notFound } from "next/navigation";

const pages: Record<string, { title: string; description: string }> = {
  "getting-started": { title: "Getting started", description: "Install dotcfg and load your first Rust configuration." },
  "directory-strategies": { title: "Directory strategies", description: "Choose where dotcfg looks for and stores application configuration." },
  formats: { title: "Configuration formats", description: "Use TOML, JSON, or YAML through feature-gated format support." },
  "loading-saving": { title: "Loading & saving", description: "Load optional or required configuration and persist typed structures." },
  "key-access": { title: "Key access", description: "Read and update individual flat or nested values, including typed values." },
  environment: { title: "Environment overrides", description: "Let prefixed environment variables override values from configuration files." },
  examples: { title: "Examples", description: "Practical dotcfg patterns for Rust applications and command-line tools." },
};

export default async function DocsPage({ params }: { params: Promise<{ slug?: string[] }> }) {
  const { slug } = await params;
  const key = slug?.join("/") ?? "";
  const page = pages[key];
  if (!page) notFound();

  return (
    <article>
      <span className="eyebrow">dotcfg documentation</span>
      <h1>{page.title}</h1>
      <p className="lead">{page.description}</p>
      <h2>Documentation in progress</h2>
      <p>This section is part of the documentation structure and will be filled out in the corresponding documentation task.</p>
    </article>
  );
}
