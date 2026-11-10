//! Layered configuration: compose several [`DotCfg`] sources into one resolved view.
//!
//! [`ConfigStack`] keeps [`DotCfg`] as one file-backed source and layers
//! several of them under stable names. Later layers override earlier ones,
//! environment overrides sit on top, nested maps deep-merge, and writes go
//! only to one explicit target — the resolved view is never flattened back
//! into a file.
//!
//! ```rust,no_run
//! use dotcfg::{ConfigStack, DotCfg};
//!
//! let defaults = DotCfg::new("myapp").at_dir("/etc/myapp");
//! let project = DotCfg::new("myapp").at_dir("./.myapp");
//!
//! let cfg = ConfigStack::new()
//!     .layer("defaults", defaults)
//!     .layer("project", project)
//!     .write_to("project")
//!     .with_env_prefix("APP")
//!     .build()?;
//!
//! let port: u16 = cfg.get_as("port")?;
//! # Ok::<(), dotcfg::Error>(())
//! ```
//!
//! Precedence, low to high: earlier layers, later layers, environment.
//! Missing files are skipped, empty files contribute nothing, malformed
//! files are an error — the same rules as single-file reads.

use std::collections::{BTreeSet, VecDeque};
use std::ffi::OsString;

use serde::de::{DeserializeOwned, DeserializeSeed, EnumAccess, MapAccess, VariantAccess, Visitor};
use serde::{Deserializer, Serialize};

use crate::env::EnvStr;
use crate::value::Value;
use crate::{DotCfg, Error, env, env_override, value};

/// Layered configuration over named [`DotCfg`] sources.
///
/// Built with the builder-style methods and validated by [`Self::build`]:
/// duplicate layer names and unknown `write_to` targets are errors, and a
/// stack without `write_to` is read-only (`set`/`set_val`/`save` report
/// [`Error::NoWriteTarget`] rather than guessing a target).
///
/// Reads resolve low → high precedence with environment on top; see the
/// module-level docs. Every method takes `&self`: resolution re-reads
/// the sources on each call, so concurrent edits on disk are picked up.
pub struct ConfigStack {
    layers: Vec<(String, DotCfg)>,
    write_to: Option<String>,
    env_prefix: Option<String>,
}

impl ConfigStack {
    /// Start an empty stack. Add sources with [`Self::layer`], then validate
    /// with [`Self::build`].
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            write_to: None,
            env_prefix: None,
        }
    }

    /// Add a source under `name`. Later layers override earlier ones.
    pub fn layer(mut self, name: impl Into<String>, source: DotCfg) -> Self {
        self.layers.push((name.into(), source));
        self
    }

    /// Route stack-level writes (`set`, `set_val`, `save`) to `name`.
    /// Without it the stack is read-only.
    pub fn write_to(mut self, name: impl Into<String>) -> Self {
        self.write_to = Some(name.into());
        self
    }

    /// Let `PREFIX_*` variables override the resolved configuration, with
    /// the highest read precedence. Source-level prefixes still apply on
    /// direct [`Self::source`] access; resolved reads use only this one.
    pub fn with_env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.env_prefix = Some(prefix.into());
        self
    }

    /// Validate duplicate names and the `write_to` target.
    pub fn build(self) -> Result<Self, Error> {
        let mut seen = std::collections::HashSet::new();
        for (name, _) in &self.layers {
            if !seen.insert(name) {
                return Err(Error::DuplicateSource(name.clone()));
            }
        }
        if let Some(target) = &self.write_to {
            if !self.layers.iter().any(|(name, _)| name == target) {
                return Err(Error::UnknownSource(target.clone()));
            }
        }
        Ok(self)
    }

    /// Merged file trees, low → high precedence. Missing files are skipped;
    /// malformed files are an error. Also reports whether any source file
    /// exists at all.
    fn merged_tree(&self) -> Result<(Value, bool), Error> {
        let mut tree = value::empty();
        let mut any_file = false;
        for (_, cfg) in &self.layers {
            if let Some(file_tree) = cfg.read_tree()? {
                any_file = true;
                value::merge(&mut tree, file_tree);
            }
        }
        Ok((tree, any_file))
    }

    /// Merged file trees, low → high precedence, read with Tokio.
    #[cfg(feature = "async")]
    async fn merged_tree_async(&self) -> Result<(Value, bool), Error> {
        let mut tree = value::empty();
        let mut any_file = false;
        for (_, cfg) in &self.layers {
            if let Some(file_tree) = cfg.read_tree_async().await? {
                any_file = true;
                value::merge(&mut tree, file_tree);
            }
        }
        Ok((tree, any_file))
    }

    /// `(path segments, var name, raw value)` for every `PREFIX_*` variable.
    ///
    /// `__` splits nesting, a single `_` stays inside a key, empty segments
    /// are dropped, everything lowercased: `APP_PORT` → `port`,
    /// `APP_API_KEY` → `api_key`, `APP_DATABASE__PORT` → `database.port`.
    /// Non-unicode names are skipped (they cannot match a text prefix);
    /// non-unicode values stay staged and fail lazily with
    /// [`Error::EnvNotUnicode`] when read, exactly like direct lookups.
    fn discover(&self) -> Vec<(Vec<String>, String, OsString)> {
        discover_env(self.env_prefix.as_deref())
    }

    /// Merged tree plus discovered env entries, and whether anything at all
    /// was found (any file, or any matching env var).
    fn resolve(&self) -> Result<(Overlay, bool), Error> {
        let (tree, any_file) = self.merged_tree()?;
        let env = self.discover();
        let any_source = any_file || !env.is_empty();
        Ok((Overlay { tree, env }, any_source))
    }

    /// Async counterpart of [`Self::resolve`], using Tokio for file reads.
    #[cfg(feature = "async")]
    async fn resolve_async(&self) -> Result<(Overlay, bool), Error> {
        let (tree, any_file) = self.merged_tree_async().await?;
        let env = self.discover();
        let any_source = any_file || !env.is_empty();
        Ok((Overlay { tree, env }, any_source))
    }

    /// Read one value as a string.
    ///
    /// A matching environment variable wins with its raw value, using the
    /// stack `__` convention (`database.port` reads `PREFIX_DATABASE__PORT`).
    /// Otherwise the merged tree renders with the format-independent
    /// canonical form (scalars raw, arrays and maps compact JSON), never in
    /// a supplier's format.
    pub fn get(&self, key: &str) -> Result<String, Error> {
        if let Some((_, raw)) = env_override(self.env_prefix.as_deref(), key)? {
            return Ok(raw);
        }
        let (tree, any_file) = self.merged_tree()?;
        if !any_file {
            return Err(Error::NotFound);
        }
        value::get_node(&tree, key).map(value::display_canonical)
    }

    /// Read one value deserialized into `T`.
    ///
    /// Environment wins via the same typed rules as [`DotCfg::get_as`],
    /// looked up with the stack `__` convention; otherwise the merged-tree
    /// node deserializes directly.
    pub fn get_as<T: DeserializeOwned>(&self, key: &str) -> Result<T, Error> {
        if let Some((var, raw)) = env_override(self.env_prefix.as_deref(), key)? {
            return env::from_env_str(&var, &raw);
        }
        let (tree, any_file) = self.merged_tree()?;
        if !any_file {
            return Err(Error::NotFound);
        }
        value::from_value(value::get_node(&tree, key)?.clone())
    }

    /// Read one value as a string with Tokio filesystem I/O.
    ///
    /// Behaves exactly like [`Self::get`]: a matching environment variable
    /// wins, and the merged tree renders in the canonical form.
    #[cfg(feature = "async")]
    pub async fn get_async(&self, key: &str) -> Result<String, Error> {
        if let Some((_, raw)) = env_override(self.env_prefix.as_deref(), key)? {
            return Ok(raw);
        }
        let (tree, any_file) = self.merged_tree_async().await?;
        if !any_file {
            return Err(Error::NotFound);
        }
        value::get_node(&tree, key).map(value::display_canonical)
    }

    /// Read one value deserialized into `T` with Tokio filesystem I/O.
    ///
    /// Behaves exactly like [`Self::get_as`], including typed environment
    /// parsing and the `__` lookup convention.
    #[cfg(feature = "async")]
    pub async fn get_as_async<T: DeserializeOwned>(&self, key: &str) -> Result<T, Error> {
        if let Some((var, raw)) = env_override(self.env_prefix.as_deref(), key)? {
            return env::from_env_str(&var, &raw);
        }
        let (tree, any_file) = self.merged_tree_async().await?;
        if !any_file {
            return Err(Error::NotFound);
        }
        value::from_value(value::get_node(&tree, key)?.clone())
    }

    /// Load the resolved configuration into `T`.
    ///
    /// File trees merge low → high, then environment applies on top with one
    /// shared definition of "resolved": `None` only when no source file
    /// exists and no matching env var is set. Env-only keys participate via
    /// the `__` discovery rule (`APP_X__Y` → `x.y`).
    pub fn load<T: DeserializeOwned>(&self) -> Result<Option<T>, Error> {
        let (overlay, any_source) = self.resolve()?;
        if !any_source {
            return Ok(None);
        }
        load_overlay(overlay.tree, overlay.env).map(Some)
    }

    /// Load the resolved configuration, or [`Error::NotFound`] when nothing
    /// is configured anywhere.
    pub fn load_or_error<T: DeserializeOwned>(&self) -> Result<T, Error> {
        self.load()?.ok_or(Error::NotFound)
    }

    /// Load the resolved configuration with Tokio filesystem I/O.
    ///
    /// Uses the same merge and environment rules as [`Self::load`], so the
    /// async and sync paths return the same values for the same files.
    #[cfg(feature = "async")]
    pub async fn load_async<T: DeserializeOwned>(&self) -> Result<Option<T>, Error> {
        let (overlay, any_source) = self.resolve_async().await?;
        if !any_source {
            return Ok(None);
        }
        load_overlay(overlay.tree, overlay.env).map(Some)
    }

    /// Async counterpart of [`Self::load_or_error`].
    #[cfg(feature = "async")]
    pub async fn load_or_error_async<T: DeserializeOwned>(&self) -> Result<T, Error> {
        self.load_async().await?.ok_or(Error::NotFound)
    }

    /// The writable source, or [`Error::NoWriteTarget`] when the stack is
    /// read-only. Never guessed: only the configured target qualifies.
    fn writer(&self) -> Result<&DotCfg, Error> {
        match &self.write_to {
            Some(name) => self
                .layers
                .iter()
                .rev()
                .find(|(layer, _)| layer == name)
                .map(|(_, cfg)| cfg)
                .ok_or_else(|| Error::UnknownSource(name.clone())),
            None => Err(Error::NoWriteTarget),
        }
    }

    /// Set one key to a string in the writable source. Everything else in
    /// that file is preserved; no other source is touched.
    pub fn set(&self, key: &str, val: &str) -> Result<(), Error> {
        self.writer()?.set(key, val)
    }

    /// Set one key to a typed value in the writable source.
    pub fn set_val<T: Serialize>(&self, key: &str, val: T) -> Result<(), Error> {
        self.writer()?.set_val(key, val)
    }

    /// Save a whole struct to the writable source, in that source's format.
    ///
    /// This persists the value given, not the resolved view: nothing is
    /// merged in, so lower layers and env overrides are never flattened
    /// into the target.
    pub fn save<T: Serialize>(&self, config: &T) -> Result<(), Error> {
        self.writer()?.save(config)
    }

    /// Set one key to a string in the writable source, using Tokio.
    ///
    /// Same target rules as [`Self::set`]: only the selected source is
    /// touched, and read-only stacks report [`Error::NoWriteTarget`].
    #[cfg(feature = "async")]
    pub async fn set_async(&self, key: &str, val: &str) -> Result<(), Error> {
        self.writer()?.set_async(key, val).await
    }

    /// Set one key to a typed value in the writable source, using Tokio.
    ///
    /// Same target rules as [`Self::set_val`].
    #[cfg(feature = "async")]
    pub async fn set_val_async<T: Serialize>(&self, key: &str, val: T) -> Result<(), Error> {
        self.writer()?.set_val_async(key, val).await
    }

    /// Save a whole struct to the writable source with Tokio filesystem I/O.
    ///
    /// Like [`Self::save`], this writes the value given rather than the
    /// resolved view, so lower layers and env overrides are never flattened
    /// into the target.
    #[cfg(feature = "async")]
    pub async fn save_async<T: Serialize>(&self, config: &T) -> Result<(), Error> {
        self.writer()?.save_async(config).await
    }

    /// Direct access to one named source, bypassing resolution. The source's
    /// own env prefix applies here, if it has one.
    pub fn source(&self, name: &str) -> Result<&DotCfg, Error> {
        self.layers
            .iter()
            .rev()
            .find(|(layer, _)| layer == name)
            .map(|(_, cfg)| cfg)
            .ok_or_else(|| Error::UnknownSource(name.to_string()))
    }
}

impl Default for ConfigStack {
    /// An empty, read-only stack with no layers and no prefix.
    fn default() -> Self {
        Self::new()
    }
}

/// File trees plus discovered env entries backing one resolution.
struct Overlay {
    tree: Value,
    env: Vec<(Vec<String>, String, OsString)>,
}

impl Overlay {
    /// Tree node at `path`, if any file layer defines it.
    fn tree_at(&self, path: &[String]) -> Option<&Value> {
        let mut node = &self.tree;
        for segment in path {
            node = node.get(segment)?;
        }
        Some(node)
    }

    /// Exact env match at `path`: `(var, raw)`.
    fn env_at(&self, path: &[String]) -> Option<(&str, &OsString)> {
        self.env
            .iter()
            .find(|(entry, _, _)| entry == path)
            .map(|(_, var, raw)| (var.as_str(), raw))
    }

    /// Whether any env entry lives strictly below `path`.
    fn has_env_children(&self, path: &[String]) -> bool {
        self.env
            .iter()
            .any(|(entry, _, _)| entry.len() > path.len() && entry.starts_with(path))
    }

    /// Child names at `path`: tree keys plus discovered next segments,
    /// sorted for deterministic resolution.
    fn child_names(&self, path: &[String]) -> Vec<String> {
        let mut names = BTreeSet::new();
        if let Some(Value::Object(map)) = self.tree_at(path) {
            names.extend(map.keys().cloned());
        }
        for (entry, _, _) in &self.env {
            if let Some(rest) = entry.strip_prefix(path) {
                if let Some(first) = rest.first() {
                    names.insert(first.clone());
                }
            }
        }
        names.into_iter().collect()
    }

    /// Owned value node at `path`. Env wins over the tree. Without env
    /// below, the file subtree resolves exactly as a single-file read, so
    /// zero-env stacks behave identically to [`DotCfg`]. A scalar or array
    /// blocking env-nested paths is [`Error::NotATable`], never silently
    /// dropped.
    fn node_at(&self, path: &[String]) -> Result<Node, Error> {
        if let Some((var, raw)) = self.env_at(path) {
            return Ok(Node::Env {
                var: var.to_owned(),
                raw: raw.to_owned(),
            });
        }
        if !self.has_env_children(path) {
            return Ok(Node::Tree(
                self.tree_at(path).cloned().unwrap_or(Value::Null),
            ));
        }
        match self.tree_at(path) {
            Some(Value::Object(_)) | Some(Value::Null) | None => self.map_at(path),
            Some(_) => Err(Error::NotATable(display_path(path))),
        }
    }

    /// Map node at `path`: one child per name, resolved eagerly.
    fn map_at(&self, path: &[String]) -> Result<Node, Error> {
        let mut children = VecDeque::new();
        for name in self.child_names(path) {
            let mut child_path = path.to_vec();
            child_path.push(name.clone());
            children.push_back((name, self.node_at(&child_path)?));
        }
        Ok(Node::Map(children))
    }
}

/// Discovered `PREFIX_*` variables for one prefix: `(path, var, raw)`.
///
/// Shared discovery behind [`ConfigStack`] and [`DotCfg`](crate::DotCfg)
/// env overlays so both observe the same variables under one convention.
pub(crate) fn discover_env(prefix: Option<&str>) -> Vec<(Vec<String>, String, OsString)> {
    let Some(prefix) = prefix.map(|prefix| prefix.to_ascii_uppercase()) else {
        return Vec::new();
    };
    let stem = format!("{prefix}_");
    let mut out = Vec::new();
    for (name, val) in std::env::vars_os() {
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(suffix) = name.strip_prefix(&stem) else {
            continue;
        };
        let path: Vec<String> = suffix
            .split("__")
            .map(|segment| segment.to_ascii_lowercase())
            .filter(|segment| !segment.is_empty())
            .collect();
        if path.is_empty() {
            continue;
        }
        out.push((path, name.to_owned(), val));
    }
    out
}

/// Deserialize `T` from a file `tree` with discovered env entries overlaid.
///
/// Shared by [`ConfigStack::load`](ConfigStack::load) and
/// [`DotCfg::load`](crate::DotCfg::load): an empty `env` behaves exactly
/// like a single-file read.
pub(crate) fn load_overlay<T: DeserializeOwned>(
    tree: Value,
    env: Vec<(Vec<String>, String, OsString)>,
) -> Result<T, Error> {
    let overlay = Overlay { tree, env };
    T::deserialize(Cursor {
        node: overlay.node_at(&[])?,
    })
}

/// Display form of a path for errors: dotted, or `<root>`.
fn display_path(path: &[String]) -> String {
    if path.is_empty() {
        String::from("<root>")
    } else {
        path.join(".")
    }
}

/// One owned value in the resolved view.
enum Node {
    /// An exact env match wins over whatever the files hold. The raw value
    /// stays staged as [`OsString`] and is interpreted on read, so a
    /// non-unicode value fails with [`Error::EnvNotUnicode`] exactly like a
    /// direct lookup instead of poisoning discovery.
    Env { var: String, raw: OsString },
    /// A file-tree value, cloned out of the merged tree.
    Tree(Value),
    /// An object: pre-resolved children in deterministic order.
    Map(VecDeque<(String, Node)>),
}

/// Borrow staged env bytes as text, or report them like direct reads do.
fn env_text<'a>(var: &str, raw: &'a OsString) -> Result<&'a str, Error> {
    raw.to_str()
        .ok_or_else(|| Error::EnvNotUnicode(var.to_string()))
}

/// Deserializer over an owned [`Node`]: no borrows cross into visitor
/// outputs, so it satisfies any `Deserialize` bound including
/// `DeserializeOwned`.
struct Cursor {
    node: Node,
}

/// Dispatch one method: env nodes parse like [`DotCfg::get_as`], tree nodes
/// deserialize like single-file reads, maps visit their children.
macro_rules! dispatch {
    ($method:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.node {
                Node::Env { var, raw } => EnvStr {
                    raw: env_text(&var, &raw)?,
                }
                .$method(visitor)
                .map_err(|err| Error::EnvParse(var, err.to_string())),
                Node::Tree(value) => value.$method(visitor).map_err(tree_err),
                Node::Map(children) => visitor.visit_map(StackMapAccess {
                    remaining: children,
                    pending: None,
                }),
            }
        }
    };
}

impl<'de> Deserializer<'de> for Cursor {
    type Error = Error;

    dispatch!(deserialize_any);
    dispatch!(deserialize_bool);
    dispatch!(deserialize_i8);
    dispatch!(deserialize_i16);
    dispatch!(deserialize_i32);
    dispatch!(deserialize_i64);
    dispatch!(deserialize_i128);
    dispatch!(deserialize_u8);
    dispatch!(deserialize_u16);
    dispatch!(deserialize_u32);
    dispatch!(deserialize_u64);
    dispatch!(deserialize_u128);
    dispatch!(deserialize_f32);
    dispatch!(deserialize_f64);
    dispatch!(deserialize_char);
    dispatch!(deserialize_str);
    dispatch!(deserialize_string);
    dispatch!(deserialize_bytes);
    dispatch!(deserialize_byte_buf);
    dispatch!(deserialize_unit);
    dispatch!(deserialize_seq);
    dispatch!(deserialize_identifier);
    dispatch!(deserialize_ignored_any);

    /// Unit structs, tuples and tuple structs follow the node kind like
    /// every other scalar: env parses, trees forward, maps fail as invalid
    /// types — each through its own natural error.
    fn deserialize_unit_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.node {
            Node::Env { var, raw } => EnvStr {
                raw: env_text(&var, &raw)?,
            }
            .deserialize_unit_struct(name, visitor)
            .map_err(|err| Error::EnvParse(var, err.to_string())),
            Node::Tree(value) => value
                .deserialize_unit_struct(name, visitor)
                .map_err(tree_err),
            Node::Map(children) => visitor.visit_map(StackMapAccess {
                remaining: children,
                pending: None,
            }),
        }
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.node {
            Node::Env { var, raw } => EnvStr {
                raw: env_text(&var, &raw)?,
            }
            .deserialize_tuple(len, visitor)
            .map_err(|err| Error::EnvParse(var, err.to_string())),
            Node::Tree(value) => value.deserialize_tuple(len, visitor).map_err(tree_err),
            Node::Map(children) => visitor.visit_map(StackMapAccess {
                remaining: children,
                pending: None,
            }),
        }
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.node {
            Node::Env { var, raw } => EnvStr {
                raw: env_text(&var, &raw)?,
            }
            .deserialize_tuple_struct(name, len, visitor)
            .map_err(|err| Error::EnvParse(var, err.to_string())),
            Node::Tree(value) => value
                .deserialize_tuple_struct(name, len, visitor)
                .map_err(tree_err),
            Node::Map(children) => visitor.visit_map(StackMapAccess {
                remaining: children,
                pending: None,
            }),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.node {
            Node::Env { var, raw } => EnvStr {
                raw: env_text(&var, &raw)?,
            }
            .deserialize_option(visitor)
            .map_err(|err| Error::EnvParse(var, err.to_string())),
            Node::Tree(value) => value.deserialize_option(visitor).map_err(tree_err),
            Node::Map(children) => visitor.visit_some(Cursor {
                node: Node::Map(children),
            }),
        }
    }

    /// Newtypes unwrap through a fresh cursor, so the wrapped value keeps
    /// full overlay semantics (env leaves stay typed) instead of freezing
    /// into strings.
    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.node {
            Node::Map(children) => visitor.visit_newtype_struct(Cursor {
                node: Node::Map(children),
            }),
            Node::Env { var, raw } => EnvStr {
                raw: env_text(&var, &raw)?,
            }
            .deserialize_newtype_struct(_name, visitor)
            .map_err(|err| Error::EnvParse(var, err.to_string())),
            Node::Tree(value) => value
                .deserialize_newtype_struct(_name, visitor)
                .map_err(tree_err),
        }
    }

    /// Externally tagged enums (`{Variant: payload}`) resolve from maps the
    /// same way single-file reads do; unit variants from env behave like
    /// [`DotCfg::get_as`].
    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.node {
            Node::Map(mut children) => {
                if children.len() != 1 {
                    return Err(Error::Deserialize(
                        "expected map with a single key".to_string(),
                    ));
                }
                let (variant, node) = children.pop_front().expect("checked length");
                visitor.visit_enum(StackEnumAccess { variant, node })
            }
            Node::Env { var, raw } => EnvStr {
                raw: env_text(&var, &raw)?,
            }
            .deserialize_enum(_name, _variants, visitor)
            .map_err(|err| Error::EnvParse(var, err.to_string())),
            Node::Tree(value) => value
                .deserialize_enum(_name, _variants, visitor)
                .map_err(tree_err),
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.node {
            Node::Map(children) => visitor.visit_map(StackMapAccess {
                remaining: children,
                pending: None,
            }),
            Node::Env { var, raw } => EnvStr {
                raw: env_text(&var, &raw)?,
            }
            .deserialize_map(visitor)
            .map_err(|err| Error::EnvParse(var, err.to_string())),
            Node::Tree(value) => value.deserialize_map(visitor).map_err(tree_err),
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.node {
            Node::Map(children) => visitor.visit_map(StackMapAccess {
                remaining: children,
                pending: None,
            }),
            Node::Env { var, raw } => EnvStr {
                raw: env_text(&var, &raw)?,
            }
            .deserialize_struct(_name, _fields, visitor)
            .map_err(|err| Error::EnvParse(var, err.to_string())),
            Node::Tree(value) => value
                .deserialize_struct(_name, _fields, visitor)
                .map_err(tree_err),
        }
    }
}

/// One-line kind of a node for error messages.
fn kind_of(node: &Node) -> &'static str {
    match node {
        Node::Env { .. } => "a string",
        Node::Tree(Value::Null) => "null",
        Node::Tree(Value::Bool(_)) => "a boolean",
        Node::Tree(Value::Number(_)) => "a number",
        Node::Tree(Value::String(_)) => "a string",
        Node::Tree(Value::Array(_)) => "a sequence",
        Node::Tree(Value::Object(_)) | Node::Map(_) => "a map",
    }
}

/// Enum access over a single-pair map: the variant comes from the one key,
/// the payload resolves through a fresh cursor, so typed env leaves keep
/// their exact deserialization.
struct StackEnumAccess {
    variant: String,
    node: Node,
}

impl<'de> EnumAccess<'de> for StackEnumAccess {
    type Error = Error;
    type Variant = StackVariantAccess;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Error>
    where
        V: DeserializeSeed<'de>,
    {
        let variant = seed.deserialize(MapKey(self.variant))?;
        Ok((variant, StackVariantAccess { node: self.node }))
    }
}

/// Payload side of [`StackEnumAccess`].
struct StackVariantAccess {
    node: Node,
}

impl<'de> VariantAccess<'de> for StackVariantAccess {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        match self.node {
            Node::Tree(Value::Null) => Ok(()),
            node => Err(Error::Deserialize(format!(
                "invalid type: {}, expected unit variant",
                kind_of(&node)
            ))),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: DeserializeSeed<'de>,
    {
        seed.deserialize(Cursor { node: self.node })
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        Cursor { node: self.node }.deserialize_tuple(len, visitor)
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        Cursor { node: self.node }.deserialize_struct("", fields, visitor)
    }
}

/// Map access over pre-resolved children.
struct StackMapAccess {
    remaining: VecDeque<(String, Node)>,
    pending: Option<Node>,
}

impl<'de> MapAccess<'de> for StackMapAccess {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: DeserializeSeed<'de>,
    {
        match self.remaining.pop_front() {
            Some((key, value)) => {
                self.pending = Some(value);
                seed.deserialize(MapKey(key)).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Error>
    where
        V: DeserializeSeed<'de>,
    {
        match self.pending.take() {
            Some(node) => seed.deserialize(Cursor { node }),
            None => Err(Error::Deserialize("map value missing for key".to_string())),
        }
    }
}

/// A map key: an owned string presented as an identifier.
struct MapKey(String);

impl<'de> Deserializer<'de> for MapKey {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.0)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

/// Map a tree deserialization failure like [`value::from_value`].
fn tree_err(err: serde_json::Error) -> Error {
    Error::Deserialize(err.to_string())
}
