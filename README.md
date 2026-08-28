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
- Returns `None` if config doesn't exist — no magic, no forced defaults unless you want them

### Comparison with `confy`

| Feature | `dotcfg` | `confy` |
| :--- | :---: | :---: |
| **Dot-dir support (`~/.app/`)** | ✅ Built-in default | ❌ (XDG/Native only) |
| **XDG directory (`~/.config/app/`)** | ✅ via `.xdg()` | ✅ |
| **Ad-hoc key get/set by string path** | ✅ (`cfg.get("user.name")`) | ❌ (Full struct only) |
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
