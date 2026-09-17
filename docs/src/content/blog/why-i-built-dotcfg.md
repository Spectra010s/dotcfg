---
title: "Why I Built dotcfg: A Flexible Config Manager for Rust Apps"
description: "The configuration problems that pushed me to build dotcfg, from non-destructive loading to key-level access and flexible directory strategies."
publishedAt: 2026-09-17
tags:
  - rust
  - configuration
  - dotcfg
version: "0.3.0"
---

When building command-line tools or desktop applications in Rust, one of the first questions you face is: **where and how should I store user configuration?**

Do you use `~/.mytool/config.toml`? Do you follow XDG conventions? What happens if the config file doesn't exist yet — should your application silently create one, or should you be able to tell that the user is running it for the first time? And what if your CLI only needs to change a single setting without defining and parsing a large config struct?

Those were the kinds of problems that led me to build **dotcfg**: a lightweight configuration manager for Rust applications that gives the developer control over how configuration is stored and accessed.

## Reading shouldn't have to write

One behaviour I wanted to avoid was a read unexpectedly creating configuration on disk.

For a CLI, that distinction matters. You might want to detect a first-time run, inspect whether configuration exists, or simply read settings without changing the user's filesystem.

With dotcfg, `load()` is non-destructive when the file doesn't exist:

```rust
use dotcfg::DotCfg;

let cfg: Option<AppConfig> = DotCfg::new("mytool").load()?;

match cfg {
    Some(config) => println!("Welcome back, {}!", config.username),
    None => println!("First time running mytool? Let's get you set up!"),
}
```

If creating defaults is what you want, you can make that choice explicitly:

```rust
let cfg: AppConfig = DotCfg::new("mytool").load_or_default()?;
```

Or require the file to already exist:

```rust
let cfg: AppConfig = DotCfg::new("mytool").load_or_error()?;
```

That makes the filesystem behaviour visible in the API instead of hiding it behind a normal read.

## Sometimes I only want one key

Another frustration was having to treat every configuration change as a full-struct operation.

Imagine a CLI command that only needs to update an auth token or username. With a struct-only workflow, you typically deserialize the complete config, mutate one field, and serialize it again.

For small changes, I wanted something more direct.

```rust
let cfg = DotCfg::new("mytool");

let username = cfg.get("username")?;
cfg.set("username", "tayo")?;
cfg.set("user.email", "tayo@example.com")?;
```

`get` and `set` make it possible to work with individual values, including nested `section.field` keys, while leaving the rest of the configuration intact.

And when the value needs to remain a real Rust type rather than a string, dotcfg also provides `get_as` and `set_val`.

## Configuration shouldn't be tied to one directory strategy

There isn't one directory convention that fits every Rust application.

For a traditional CLI, a dot-directory can make sense:

```rust
let cfg = DotCfg::new("mytool");
// ~/.mytool/config.toml
```

For applications that want the platform's XDG configuration strategy:

```rust
let cfg = DotCfg::new("mytool").xdg();
```

You can also choose a different filename:

```rust
let cfg = DotCfg::new("mytool").filename("settings");
```

### Custom directories in 0.3.0

With dotcfg 0.3.0, that flexibility extends to exact directories:

```rust
let cfg = DotCfg::new("mytool")
    .at_dir("/my/project/.config");
```

`at_dir()` uses the directory you provide directly instead of applying the dot-directory or XDG strategy. That makes project-local configuration, tests, and portable setups much easier to represent.

## Finding configuration from inside a project

Custom directories solve the case where you already know the path. CLI tools often have another problem: the command may be running several directories below the project root.

In 0.3.0, dotcfg can search through the current directory and its ancestors for the application's dot-directory:

```rust
let cfg = DotCfg::new("mytool");

if let Some(project_cfg) = cfg.find_in_ancestors()? {
    // use the discovered project configuration
}
```

You can also start the search from a specific path with `find_in_ancestors_from()`.

If nothing is found, discovery returns `Ok(None)` rather than silently falling back to another location. The caller gets to decide what happens next.

## A small CLI example

The full-struct and key-level APIs can live together. You can load a complete configuration when that is convenient and still update an individual setting later.

```rust
use dotcfg::DotCfg;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
struct ServerConfig {
    host: String,
    port: u16,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = DotCfg::new("serverapp");

    let config: ServerConfig = cfg.load_or_default()?;
    println!("Running on {}:{}", config.host, config.port);

    cfg.set_val("port", 9000u16)?;

    Ok(())
}
```

I built dotcfg around that idea: configuration APIs should give you useful defaults without taking control away from you.

You can load a whole struct or work with one key. You can use a traditional dot-directory, XDG, an exact directory, or discover project configuration. And you can enable the formats your application actually needs — TOML, JSON, YAML, or more than one of them.

If that sounds useful for something you're building, check out **dotcfg on GitHub** or the rest of the documentation.