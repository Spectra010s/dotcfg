---
title: "Typed Get & Set in dotcfg: When Your Config Must Be a Real Type"
description: "Use get_as and set_val when configuration values need to stay real Rust types instead of making a round trip through strings."
publishedAt: 2026-09-17
tags:
  - rust
  - configuration
  - dotcfg
  - types
version: "0.2.0"
---

`dotcfg` 0.2.0 introduced `get_as` and `set_val`, the typed companions to `get` and `set`.

`get` and `set` are useful when a string is exactly what you want. But configuration often represents values that have real types: a server port is a number, a feature flag is a boolean, a plugin list is a sequence, and a section may map directly to a Rust struct.

That's where the typed API comes in.

## The basic idea

You choose the type you want to store or retrieve:

```rust
let cfg = DotCfg::new("mytool");

cfg.set_val("port", 8080u16)?;
let port: u16 = cfg.get_as("port")?;
```

There is no `parse::<u16>()` step in your application. `get_as` deserializes the value into the type you ask for, while `set_val` serializes the value using its native type.

That distinction matters when the configuration is read elsewhere as structured data.

## Strings and typed values are not the same thing

Consider a port number.

```rust
cfg.set("port", "8080")?;
```

With the string API, TOML stores that value as a string:

```toml
port = "8080"
```

But this Rust structure expects a number:

```rust
struct Config {
    port: u16,
}
```

Trying to deserialize the string value into that field will fail because `"8080"` and `8080` are different TOML values.

With `set_val`, the type survives the write:

```rust
cfg.set_val("port", 8080u16)?;
```

```toml
port = 8080
```

Now a later `load::<Config>()` sees the number it expects.

## More than numbers

The same API works for other serializable values:

```rust
cfg.set_val("features.auto_update", true)?;
let enabled: bool = cfg.get_as("features.auto_update")?;

cfg.set_val(
    "plugins",
    vec!["fmt".to_string(), "lint".to_string()],
)?;
let plugins: Vec<String> = cfg.get_as("plugins")?;
```

It also works with a whole structured value:

```rust
#[derive(Serialize, Deserialize)]
struct Server {
    host: String,
    port: u16,
}

cfg.set_val(
    "server",
    Server {
        host: "localhost".into(),
        port: 8080,
    },
)?;

let server: Server = cfg.get_as("server")?;
```

So you don't have to choose between dotcfg's key-level API and Rust's type system. A single key can itself represent a number, boolean, sequence, or structured value.

## Type mismatches become errors

Typed access also makes the expectation explicit at the call site:

```rust
let port: u16 = cfg.get_as("port")?;
```

If the stored value cannot be deserialized as a `u16`, dotcfg returns an error instead of giving your application a string that it still needs to interpret.

Compare that with:

```rust
let port = cfg.get("port")?;
let port = port.parse::<u16>()?;
```

There are cases where the string API is useful, but when the application already knows what type the value should be, the extra conversion is unnecessary.

## Nested keys still work

`get_as` and `set_val` follow the same key-access model as `get` and `set`, including nested `section.field` paths.

```rust
cfg.set_val("features.auto_update", true)?;
let enabled: bool = cfg.get_as("features.auto_update")?;
```

Updating that value preserves the other keys in the configuration rather than requiring you to reconstruct the whole file yourself.

## When I reach for the typed API

I use `get_as` and `set_val` when the value has meaning beyond its text representation: ports and numeric limits, boolean flags, lists, or grouped settings represented by a struct.

For plain text, `get` and `set` remain the simpler option. For values that must stay typed, the typed pair avoids manual parsing and keeps the representation on disk consistent with what the Rust code expects.

```rust
cfg.set_val("port", 8080u16)?;
let port: u16 = cfg.get_as("port")?;
```

The important part isn't just convenience. It's that a number stays a number, a boolean stays a boolean, and structured configuration stays structured.