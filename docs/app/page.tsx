import Link from "next/link";

const topics = [
  ["Directory strategies", "Use a dot directory, XDG, an exact custom path, or discover project config from ancestors.", "/directory-strategies"],
  ["Multiple formats", "TOML by default, with feature-gated JSON and YAML support.", "/formats"],
  ["Typed key access", "Read and write individual values as Rust types without replacing the whole config.", "/key-access"],
  ["Environment overrides", "Layer environment variables over file configuration for CLI and deployment workflows.", "/environment"],
] as const;

export default function Home() {
  return (
    <>
      <span className="eyebrow">Flexible configuration for Rust</span>
      <h1>Config that fits your application.</h1>
      <p className="lead">
        dotcfg handles where configuration lives, how it is encoded, and how you read it — without forcing one directory strategy or one access pattern.
      </p>

      <pre><code>{`use dotcfg::DotCfg;\n\nlet cfg = DotCfg::new("mytool");\nlet config: Option&lt;Config&gt; = cfg.load()?;`}</code></pre>

      <div className="cards">
        {topics.map(([title, description, href]) => (
          <Link className="card" href={href} key={href}>
            <strong>{title}</strong>
            <p>{description}</p>
          </Link>
        ))}
      </div>

      <h2>Start with the basics</h2>
      <p>
        Add <code>dotcfg</code> to your Rust application, choose the configuration strategy that matches your project, then load a complete typed config or work with individual keys.
      </p>
      <p><Link href="/getting-started"><strong>Read the getting started guide →</strong></Link></p>
    </>
  );
}
