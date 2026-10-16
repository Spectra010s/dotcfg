---
title: 'Introducing ConfigStack: Layered Configuration in dotcfg'
description: 'Layer multiple configuration sources with predictable precedence, deep merging, explicit writes, and environment overrides.'
publishedAt: 2026-10-16
tags:
  - rust
  - configuration
  - dotcfg
  - configstack
---

A single configuration file is enough for a lot of applications.

Sometimes it isn't.

An application might ship with defaults, have configuration specific to a project or machine, and use environment variables for deployment-specific values. Once configuration starts coming from more than one place, loading a file is no longer the difficult part.

The difficult part is deciding how those sources work together.

dotcfg now has `ConfigStack` for exactly that.

## From one source to many

`DotCfg` represents a single configuration source.

For example:

```rust
let cfg = DotCfg::new("myapp");
let config: AppConfig = cfg.load_or_default()?;
```

That remains the simplest way to use dotcfg, and it isn't going away.

`ConfigStack` is for applications that need multiple configuration sources with predictable precedence between them.

A stack can look roughly like this:

```rust
let config = ConfigStack::new()
    .layer("defaults", defaults)
    .layer("project", project)
    .write_to("project")
    .with_env_prefix("APP")
    .build()?;
```

Each layer has a name and a position in the stack.

Later layers override earlier ones, and environment variables sit above the configured layers when an environment prefix is enabled.

Conceptually:

```text
defaults
    ↓
project
    ↓
environment
```

The resolved configuration is the result of applying those sources in order.

## Later layers win

Suppose the first layer contains:

```toml
[server]
host = "localhost"
port = 3000

[logging]
level = "info"
```

and a later layer contains:

```toml
[server]
port = 8080
```

The resolved configuration becomes:

```toml
[server]
host = "localhost"
port = 8080

[logging]
level = "info"
```

`server.port` comes from the later layer, while the other values remain available from the earlier one.

This is where layering becomes slightly more interesting than simply loading several files in sequence.

## Maps deep-merge, arrays replace

When two layers contain maps, `ConfigStack` merges them recursively.

Given:

```toml
# defaults

[server]
host = "localhost"
port = 3000

[server.tls]
enabled = false
```

and:

```toml
# project

[server]
port = 8080

[server.tls]
enabled = true
```

the later values override only the corresponding values in the earlier map.

Arrays deliberately behave differently.

If a later layer contains an array at the same path as an earlier layer, the later array replaces it.

`ConfigStack` doesn't try to guess whether arrays should be appended, deduplicated, merged by position, or combined using some application-specific rule.

The rule is intentionally predictable:

**Maps deep-merge. Arrays replace. Later values win.**

## Reading the resolved configuration

Once built, a stack can be read much like the rest of dotcfg.

Individual values can be accessed with `get()`:

```rust
let port = config.get("server.port")?;
```

Values can be converted to a requested type with `get_as()`:

```rust
let port: u16 = config.get_as("server.port")?;
```

And the resolved configuration can be deserialized into a Rust type with `load()`:

```rust
let config: AppConfig = config.load()?;
```

The caller doesn't need to manually inspect every layer to determine where each value came from.

Resolution happens across the stack first.

## Writing is explicit

Reading from multiple sources raises an important question:

**Where should a write go?**

Consider a stack containing `defaults` and `project`.

If you run:

```rust
config.set("server.port", 9000)?;
```

dotcfg shouldn't silently guess whether that belongs in the defaults or the project configuration.

A stack therefore needs an explicit write target before resolved writes are allowed:

```rust
let config = ConfigStack::new()
    .layer("defaults", defaults)
    .layer("project", project)
    .write_to("project")
    .build()?;
```

Now writes have a clear owner.

If no write target is configured, the stack remains read-only.

This keeps two separate ideas separate:

- values may be **read from many sources**
- changes are **written to one explicitly selected source**

## Layers are still individually accessible

Using a stack doesn't hide the underlying configuration sources.

A named source can be accessed directly:

```rust
config.source("project")
```

This is useful when the application intentionally wants to work with one layer rather than the fully resolved configuration.

The stack provides a combined view, but each source still has its own identity.

Source names also make configuration mistakes explicit. Duplicate names, references to unknown sources, and attempts to perform resolved writes without a write target are reported as errors instead of being silently resolved.

## Environment variables belong at the top

Environment overrides can be added with a prefix:

```rust
let config = ConfigStack::new()
    .layer("defaults", defaults)
    .layer("project", project)
    .with_env_prefix("APP")
    .build()?;
```

Environment values have the highest precedence.

There is also one important naming rule.

A single underscore is part of a configuration key, while a double underscore represents nesting:

```text
APP_PORT              → port
APP_API_KEY           → api_key
APP_DATABASE__PORT    → database.port
APP_SERVER__TLS__MODE → server.tls.mode
```

This distinction matters because an environment variable such as:

```text
APP_DATABASE_PORT
```

cannot otherwise tell us whether the intended key is `database_port` or `database.port`.

With the current convention, it unambiguously means:

```text
database_port
```

while:

```text
APP_DATABASE__PORT
```

means:

```text
database.port
```

The same convention is used by both `DotCfg` and `ConfigStack`.

## What about missing or empty layers?

Not every source in a stack needs to contain configuration.

A missing optional source doesn't need to make the entire stack unusable, and an empty configuration is different from malformed configuration.

That distinction matters when configuration comes from multiple places.

A source that contributes no values simply has nothing to add to the resolved configuration.

A malformed source, however, is still an error.

Layering shouldn't turn invalid configuration into something that is silently ignored.

## Why ConfigStack is separate from DotCfg

`ConfigStack` could have been folded into `DotCfg`, but doing that would make the basic API responsible for concepts that many applications don't need.

If an application has one configuration source, this is still enough:

```rust
let cfg = DotCfg::new("myapp");
```

There is no need to create a stack just to read one file.

`DotCfg` remains the single-source configuration API.

`ConfigStack` composes multiple sources when the application needs precedence, merging and explicit write ownership.

That keeps the simple case simple without limiting applications that eventually need more.

## A foundation for more flexible configuration

Layered configuration changes what dotcfg can represent.

A CLI can combine defaults with project-specific configuration.

A desktop application can keep application defaults separate from user-controlled values.

A server can resolve file-backed configuration and then let deployment environment variables override specific values.

And applications that only need one configuration file can continue using `DotCfg` exactly for that job.

`ConfigStack` isn't intended to replace `DotCfg`.

It fills the space that appears when configuration stops coming from one place.
