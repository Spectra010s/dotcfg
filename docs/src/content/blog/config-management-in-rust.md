---
title: "Config Management in Rust: From Zero-Boilerplate to Full Control"
description: "A look at the architectural trade-off between global configuration choices and per-instance control, and why dotcfg takes the latter approach."
publishedAt: 2026-09-17
tags:
  - rust
  - configuration
  - dotcfg
  - architecture
version: "0.3.0"
---

In the Rust ecosystem, configuration crates often make a fundamental architectural trade-off: **zero-boilerplate simplicity** vs. **runtime flexibility**.

Understanding how those crates are designed under the hood helps explain why some choices are global while others can vary for each configuration file — and why that distinction starts to matter as an application grows.

## The global format constraint

Take `confy` as an example. It supports TOML, YAML, RON, and an alternative TOML implementation through Cargo features, but its current design requires **exactly one configuration-language feature** to be enabled for a build.

At the implementation level, the selected feature controls module-level serialization code and the file extension:

```rust
#[cfg(feature = "toml_conf")]
const EXTENSION: &str = "toml";

#[cfg(feature = "yaml_conf")]
const EXTENSION: &str = "yml";
```

Because the format-specific definitions live at module scope, enabling conflicting configuration-language features is rejected at compile time. This is the kind of error you see:

```text
error[E0428]: the name `EXTENSION` is defined multiple times
   --> src/lib.rs:157:1
    |
154 | const EXTENSION: &str = "toml";
    | ------------------------------- previous definition of the value `EXTENSION` here
...
157 | const EXTENSION: &str = "yml";
    | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `EXTENSION` redefined here

error: Exactly one config language feature must be enabled to compile confy.
       Please disable one of either the `toml_conf`, `yaml_conf`, or `ron_conf` features.
```

That isn't inherently a bad design. It fits `confy`'s goal: choose the format for your application's configuration at compile time, then let the crate handle the details.

But it creates a boundary worth understanding. The format choice belongs to the crate configuration for the whole binary, not to an individual configuration handle.

## Why that can matter in a real application

An application can easily end up touching several structured formats at once:

1. **User settings (`config.toml`)** — readable configuration intended to be edited by a person.
2. **OAuth credentials (`credentials.json`)** — structured data that may come from an API.
3. **Export or CI templates (`manifest.yaml`)** — configuration exchanged with other tools.

If the configuration abstraction itself is compiled around one format, the other files need to be handled through another serialization path.

For a small tool, that may be completely fine. For a tool that wants one consistent configuration API across those files, per-instance format selection becomes useful.

## The instance-based alternative in dotcfg

`dotcfg` takes that second approach. Formats are represented on each `DotCfg` handle:

```rust
pub enum Format {
    Toml,
    #[cfg(feature = "json")]
    Json,
    #[cfg(feature = "yaml")]
    Yaml,
}
```

Cargo features decide which formats are available in the binary, while each configuration instance decides which enabled format it actually uses.

With dotcfg `0.3.0`, an application that needs all three can enable JSON and YAML alongside the default TOML support:

```toml
[dependencies]
dotcfg = { version = "0.3", features = ["json", "yaml"] }
```

Then each file can choose its own format:

```rust
use dotcfg::DotCfg;

// Human-editable application settings
let main_cfg = DotCfg::new("myapp").toml();
main_cfg.save(&user_preferences)?;

// Machine-generated credentials
let auth_cfg = DotCfg::new("myapp")
    .json()
    .filename("credentials");
auth_cfg.save(&oauth_response)?;

// Export template
let export_cfg = DotCfg::new("myapp")
    .yaml()
    .filename("manifest");
export_cfg.save(&template_data)?;
```

All three handles can exist in the same process. The format is local to the handle instead of being one global format choice for the application.

## The same distinction applies to directories

Format selection isn't the only place where global and per-instance configuration differ.

`confy` 2.0 exposes an application-wide configuration strategy through `change_config_strategy`, switching between its App and Native strategies. It also provides path-based APIs when you already have a specific path.

`dotcfg` keeps its directory choice on each handle. The default uses the application's dot-directory, while `.xdg()` selects the XDG strategy for that instance:

```rust
let local = DotCfg::new("mytool");
let xdg = DotCfg::new("mytool").xdg();
```

And in `0.3.0`, a handle can point directly at a custom directory:

```rust
let project = DotCfg::new("mytool")
    .at_dir("/my/project/.config");
```

For project-aware CLI tools, dotcfg can also discover a configuration by walking through ancestor directories:

```rust
if let Some(project_cfg) = DotCfg::new("mytool").find_in_ancestors()? {
    // use the project-local configuration
}
```

The important architectural point is the same: these choices belong to a particular configuration handle rather than changing shared process-wide state.

## Loading behaviour is another deliberate trade-off

`confy::load` requires the configuration type to implement `Default`. If the file doesn't exist, `confy` creates it using `T::default()` and returns that value.

That is convenient when the application always wants configuration to exist.

`dotcfg` makes the non-destructive case its normal `load()` behaviour:

```rust
let config: Option<AppConfig> = DotCfg::new("mytool").load()?;
```

A missing file produces `Ok(None)`. If automatic creation is what the application wants, that behaviour is explicit:

```rust
let config: AppConfig = DotCfg::new("mytool").load_or_default()?;
```

Or the application can require an existing file:

```rust
let config: AppConfig = DotCfg::new("mytool").load_or_error()?;
```

Neither model is universally better. They optimize for different assumptions about what a missing configuration file means.

## Full configuration and individual keys

The same flexibility extends to how configuration is accessed.

A complete structure can be loaded or saved:

```rust
let config: Option<AppConfig> = cfg.load()?;
cfg.save(&config)?;
```

But dotcfg can also work with individual keys:

```rust
let username = cfg.get("user.name")?;
cfg.set("user.name", "tayo")?;
```

And typed key access is available when a value should remain a native number, boolean, sequence, or structure:

```rust
cfg.set_val("server.port", 8080u16)?;
let port: u16 = cfg.get_as("server.port")?;
```

That lets a CLI update one setting without forcing every command into a full-struct workflow.

## The trade-off

Zero-boilerplate configuration is valuable precisely because it makes decisions for you. If an application has one format, one configuration strategy, and always wants a default configuration to exist, that can be a very good fit.

`dotcfg` is aimed at the point where those decisions need to vary: different formats in one process, different locations for different configuration handles, non-destructive reads, project-local discovery, and both full-struct and key-level access.

The goal isn't to remove conventions. It's to make the convention a default rather than a constraint.