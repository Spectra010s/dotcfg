import type { Metadata } from "next";
import Link from "next/link";
import "./globals.css";

export const metadata: Metadata = {
  title: "dotcfg — Rust configuration, your way",
  description: "Documentation for dotcfg, flexible configuration management for Rust applications.",
};

const sections = [
  ["Getting started", "/getting-started"],
  ["Directory strategies", "/directory-strategies"],
  ["Formats", "/formats"],
  ["Loading & saving", "/loading-saving"],
  ["Key access", "/key-access"],
  ["Environment overrides", "/environment"],
  ["Examples", "/examples"],
  ["API reference", "https://docs.rs/dotcfg"],
] as const;

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body>
        <header className="topbar">
          <Link className="brand" href="/"><span>dot</span>cfg</Link>
          <nav className="toplinks">
            <a className="optional" href="https://crates.io/crates/dotcfg">crates.io</a>
            <a href="https://github.com/Spectra010s/dotcfg">GitHub</a>
          </nav>
        </header>
        <div className="shell">
          <aside className="sidebar">
            <p className="sidebar-title">Documentation</p>
            <nav>
              {sections.map(([label, href]) => href.startsWith("http") ? (
                <a key={href} href={href}>{label}</a>
              ) : (
                <Link key={href} href={href}>{label}</Link>
              ))}
            </nav>
          </aside>
          <main>{children}</main>
        </div>
      </body>
    </html>
  );
}
