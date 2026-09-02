# dotcfg

[![CI](https://github.com/Spectra010s/dotcfg/actions/workflows/ci.yml/badge.svg)](https://github.com/Spectra010s/dotcfg/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/dotcfg)](https://crates.io/crates/dotcfg)
[![docs.rs](https://img.shields.io/docsrs/dotcfg)](https://docs.rs/dotcfg)

Flexible config management for Rust applications.

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

You can get a value from the config as a specific type you choose — the methods are `get_as` and `set_val` (like `get`/`set` but typed).

Use when this config must be a real type. For example, you want `port` to always be a positive number:

```rust
// “port must be a number” — store and get it as u16 directly
cfg.set_val("port", 8080u16)?;
let port: u16 = cfg.get_as("port")?; // no parse::<u16>(), type-safe

// same for other types — bool, Vec, or a whole struct
cfg.set_val("features.auto_update", true)?;
let enabled: bool = cfg.get_as("features.auto_update")?;
cfg.set_val("plugins", vec!["fmt".to_string(), "lint".to_string()])?;
let plugins: Vec<String> = cfg.get_as("plugins")?;
```

`get_as` infers the type you ask for (`u16`, `bool`, `Vec<String>`, or a whole `struct` like `Server { host, port }`), so you don’t parse strings yourself. A mismatch (e.g., `get_as::<u16>` on `"alice"`) returns a clear error.

These work like `get`/`set`: `set_val` creates the file/dir if missing, handles `section.field`, and preserves other keys.

## Environment Variable Overrides

You can let environment variables override the config file — the method is `with_env_prefix`.

Use when you want `MYAPP_PORT=9000` to take precedence over `port` in the file without changing the file:

```rust
let cfg = DotCfg::new("mytool").with_env_prefix("MYAPP");

// MYAPP_PORT if set, otherwise `port` from the file
let port: u16 = cfg.get_as("port")?;
// MYAPP_USER_NAME for `user.name`, MYAPP_PLUGINS=fmt,lint for `Vec<String>`
```

`get` returns the raw env string, `get_as` parses it (`bool` as `true`/`false`/`1`/`0`, numbers, `Vec` as comma-separated). `set`/`set_val` still write only to the file.

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
