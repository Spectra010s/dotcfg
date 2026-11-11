---
title: 'One Value Tree for TOML, JSON and YAML in dotcfg'
description: 'dotcfg v0.4.0 gives TOML, JSON, and YAML one internal value model, so key access and layered configuration can follow the same rules across formats.'
publishedAt: 2026-11-10
tags:
  - rust
  - configuration
  - dotcfg
  - internals
---

Suppose your app reads a TOML file today and a YAML file tomorrow. You still want `server.port` to mean the same thing: find `server`, then read its numeric `port`. That sounds simple until the library itself needs to inspect, change, or merge those values. Each parser gives you its own kind of value, and every new operation has to account for that difference.

dotcfg v0.4.0 brings those formats into one internal value tree. Files still enter and leave as TOML, JSON, or YAML. Between those steps, dotcfg can work with configuration values without carrying three separate representations through every operation.

## Three files, one shape

These files say the same thing in different syntax.

TOML:

```toml
[server]
host = "localhost"
port = 3000
```

JSON:

```json
{
  "server": {
    "host": "localhost",
    "port": 3000
  }
}
```

YAML:

```yaml
server:
  host: localhost
  port: 3000
```

After parsing, each can be understood as the same tree:

```text
Map
└── server: Map
    ├── host: String("localhost")
    └── port: Number(3000)
```

Now a lookup such as `cfg.get_as::<u16>("server.port")` can follow the same path through that tree, regardless of the file format. A typed update with `set_val("server.port", 8080u16)` replaces a number with a number. When dotcfg writes the result, it renders the tree in that source's format.

This boundary matters because key access is only one use of the data. A full `load()` needs to deserialize a Rust type from it and may apply environment overrides. Once several files contribute settings, their values need to be combined.

## What it means for ConfigStack

[`ConfigStack`](/blog/introducing-configstack) can put a TOML defaults file below a YAML project file, for example. It does not need one merge algorithm for TOML and another for YAML. Both files have already become value trees before the merge begins.

If both layers contain a `server` map, their entries merge recursively. A later `server.port` replaces the earlier port without discarding `server.host`. An array in a later layer replaces the earlier array. The same rules apply if either file is JSON. Environment values, when enabled, have the highest precedence.

That is the practical payoff of the shared representation: you can choose a file format for each source without changing how keys are read or how layers resolve.

## Formats still have their own rules

The tree does not make TOML, JSON, and YAML identical. Each still has its own syntax and values it can represent. TOML, for instance, has no `null` value, while JSON does. Parsing errors still come from the source format, and a file is written using the format selected for that source.

The shared tree gives dotcfg one place to handle configuration operations. The format-specific work stays at the edges, where files are parsed and rendered. That is what lets a typed key read, a one-key update, and a layered merge behave consistently without making you choose a different configuration API for each format.

See the [formats guide](/formats) for selecting TOML, JSON, or YAML, and the [ConfigStack guide](/config-stack) for layered reads and writes.
