---
title: 'Introducing ConfigStack: Layered Configuration in dotcfg'
description: 'When one config file is not enough, combine defaults, project settings, and environment overrides while keeping writes in the right place.'
publishedAt: 2026-11-10
tags:
  - rust
  - configuration
  - dotcfg
  - configstack
---

Say you build a CLI with one config file. `DotCfg` handles it: load the file, read a key, save a change. Then you want defaults that ship with the app, settings that belong to each project, and an environment variable that can override a value during deployment. You now have three places to look for the same setting.

Which value should the app use? And if someone changes it, which file should receive the write?

dotcfg v0.4.0 introduces `ConfigStack` to answer those questions. It combines named `DotCfg` sources in a defined order and lets you choose one source for writes.

## A project on top of defaults

Imagine a defaults file containing:

```toml
[server]
host = "127.0.0.1"
port = 3000

[logging]
level = "info"

[features]
enabled = ["search", "exports"]
```

A project needs a different port and a smaller feature list:

```toml
[server]
port = 8080

[features]
enabled = ["search"]
```

You can put those files in a stack, with the project after the defaults:

```rust
use dotcfg::{ConfigStack, DotCfg};

let defaults = DotCfg::new("myapp").at_dir("./config/defaults");
let project = DotCfg::new("myapp").at_dir("./config/project");

let config = ConfigStack::new()
    .layer("defaults", defaults)
    .layer("project", project)
    .write_to("project")
    .with_env_prefix("APP")
    .build()?;

let port: u16 = config.get_as("server.port")?;
```

The read returns `8080`. The project overrides `server.port`, while `server.host` and `logging.level` still come from the defaults. Nested maps merge; the later `features.enabled` array replaces the earlier one, so the resolved list contains only `"search"`.

Now set `APP_SERVER__PORT=9000`. The same read returns `9000`, because matching environment variables sit above the file layers. The double underscore marks a nested key; a single underscore stays inside a key, so `APP_API_KEY` maps to `api_key`. The order tells you which value wins.

## Reading from many places, writing to one

The stack can resolve several sources, but a write needs an owner. In the example, `write_to("project")` names it:

```rust
config.set_val("server.port", 8081u16)?;
```

That changes the project file only. The defaults remain untouched. If `APP_SERVER__PORT` is still set, a new read still sees its value of `9000`; the environment override is not copied into the file. `save()` also writes only the value you pass to the project source, not the merged result. Without `write_to()`, stack writes return `Error::NoWriteTarget`.

Sometimes you want to inspect a file on its own. The source name gives you its `DotCfg` directly:

```rust
let project_port: u16 = config.source("project")?.get_as("server.port")?;
```

That read skips stack resolution, so it sees the project's value even when an environment override wins in the combined view.

## When a source is missing

A project does not have to create a config file before it can use the defaults. Missing files are skipped. `ConfigStack::load()` returns `None` only if every file is missing and no matching environment variable exists. If a file exists but is empty, it still counts as a source and must deserialize into the type you requested. A malformed file is an error, even when another layer has valid values.

If your app has only one config file, keep using `DotCfg`. Use `ConfigStack` when values need to come from several places and you need clear precedence and a deliberate write target. For the full method behavior, see the [ConfigStack guides](/config-stack) and, if your app uses Tokio, the [async ConfigStack guide](/async-io/config-stack).
