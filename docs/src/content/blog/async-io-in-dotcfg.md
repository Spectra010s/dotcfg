---
title: 'Introducing Async I/O in dotcfg'
description: 'dotcfg v0.4.0 brings async file I/O to DotCfg and ConfigStack. Learn when one source is enough, when to use layers, and what stays the same.'
publishedAt: 2026-11-10
tags:
  - rust
  - configuration
  - dotcfg
  - async
---

If you've ever loaded a config file from an async command or service, you've had to decide what to do with the file read. Calling a synchronous method works, but the calling thread waits for the disk. That may be fine at startup. It is less appealing when the application reads or updates configuration while it is running.

v0.4.0 introduces optional Tokio-backed async I/O in dotcfg. Both `DotCfg` and `ConfigStack` can now read and write files with `.await`. Which one should you use? That still depends on where your configuration comes from.

## One file? Use DotCfg

If your application has one config file, start with `DotCfg`. Enable the `async` feature and run the methods inside a Tokio runtime:

```toml
[dependencies]
dotcfg = { version = "0.4.0", features = ["async"] }
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use dotcfg::DotCfg;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct AppConfig {
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = DotCfg::new("myapp");

    let config: AppConfig = match cfg.load_async().await? {
        Some(config) => config,
        None => {
            let default = AppConfig { port: 3000 };
            cfg.save_async(&default).await?;
            default
        }
    };

    println!("Listening on port {}", config.port);
    Ok(())
}
```

Here, a missing file is a choice the application makes: create one with a port of `3000`. If you would rather require an existing config, call `load_or_error_async()`. If your type implements `Default`, `load_or_default_async()` can create and save that default for you.

`load_async()` returns `Option<T>` because a missing file is different from an empty or malformed one. An empty file still has to deserialize into `AppConfig`, which requires `port`. A malformed file still returns a parse error. Async changes how the file is read; it does not change what the file means.

The same idea applies to keys. `get_async()` and `set_async()` read and write strings. `get_as_async()` and `set_val_async()` handle typed values, so `set_val_async("port", 8080u16).await?` stores a number rather than the string `"8080"`.

## Several sources? Use ConfigStack

Suppose your app ships defaults, lets each project override them, and accepts environment variables in deployment. That is a [`ConfigStack`](/blog/introducing-configstack): named sources with an order, plus an explicit place for writes. You build it the same way in async code:

```rust
use dotcfg::{ConfigStack, DotCfg};

// Inside an async function returning Result.
let defaults = DotCfg::new("myapp").at_dir("./config/defaults");
let project = DotCfg::new("myapp").at_dir("./config/project");
let cfg = ConfigStack::new()
    .layer("defaults", defaults)
    .layer("project", project)
    .write_to("project")
    .with_env_prefix("APP")
    .build()?;

let port: u16 = cfg.get_as_async("server.port").await?;
cfg.set_val_async("server.port", 8080u16).await?;
```

If the defaults file says `server.port = 3000` and the project file says `8080`, the read returns `8080`. If `APP_SERVER__PORT=9000` is set, it returns `9000`. Nested maps still merge, while an array in a later layer replaces the earlier array. Async reads resolve the same configuration as synchronous reads.

The write goes only to `project`, because that is the source named by `write_to()`. Even if an environment variable wins the read, it is not written to disk. `save_async()` also saves only the value you pass to the chosen source; it does not copy the resolved stack into that file. Leave out `write_to()` and writes return `Error::NoWriteTarget`.

`ConfigStack::load_async()` returns `None` only when every file is missing and there are no matching environment values. Use `load_or_error_async()` when something must be configured. There is no stack `load_or_default_async()` because the stack cannot decide which file should receive a new default. If you know the source, you can call `cfg.source("project")?.load_or_default_async().await?` on its `DotCfg` instead.

## What changes, and what does not

Use the async methods when file I/O happens along an async path. Tokio handles the file operations; parsing, environment overrides, layer precedence, and write ownership keep their existing behavior. You do not need `ConfigStack` just because your application is async, and adding `.await` does not make layer resolution concurrent.

For the method details, see the [async I/O introduction](/async-io) and the guides for [loading and saving](/async-io/loading-saving), [key access](/async-io/key-access), and [ConfigStack](/async-io/config-stack).
