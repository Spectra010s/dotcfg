---
title: 'One Value Tree for TOML, JSON and YAML in dotcfg'
description: 'Why dotcfg now uses one internal representation for configuration values across file formats.'
publishedAt: 2026-10-16
tags:
  - rust
  - configuration
  - dotcfg
  - internals
---

dotcfg currently supports TOML, JSON and YAML.

That sounds simple enough: detect the format, parse the file, and expose the configuration through the same API.

But supporting multiple formats creates a less obvious problem.

What should configuration look like *inside* dotcfg?

For a while, the answer depended too much on where the value came from. TOML values were TOML values, JSON values were JSON values, and YAML had its own representation.

That works when the job is mostly:

```text
file → deserialize → Rust type
```

It becomes much harder once the library itself needs to work with configuration values.

Things like:

- reading a nested key with `get()`
- changing one value with `set()`
- converting a value with `get_as()`
- applying environment overrides
- merging configuration sources
- supporting different file formats consistently

all need some shared idea of what a configuration value actually is.

So dotcfg now has one.

## The problem with format-specific values

Consider the same configuration written in three of the formats dotcfg supports today.

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

To the application, these represent the same thing.

There is a `server` map containing a string called `host` and a number called `port`.

Internally, however, each parser has its own value representation.

If dotcfg lets those representations leak throughout its implementation, every operation eventually becomes format-aware.

Getting `server.port` needs format-specific logic.

Setting it needs the same.

Merging values needs the same again.

Every feature that operates on configuration has to answer the format question before it can answer the configuration question.

That's the wrong level for most of dotcfg to be working at.

## A shared configuration value

Instead, dotcfg converts parsed configuration into a common internal value representation.

Conceptually, a configuration value can be something like:

```text
null
boolean
number
string
array
map
```

The exact syntax used in the original file is no longer important to operations that don't need to know about it.

The TOML:

```toml
[server]
port = 3000
```

and JSON:

```json
{
  "server": {
    "port": 3000
  }
}
```

can both become the same kind of tree internally:

```text
Map
└── server
    └── Map
        └── port
            └── Number(3000)
```

Now `server.port` means the same thing regardless of which supported format produced it.

## Parse first, operate second

This gives dotcfg a cleaner boundary.

At the edge of the library, formats still matter.

A TOML file must be parsed as TOML. JSON must be parsed as JSON. YAML must be parsed as YAML. Any format added in the future will have its own parsing rules too.

But after parsing, the value can enter the common representation.

Roughly:

```text
TOML ─┐
JSON ─┤
YAML ─┼─→ internal value tree → dotcfg operations
...  ─┘
```

When writing configuration, the process goes in the other direction:

```text
dotcfg operations → internal value tree → target format
```

That means most configuration logic can work on configuration itself instead of on a particular serialization format.

It also means adding another format doesn't have to mean teaching every part of dotcfg how that format represents values.

## Why this matters for `get()` and `set()`

Take a nested lookup:

```rust
cfg.get("server.port")?
```

Without a shared representation, traversing that path can depend on the underlying format.

With a common tree, the operation is much simpler conceptually:

```text
root
 ↓
server
 ↓
port
```

The same traversal works regardless of which supported format the configuration came from.

The same applies to `set()`.

Changing:

```text
server.port
```

means finding the `server` map and updating its `port` entry.

The format becomes important again when the resulting configuration needs to be serialized, not while dotcfg is deciding what `server.port` means.

## It also gives layering something solid to operate on

This became especially important as dotcfg gained [`ConfigStack`](/blog/introducing-configstack).

Layering requires values from different configuration sources to be combined.

Maps need to deep-merge.

Later values need to override earlier values.

Arrays need to replace earlier arrays.

Doing that cleanly while every source might still be represented by a different format-specific value type would make the layering implementation unnecessarily dependent on the formats involved.

With a shared value tree, the merge rule can operate on the values themselves:

```text
Map + Map
    → recursively merge

anything else + later value
    → later value wins
```

`ConfigStack` doesn't need a separate merge algorithm for every supported format.

It needs one configuration merge algorithm.

## Same API, more consistent semantics

A common internal representation also gives dotcfg one place to define behavior that should be consistent across formats.

If a key is missing, that should mean the same thing regardless of the file format.

If a value exists but cannot be converted to the requested Rust type, that distinction should not depend on the parser that happened to create it.

If environment variables override a value, they should enter the same configuration model as values loaded from files.

The public API can therefore describe configuration semantics rather than format-specific implementation details.

That is important because supporting multiple formats shouldn't mean exposing several subtly different configuration libraries behind one Rust API.

## Formats still have differences

A shared value representation doesn't mean every configuration format suddenly becomes identical.

They aren't.

Each format has its own syntax and its own set of values it can naturally represent. Some values may need conversion when moving into the common model, and some distinctions from a source format may not have an equivalent everywhere else.

The shared tree isn't meant to erase those differences.

Its job is to give dotcfg a predictable representation for the values the library needs to manipulate consistently.

Parsing and serialization remain format-specific.

Configuration operations don't have to be.

## More than an internal refactor

This could look like implementation cleanup.

In practice, it changes what becomes reasonable to build on top of dotcfg.

`ConfigStack` is one example.

Environment overrides are another.

Future features that need to inspect, transform or combine configuration can operate against one representation instead of growing separate implementations for every format.

The same applies to future format support. A new format still needs parsing and serialization, but the rest of dotcfg doesn't need an entirely new implementation of configuration operations just because another format was added.

And fixes to those operations can happen in one place.

The important part isn't that dotcfg now has another internal type.

It's that the rest of the library has a common language for configuration.

**File formats are how configuration enters and leaves dotcfg. Inside, it's just configuration.**
