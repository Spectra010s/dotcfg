# Contributing to dotcfg

Thanks for taking the time to contribute to dotcfg.

Contributions can include bug fixes, new features, tests, documentation improvements, examples, and issue reports. For larger changes, opening an issue first is a good way to discuss the approach before investing time in an implementation.

## Development setup

dotcfg requires Rust 1.85 or newer.

Clone the repository and enter the project directory:

```sh
git clone https://github.com/Spectra010s/dotcfg.git
cd dotcfg
```

Run the default test suite:

```sh
cargo test
```

Before submitting a change, also check formatting and Clippy:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

## Feature combinations

TOML is enabled by default. JSON and YAML are optional features. Changes that affect serialization, loading, saving, or key access should be checked against the supported formats.

```sh
cargo test
cargo test --no-default-features --features toml
cargo test --no-default-features --features json
cargo test --no-default-features --features yaml
cargo test --all-features
```

If your change only affects one format, run the relevant feature configuration at minimum.

## Documentation

The documentation site lives in `docs/` and uses Astro.

```sh
cd docs
pnpm install
pnpm run build
```

Keep examples aligned with the actual public API. If you add or change public behavior, update the relevant documentation and README examples as part of the same contribution.

## Pull requests

Keep pull requests focused on one change or closely related set of changes. Avoid unrelated refactors while fixing an issue or adding a feature.

A pull request should:

- explain what changed and why;
- include tests when behavior changes;
- update documentation when the public API or behavior changes;
- pass formatting, Clippy, tests, and relevant CI checks.

## Commits

This repository uses Conventional Commit-style messages. Examples:

```text
feat(discovery): add ancestor search for project configs
fix(keys): preserve nested key values
docs(readme): clarify environment overrides
test(yaml): cover yaml key access
refactor(paths): simplify config path resolution
```

Use a scope on every commit. Examples:

## Reporting bugs

When reporting a bug, include enough information to reproduce it where possible: the dotcfg version, enabled features, Rust version, expected behavior, actual behavior, and a minimal example.

Please do not include secrets or sensitive configuration values in issues, logs, or examples.

## License

By contributing to dotcfg, you agree that your contributions will be licensed under the repository's existing MIT OR Apache-2.0 license terms.
