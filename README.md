# dotcfg

[![CI](https://github.com/Spectra010s/dotcfg/actions/workflows/ci.yml/badge.svg)](https://github.com/Spectra010s/dotcfg/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/dotcfg)](https://crates.io/crates/dotcfg)
[![docs.rs](https://img.shields.io/docsrs/dotcfg)](https://docs.rs/dotcfg)

Flexible config management for Rust apps.

## Why dotcfg?

Most config crates either lock you into a fixed directory strategy or only handle reading. `dotcfg` gives you full control:

- Choose `~/.toolname/` or `~/.config/toolname/` — your call
- TOML, JSON or YAML — feature-gated, include only what you need
- Load the whole config or get/set individual keys without touching the rest
- Read and write keys as real types — numbers, bools, arrays, structs — not just strings
- Returns `None` if config doesn't exist — no magic, no forced defaults unless you want them

### Comparison with `confy`

| Feature | `dotcfg` | `confy` |
| :--- | :---: | :---: |
| **Dot-dir support (`~/.app/`)** | ✅ Built-in default | ❌ (XDG/Native only) |
| **XDG directory (`~/.config/app/`)** | ✅ via `.xdg()` | ✅ |
| **Ad-hoc key get/set by string path** | ✅ (`cfg.get("user.name")`) | ❌ (Full struct only) |
| **Typed key get/set** | ✅ (`cfg.get_as::<u16>("port")`) | ❌ (Full struct only) |
| **Full struct load/save** | ✅ | ✅ |
| **Missing file handling** | ✅ Flexible (`None`, default, or error) | ⚠️ Forces file creation with `Default` |
| **Multiple formats compiled in** | ✅ (TOML, JSON, YAML together) | ❌ Only 1 format can be compiled in |

## Installation

```toml
# TOML only (default)
dotcfg = "0.1"

# JSON only
dotcfg = { version = "0.1", default-features = false, features = ["json"] }

# YAML only
dotcfg = { version = "0.1", default-features = false, features = ["yaml"] }

# All three
dotcfg = { version = "0.1", features = ["json", "yaml"] }
```

## Quick Start

```rust
use dotcfg::DotCfg;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
struct Config {
    username: String,
    port: u16,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ~/.mytool/config.toml (default)
    let cfg = DotCfg::new("mytool");

    // Save — creates dir if needed
    cfg.save(&Config { username: "tayo".into(), port: 8080 })?;

    // Load — None if file doesn't exist
    let config: Option<Config> = cfg.load()?;

    // Per-key (no need to load whole struct)
    cfg.set("username", "jane")?;
    let val = cfg.get("user.username")?;

    // Per-key, typed
    cfg.set_val("port", 8080u16)?;
    let port: u16 = cfg.get_as("port")?;

    Ok(())
}
```

## Directory Strategy

```rust
// ~/.mytool/config.toml  (default — like .cargo, .ssh)
let cfg = DotCfg::new("mytool");

// ~/.config/mytool/config.toml  (XDG standard, cross-platform)
let cfg = DotCfg::new("mytool").xdg();

// explicit dot (same as default)
let cfg = DotCfg::new("mytool").dot();
```

## Format

```rust
// TOML (default)
let cfg = DotCfg::new("mytool");

// JSON (needs `json` feature)
let cfg = DotCfg::new("mytool").json();

// YAML (needs `yaml` feature)
let cfg = DotCfg::new("mytool").yaml();
// → ~/.mytool/config.yaml

// Custom filename
let cfg = DotCfg::new("mytool").json().filename("settings");
// → ~/.mytool/settings.json  (or ~/.config/... with .xdg())
```

## Loading

```rust
// Returns None if file doesn't exist — you decide what to do
let config: Option<Config> = cfg.load()?;

// Returns an error if file doesn't exist — useful for CLIs that require setup
let config: Config = cfg.load_or_error()?;

// Creates the file with Default values if it doesn't exist — opt-in
let config: Config = cfg.load_or_default()?;
```

## Per-key Get & Set

Get or set individual keys without loading or overwriting the whole config.

Supports flat and nested (one level: `section.field`) keys:

```rust
// Flat
cfg.get("username")?;
cfg.set("username", "tayo")?;

// Nested → [user] table in TOML / nested object in JSON / nested mapping in YAML
cfg.get("user.username")?;
cfg.set("user.username", "tayo")?;
```

`set` creates the file/dir if missing and preserves all other keys. Values are stored as strings.

## Typed Get & Set

`get` and `set` deal in strings. `get_as` and `set_val` deal in real types — the value
goes straight through serde in the config format's own representation, with no
stringify/re-parse step in between:

```rust
// Write native values — numbers stay numbers, arrays stay arrays
cfg.set_val("port", 8080u16)?;
cfg.set_val("ratio", 0.75)?;
cfg.set_val("plugins", vec!["fmt", "lint"])?;
cfg.set_val("features.auto_update", true)?;

// Read them back into whatever type you need
let port: u16 = cfg.get_as("port")?;
let plugins: Vec<String> = cfg.get_as("plugins")?;
let auto: bool = cfg.get_as("features.auto_update")?;
```

Any `Serialize` type goes in and any `DeserializeOwned` type comes out, so a whole
section can be read as a struct:

```rust
#[derive(Serialize, Deserialize)]
struct Server { host: String, port: u16 }

cfg.set_val("server", Server { host: "localhost".into(), port: 8080 })?;
let server: Server = cfg.get_as("server")?;
```

These are additive — `get`/`set` behave exactly as before. `set_val` follows the same
rules as `set`: it creates the file/dir if missing, creates the intermediate table for a
`section.field` key, and preserves every other key. A type mismatch (say `get_as::<u16>`
on a key holding `"alice"`) returns the format's serde error rather than panicking.

## Clap Integration

Combine `dotcfg` with `clap` to support standard CLI configuration precedence:
**CLI argument flag > Config file on disk > Default fallback**.

```rust
use clap::Parser;
use dotcfg::DotCfg;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
struct AppConfig {
    host: String,
    port: u16,
}

#[derive(Parser)]
struct Cli {
    #[arg(short, long)]
    host: Option<String>,

    #[arg(short, long)]
    port: Option<u16>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();
    let cfg = DotCfg::new("mycli");

    // Load config from disk or fallback to defaults
    let saved: AppConfig = cfg.load()?.unwrap_or_default();

    // CLI flag takes precedence over config file
    let host = args.host.unwrap_or(saved.host);
    let port = args.port.unwrap_or(saved.port);

    println!("Running on {host}:{port}");
    Ok(())
}
```

See [`examples/clap_cli.rs`](examples/clap_cli.rs) for a full runnable example.

## Other Utilities

```rust
cfg.exists()?;      // does config file exist?
cfg.dir()?;         // config directory path
cfg.file_path()?;   // full file path
cfg.delete_file()?; // delete file, keep dir
cfg.delete_dir()?;  // delete entire dir
```

## Features

| Feature | Default | Description |
|---------|---------|-------------|
| `toml` | ✅ | TOML format support |
| `json` | ❌ | JSON format support |
| `yaml` | ❌ | YAML format support |

## License

Dual-licensed under `MIT OR Apache-2.0`. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
