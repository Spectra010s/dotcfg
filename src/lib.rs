//! # dotcfg
//!
//! Flexible config management for Rust applications.
//!
//! - Choose between `~/.toolname/`, `~/.config/toolname/`, or a custom directory
//! - TOML, JSON or YAML format (feature-gated)
//! - Load, save, get, set — full or per-key
//! - Typed per-key access with `get_as` / `set_val` (numbers, bools, arrays, structs)
//! - Flat (`username`) and nested (`user.username`) key support
//! - Opt-in environment variable overrides via `with_env_prefix`
//! - Returns `None` if config doesn't exist — no magic auto-create unless you want it
//!
//! ## Features
//!
//! - `toml` (default) — enables TOML support
//! - `json` — enables JSON support
//! - `yaml` — enables YAML support
//!
//! ## Example
//!
//! ```rust,no_run
//! use dotcfg::DotCfg;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize, Default)]
//! struct MyConfig {
//!     username: String,
//!     port: u16,
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let cfg = DotCfg::new("mytool");
//!
//!     // Load — returns None if file doesn't exist
//!     let config: Option<MyConfig> = cfg.load()?;
//!
//!     // Save
//!     cfg.save(&MyConfig { username: "john".into(), port: 8080 })?;
//!
//!     // Get a single key
//!     let val = cfg.get("username")?;
//!
//!     // Set a single key
//!     cfg.set("username", "jane")?;
//!
//!     // Typed per-key access — no string round trip
//!     cfg.set_val("port", 8080u16)?;
//!     let port: u16 = cfg.get_as("port")?;
//!
//!     Ok(())
//! }
//! ```

mod env;
pub mod error;
mod value;

use std::{
    fs,
    path::{Path, PathBuf},
};

pub use error::Error;
use serde::{Deserialize, Serialize};
use value::Value;

#[cfg(not(any(feature = "toml", feature = "json", feature = "yaml")))]
compile_error!(
    "dotcfg requires at least one of the `toml`, `json` or `yaml` features to be enabled"
);

/// Where the config folder lives
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirStrategy {
    /// `~/.toolname/` — like `.cargo`, `.ssh`, `.git`
    Dot,
    /// `~/.config/toolname/` — XDG standard
    Xdg,
    /// Custom directory path
    Custom(PathBuf),
}

/// Config file format
pub enum Format {
    #[cfg(feature = "toml")]
    Toml,
    #[cfg(feature = "json")]
    Json,
    #[cfg(feature = "yaml")]
    Yaml,
}

/// The main dotcfg handle. Create one per app.
///
/// ```rust,no_run
/// use dotcfg::DotCfg;
///
/// // ~/.mytool/config.toml (default)
/// let cfg = DotCfg::new("mytool");
///
/// // ~/.config/mytool/config.toml
/// let cfg = DotCfg::new("mytool").xdg();
///
/// // Custom directory — for testing, project-local configs, etc.
/// let cfg = DotCfg::new("mytool").at_dir("/tmp/my-test-dir");
///
/// // ~/.mytool/settings.json (requires `json` feature)
/// #[cfg(feature = "json")]
/// let cfg = DotCfg::new("mytool").json().filename("settings");
///
/// // ~/.mytool/config.yaml (requires `yaml` feature)
/// #[cfg(feature = "yaml")]
/// let cfg = DotCfg::new("mytool").yaml();
/// ```
pub struct DotCfg {
    app_name: String,
    strategy: DirStrategy,
    format: Format,
    filename: String,
    env_prefix: Option<String>,
}

impl DotCfg {
    /// Create a new DotCfg for your app.
    /// Defaults: `~/.appname/config.toml`, no auto-create.
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            strategy: DirStrategy::Dot,
            #[cfg(feature = "toml")]
            format: Format::Toml,
            #[cfg(all(feature = "json", not(feature = "toml")))]
            format: Format::Json,
            #[cfg(all(feature = "yaml", not(feature = "toml"), not(feature = "json")))]
            format: Format::Yaml,
            filename: "config".to_string(),
            env_prefix: None,
        }
    }

    /// Use `~/.config/toolname/` (XDG)
    pub fn xdg(mut self) -> Self {
        self.strategy = DirStrategy::Xdg;
        self
    }

    /// Use `~/.toolname/` (default)
    pub fn dot(mut self) -> Self {
        self.strategy = DirStrategy::Dot;
        self
    }

    /// Use a custom directory path.
    ///
    /// Bypasses both `Dot` and `Xdg` resolution. The config file lives directly
    /// under the directory you pass, exactly as given. It can be any path you
    /// choose, like a temp dir, a project dir, a portable dir, or even a home
    /// dir if you want. No prefix is enforced, so `".mytool"` and `"my-config"`
    /// both work.
    ///
    /// Useful for isolated testing with `tempfile::tempdir()`, project-local
    /// configs inside a repository, or portable setups with non-standard paths.
    ///
    /// ```rust,no_run
    /// # use dotcfg::DotCfg;
    /// // /my/project/.config/config.toml — any dir, exactly as given
    /// let cfg = DotCfg::new("mytool").at_dir("/my/project/.config");
    /// ```
    pub fn at_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.strategy = DirStrategy::Custom(path.into());
        self
    }

    /// Search ancestors for a project config.
    ///
    /// Starts at `std::env::current_dir()` and walks up through `ancestors()`,
    /// looking for `.{app_name}/{filename}.{ext}`. If found, returns a new
    /// `DotCfg` with `DirStrategy::Custom` pointing to that directory. If no
    /// ancestor contains it, returns `Ok(None)` with no fallback to home or XDG.
    /// This keeps the lookup explicit, matching `dotcfg`'s no-magic policy.
    ///
    /// ```rust,no_run
    /// # use dotcfg::DotCfg;
    /// # use serde::{Deserialize, Serialize};
    /// # #[derive(Serialize, Deserialize)] struct Cfg { val: String }
    /// let project: Option<DotCfg> = DotCfg::new("mytool").find_in_ancestors().unwrap();
    /// match project {
    ///     Some(cfg) => { let _: Option<Cfg> = cfg.load().unwrap(); }
    ///     None => { let _ = DotCfg::new("mytool").xdg(); }
    /// }
    /// ```
    pub fn find_in_ancestors(self) -> Result<Option<Self>, Error> {
        let cwd = std::env::current_dir().map_err(Error::Io)?;
        self.find_in_ancestors_from(cwd)
    }

    /// Same as `find_in_ancestors` but starts from an explicit `start` directory.
    ///
    /// Useful for testing or when you already have a project path. The `start`
    /// directory itself is checked first, then its parents up to the filesystem
    /// root.
    ///
    /// ```rust,no_run
    /// # use dotcfg::DotCfg;
    /// let cfg = DotCfg::new("mytool").find_in_ancestors_from("/tmp/my/project/src").unwrap();
    /// ```
    pub fn find_in_ancestors_from(self, start: impl AsRef<Path>) -> Result<Option<Self>, Error> {
        let ext = match &self.format {
            #[cfg(feature = "toml")]
            Format::Toml => "toml",
            #[cfg(feature = "json")]
            Format::Json => "json",
            #[cfg(feature = "yaml")]
            Format::Yaml => "yaml",
        };
        let file_name = format!("{}.{}", self.filename, ext);
        let dot_dir = format!(".{}", self.app_name);
        for ancestor in start.as_ref().ancestors() {
            let dir = ancestor.join(&dot_dir);
            let file = dir.join(&file_name);
            if file.is_file() {
                return Ok(Some(Self {
                    app_name: self.app_name,
                    strategy: DirStrategy::Custom(dir),
                    format: self.format,
                    filename: self.filename,
                    env_prefix: self.env_prefix,
                }));
            }
        }
        Ok(None)
    }

    /// Use JSON format
    #[cfg(feature = "json")]
    pub fn json(mut self) -> Self {
        self.format = Format::Json;
        self
    }

    /// Use YAML format
    #[cfg(feature = "yaml")]
    pub fn yaml(mut self) -> Self {
        self.format = Format::Yaml;
        self
    }

    /// Use TOML format (default)
    #[cfg(feature = "toml")]
    pub fn toml(mut self) -> Self {
        self.format = Format::Toml;
        self
    }

    /// Set the config filename (without extension). Default is `"config"`.
    pub fn filename(mut self, name: impl Into<String>) -> Self {
        self.filename = name.into();
        self
    }

    /// Let environment variables override per-key reads.
    ///
    /// Once a prefix is set, [`Self::get`] and [`Self::get_as`] check the
    /// environment first and only fall back to the config file when the
    /// variable is unset. Opt-in: without this, the environment is never read.
    ///
    /// A key maps to `<PREFIX>_<KEY>`, uppercased, with `.` replaced by `_`:
    ///
    /// | key | env var (prefix `myapp`) |
    /// | :-- | :-- |
    /// | `port` | `MYAPP_PORT` |
    /// | `user.name` | `MYAPP_USER_NAME` |
    ///
    /// ```rust,no_run
    /// # use dotcfg::DotCfg;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cfg = DotCfg::new("mytool").with_env_prefix("myapp");
    ///
    /// // reads $MYAPP_PORT if set, otherwise `port` from the config file
    /// let port: u16 = cfg.get_as("port")?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Writes are unaffected — [`Self::set`] and [`Self::set_val`] always go to
    /// the file and never touch the environment.
    pub fn with_env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.env_prefix = Some(prefix.into());
        self
    }

    /// The env var name `key` maps to, or `None` when no prefix is configured.
    ///
    /// Single source of truth for the mapping — both read paths go through it.
    fn env_var_name(&self, key: &str) -> Option<String> {
        let prefix = self.env_prefix.as_ref()?;
        Some(format!(
            "{}_{}",
            prefix.to_ascii_uppercase(),
            key.replace('.', "_").to_ascii_uppercase()
        ))
    }

    /// The environment override for `key`, as `(var name, raw value)`.
    ///
    /// `Ok(None)` means "no prefix configured, or the var is unset" — i.e. fall
    /// through to the file. A var that is set but unreadable is an error rather
    /// than a silent fallback, so a misconfigured environment stays visible.
    fn env_override(&self, key: &str) -> Result<Option<(String, String)>, Error> {
        let Some(var) = self.env_var_name(key) else {
            return Ok(None);
        };

        match std::env::var(&var) {
            Ok(raw) => Ok(Some((var, raw))),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_)) => Err(Error::EnvNotUnicode(var)),
        }
    }

    /// Returns the config directory path
    pub fn dir(&self) -> Result<PathBuf, Error> {
        let dir = match &self.strategy {
            DirStrategy::Dot => {
                // ~/.toolname/ — Unix convention, we resolve manually
                let home = home::home_dir().ok_or(Error::NoHomeDir)?;
                home.join(format!(".{}", self.app_name))
            }
            DirStrategy::Xdg => {
                // Uses etcetera — handles Linux (XDG), macOS, Windows correctly
                use etcetera::app_strategy::{AppStrategy, AppStrategyArgs, Xdg};
                let strategy = Xdg::new(AppStrategyArgs {
                    top_level_domain: "".to_string(),
                    author: "".to_string(),
                    app_name: self.app_name.clone(),
                })
                .map_err(|_| Error::NoHomeDir)?;
                strategy.config_dir()
            }
            DirStrategy::Custom(path) => path.clone(),
        };
        Ok(dir)
    }

    /// Returns the full config file path
    pub fn file_path(&self) -> Result<PathBuf, Error> {
        let ext = match self.format {
            #[cfg(feature = "toml")]
            Format::Toml => "toml",
            #[cfg(feature = "json")]
            Format::Json => "json",
            #[cfg(feature = "yaml")]
            Format::Yaml => "yaml",
        };
        Ok(self.dir()?.join(format!("{}.{}", self.filename, ext)))
    }

    /// Returns true if the config file exists
    pub fn exists(&self) -> Result<bool, Error> {
        Ok(self.file_path()?.exists())
    }

    /// Ensures the config directory exists, creating it if needed
    fn ensure_dir(&self) -> Result<(), Error> {
        let dir = self.dir()?;
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        Ok(())
    }

    /// Read the config file into the common value tree.
    ///
    /// `Ok(None)` when the file doesn't exist. An existing file with no values
    /// (zero bytes, whitespace, an empty document) is an empty tree; a malformed
    /// one is an error.
    fn read_tree(&self) -> Result<Option<Value>, Error> {
        let path = self.file_path()?;

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)?;
        value::parse(&self.format, &content).map(Some)
    }

    /// Like [`Self::read_tree`], but a missing file is an empty tree — the
    /// starting point for mutations.
    fn read_tree_or_empty(&self) -> Result<Value, Error> {
        Ok(self.read_tree()?.unwrap_or_else(value::empty))
    }

    /// Serialize `tree` in the selected format and write it, creating the
    /// config directory if needed.
    fn write_tree(&self, tree: &Value) -> Result<(), Error> {
        let content = value::render(&self.format, tree)?;
        self.ensure_dir()?;
        fs::write(self.file_path()?, content)?;
        Ok(())
    }

    /// Load the config file.
    ///
    /// Returns `None` if the file doesn't exist — no auto-create.
    /// Use [`Self::load_or_default`] if you want auto-create behavior.
    ///
    /// A file that exists but holds no values (empty, whitespace only, or an
    /// empty document) is deserialized as an empty config, so it succeeds only
    /// if `T` can be built from no fields (e.g. all fields are `Option` or
    /// `#[serde(default)]`). A malformed file is an error.
    pub fn load<T>(&self) -> Result<Option<T>, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        match self.read_tree()? {
            Some(tree) => value::from_value(tree).map(Some),
            None => Ok(None),
        }
    }

    /// Load the config or return an error if it doesn't exist.
    ///
    /// Useful when your CLI requires setup before use.
    pub fn load_or_error<T>(&self) -> Result<T, Error>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.load()?.ok_or(Error::NotFound)
    }

    /// Load the config or create it with default values if it doesn't exist.
    ///
    /// This is the confy-style behavior — opt-in.
    pub fn load_or_default<T>(&self) -> Result<T, Error>
    where
        T: for<'de> Deserialize<'de> + Serialize + Default,
    {
        match self.load()? {
            Some(cfg) => Ok(cfg),
            None => {
                let default = T::default();
                self.save(&default)?;
                Ok(default)
            }
        }
    }

    /// Save a config struct to disk.
    ///
    /// Creates the config directory if it doesn't exist.
    pub fn save<T: Serialize>(&self, config: &T) -> Result<(), Error> {
        self.ensure_dir()?;
        let path = self.file_path()?;

        let content = match self.format {
            #[cfg(feature = "toml")]
            Format::Toml => toml::to_string_pretty(config)?,
            #[cfg(feature = "json")]
            Format::Json => serde_json::to_string_pretty(config)?,
            #[cfg(feature = "yaml")]
            Format::Yaml => serde_yaml_ng::to_string(config)?,
        };

        fs::write(&path, content)?;
        Ok(())
    }

    /// Get a single config value by key.
    ///
    /// Supports flat keys (`"username"`) and nested keys (`"user.username"`).
    ///
    /// Returns the value as a `String`. If [`Self::with_env_prefix`] is set and
    /// the matching environment variable exists, its raw value is returned
    /// as-is and the file is not read.
    pub fn get(&self, key: &str) -> Result<String, Error> {
        if let Some((_, raw)) = self.env_override(key)? {
            return Ok(raw);
        }

        let tree = self.read_tree()?.ok_or(Error::NotFound)?;
        value::get_node(&tree, key).map(|node| value::display(&self.format, node))
    }

    /// Set a single config value by key.
    ///
    /// Supports flat keys (`"username"`) and nested keys (`"user.username"`).
    ///
    /// Creates the config file and directory if they don't exist.
    /// If the file exists, only the specified key is updated — everything else is preserved.
    pub fn set(&self, key: &str, value: &str) -> Result<(), Error> {
        self.set_node(key, Value::String(value.to_string()))
    }

    /// Splice `new_val` into the tree at `key` and write it back.
    fn set_node(&self, key: &str, new_val: Value) -> Result<(), Error> {
        let mut tree = self.read_tree_or_empty()?;
        value::set_node(&mut tree, key, new_val)?;
        self.write_tree(&tree)
    }

    /// Get a single config value by key, deserialized into `T`.
    ///
    /// Like [`Self::get`], but returns a typed value instead of a `String`.
    /// The value node is handed straight to serde from the internal value
    /// tree — no stringify/re-parse round trip — so arrays, numbers and
    /// booleans deserialize cleanly.
    ///
    /// Supports flat keys (`"port"`) and nested keys (`"features.auto_update"`).
    ///
    /// ```rust,no_run
    /// # use dotcfg::DotCfg;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cfg = DotCfg::new("mytool");
    /// let port: u16 = cfg.get_as("port")?;
    /// let plugins: Vec<String> = cfg.get_as("plugins")?;
    /// let auto: bool = cfg.get_as("features.auto_update")?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Environment overrides
    ///
    /// With [`Self::with_env_prefix`] set, a matching environment variable wins
    /// over the file. Env values are untyped strings, so they are parsed rather
    /// than deserialized from a native value: numbers and floats via
    /// [`str::parse`], `bool` as `true`/`false`/`1`/`0` (case-insensitive), and
    /// sequences as **comma-separated** items (`MYAPP_TAGS=cli,fast`). Maps and
    /// structs can't come from a single variable — use nested keys.
    ///
    /// # Errors
    ///
    /// - [`Error::NotFound`] if the config file doesn't exist
    /// - [`Error::KeyNotFound`] if the key isn't present
    /// - [`Error::EnvParse`] if an env override isn't readable as a `T`
    ///   (a bad override is never silently ignored in favor of the file)
    /// - [`Error::Deserialize`] if the file value isn't a `T`
    /// - the format's parse error if the file is malformed
    pub fn get_as<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<T, Error> {
        if let Some((var, raw)) = self.env_override(key)? {
            return env::from_env_str(&var, &raw);
        }

        let tree = self.read_tree()?.ok_or(Error::NotFound)?;
        value::from_value(value::get_node(&tree, key)?.clone())
    }

    /// Set a single config value by key from any [`Serialize`] type.
    ///
    /// Like [`Self::set`], but writes a typed value instead of a string:
    /// `value` is serialized into the internal value tree and spliced in, so `42u16` lands as a number and
    /// `vec!["a", "b"]` as an array.
    ///
    /// Supports flat keys (`"port"`) and nested keys (`"features.auto_update"`),
    /// creating the intermediate table/map on demand. Creates the config file
    /// and directory if they don't exist; other keys are preserved.
    ///
    /// ```rust,no_run
    /// # use dotcfg::DotCfg;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let cfg = DotCfg::new("mytool");
    /// cfg.set_val("port", 8080u16)?;
    /// cfg.set_val("plugins", vec!["fmt", "lint"])?;
    /// cfg.set_val("features.auto_update", true)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_val<T: Serialize>(&self, key: &str, value: T) -> Result<(), Error> {
        self.set_node(key, value::to_value(value)?)
    }

    /// Delete the config file. The directory is kept.
    pub fn delete_file(&self) -> Result<(), Error> {
        let path = self.file_path()?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Delete the entire config directory and all its contents.
    pub fn delete_dir(&self) -> Result<(), Error> {
        let dir = self.dir()?;
        if dir.exists() {
            fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    /// Ensures the config directory exists, creating it asynchronously if needed
    #[cfg(feature = "async")]
    async fn ensure_dir_async(&self) -> Result<(), DotCfgError> {
        let dir = self.dir()?;
        if !tokio::fs::try_exists(&dir).await.map_err(DotCfgError::Io)? {
            tokio::fs::create_dir_all(&dir).await?;
        }
        Ok(())
    }

    /// Load the config file asynchronously.
    ///
    /// Returns `None` if the file doesn't exist — no auto-create.
    /// Use [`Self::load_or_default_async`] if you want auto-create behavior.
    #[cfg(feature = "async")]
    pub async fn load_async<T>(&self) -> Result<Option<T>, DotCfgError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let path = self.file_path()?;

        if !tokio::fs::try_exists(&path)
            .await
            .map_err(DotCfgError::Io)?
        {
            return Ok(None);
        }

        let content = tokio::fs::read_to_string(&path).await?;

        let config = match self.format {
            #[cfg(feature = "toml")]
            Format::Toml => toml::from_str(&content)?,
            #[cfg(feature = "json")]
            Format::Json => serde_json::from_str(&content)?,
            #[cfg(feature = "yaml")]
            Format::Yaml => serde_yaml_ng::from_str(&content)?,
        };

        Ok(Some(config))
    }

    /// Load the config or return an error if it doesn't exist (async).
    #[cfg(feature = "async")]
    pub async fn load_or_error_async<T>(&self) -> Result<T, DotCfgError>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.load_async().await?.ok_or(DotCfgError::NotFound)
    }

    /// Load the config or create it with default values asynchronously if it doesn't exist.
    #[cfg(feature = "async")]
    pub async fn load_or_default_async<T>(&self) -> Result<T, DotCfgError>
    where
        T: for<'de> Deserialize<'de> + Serialize + Default,
    {
        match self.load_async().await? {
            Some(cfg) => Ok(cfg),
            None => {
                let default = T::default();
                self.save_async(&default).await?;
                Ok(default)
            }
        }
    }

    /// Save a config struct to disk asynchronously.
    #[cfg(feature = "async")]
    pub async fn save_async<T: Serialize>(&self, config: &T) -> Result<(), DotCfgError> {
        self.ensure_dir_async().await?;
        let path = self.file_path()?;

        let content = match self.format {
            #[cfg(feature = "toml")]
            Format::Toml => toml::to_string_pretty(config)?,
            #[cfg(feature = "json")]
            Format::Json => serde_json::to_string_pretty(config)?,
            #[cfg(feature = "yaml")]
            Format::Yaml => serde_yaml_ng::to_string(config)?,
        };

        tokio::fs::write(&path, content).await?;
        Ok(())
    }

    /// Get a single config value by key asynchronously.
    #[cfg(feature = "async")]
    pub async fn get_async(&self, key: &str) -> Result<String, DotCfgError> {
        if let Some((_, raw)) = self.env_override(key)? {
            return Ok(raw);
        }

        let path = self.file_path()?;

        if !tokio::fs::try_exists(&path)
            .await
            .map_err(DotCfgError::Io)?
        {
            return Err(DotCfgError::NotFound);
        }

        let content = tokio::fs::read_to_string(&path).await?;

        match self.format {
            #[cfg(feature = "toml")]
            Format::Toml => {
                let value: toml::Value = toml::from_str(&content)?;
                get_toml_value(&value, key)
            }
            #[cfg(feature = "json")]
            Format::Json => {
                let value: serde_json::Value = serde_json::from_str(&content)?;
                get_json_value(&value, key)
            }
            #[cfg(feature = "yaml")]
            Format::Yaml => {
                let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
                get_yaml_value(&value, key)
            }
        }
    }

    /// Set a single config value by key asynchronously.
    #[cfg(feature = "async")]
    pub async fn set_async(&self, key: &str, value: &str) -> Result<(), DotCfgError> {
        let path = self.file_path()?;

        match self.format {
            #[cfg(feature = "toml")]
            Format::Toml => {
                let exists = tokio::fs::try_exists(&path)
                    .await
                    .map_err(DotCfgError::Io)?;
                let mut table: toml::Value = if exists {
                    let content = tokio::fs::read_to_string(&path).await?;
                    toml::from_str(&content)?
                } else {
                    toml::Value::Table(toml::map::Map::new())
                };

                set_toml_value(&mut table, key, value)?;
                self.ensure_dir_async().await?;
                tokio::fs::write(&path, toml::to_string_pretty(&table)?).await?;
            }
            #[cfg(feature = "json")]
            Format::Json => {
                let exists = tokio::fs::try_exists(&path)
                    .await
                    .map_err(DotCfgError::Io)?;
                let mut json: serde_json::Value = if exists {
                    let content = tokio::fs::read_to_string(&path).await?;
                    serde_json::from_str(&content)?
                } else {
                    serde_json::Value::Object(serde_json::Map::new())
                };

                set_json_value(&mut json, key, value)?;
                self.ensure_dir_async().await?;
                tokio::fs::write(&path, serde_json::to_string_pretty(&json)?).await?;
            }
            #[cfg(feature = "yaml")]
            Format::Yaml => {
                let exists = tokio::fs::try_exists(&path)
                    .await
                    .map_err(DotCfgError::Io)?;
                let mut yaml: serde_yaml_ng::Value = if exists {
                    let content = tokio::fs::read_to_string(&path).await?;
                    serde_yaml_ng::from_str(&content)?
                } else {
                    serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new())
                };

                set_yaml_value(&mut yaml, key, value)?;
                self.ensure_dir_async().await?;
                tokio::fs::write(&path, serde_yaml_ng::to_string(&yaml)?).await?;
            }
        }

        Ok(())
    }

    /// Get a single config value by key, deserialized into `T` asynchronously.
    #[cfg(feature = "async")]
    pub async fn get_as_async<T: serde::de::DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<T, DotCfgError> {
        if let Some((var, raw)) = self.env_override(key)? {
            return env::from_env_str(&var, &raw);
        }

        let path = self.file_path()?;

        if !tokio::fs::try_exists(&path)
            .await
            .map_err(DotCfgError::Io)?
        {
            return Err(DotCfgError::NotFound);
        }

        let content = tokio::fs::read_to_string(&path).await?;

        match self.format {
            #[cfg(feature = "toml")]
            Format::Toml => {
                let value: toml::Value = toml::from_str(&content)?;
                Ok(get_toml_node(&value, key)?.clone().try_into()?)
            }
            #[cfg(feature = "json")]
            Format::Json => {
                let value: serde_json::Value = serde_json::from_str(&content)?;
                Ok(serde_json::from_value(get_json_node(&value, key)?.clone())?)
            }
            #[cfg(feature = "yaml")]
            Format::Yaml => {
                let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)?;
                Ok(serde_yaml_ng::from_value(
                    get_yaml_node(&value, key)?.clone(),
                )?)
            }
        }
    }

    /// Set a single config value by key from any [`Serialize`] type asynchronously.
    #[cfg(feature = "async")]
    pub async fn set_val_async<T: Serialize>(
        &self,
        key: &str,
        value: T,
    ) -> Result<(), DotCfgError> {
        let path = self.file_path()?;

        match self.format {
            #[cfg(feature = "toml")]
            Format::Toml => {
                let new_val = toml::Value::try_from(value)?;
                let exists = tokio::fs::try_exists(&path)
                    .await
                    .map_err(DotCfgError::Io)?;

                let mut table: toml::Value = if exists {
                    let content = tokio::fs::read_to_string(&path).await?;
                    toml::from_str(&content)?
                } else {
                    toml::Value::Table(toml::map::Map::new())
                };

                set_toml_node(&mut table, key, new_val)?;
                self.ensure_dir_async().await?;
                tokio::fs::write(&path, toml::to_string_pretty(&table)?).await?;
            }
            #[cfg(feature = "json")]
            Format::Json => {
                let new_val = serde_json::to_value(value)?;
                let exists = tokio::fs::try_exists(&path)
                    .await
                    .map_err(DotCfgError::Io)?;

                let mut json: serde_json::Value = if exists {
                    let content = tokio::fs::read_to_string(&path).await?;
                    serde_json::from_str(&content)?
                } else {
                    serde_json::Value::Object(serde_json::Map::new())
                };

                set_json_node(&mut json, key, new_val)?;
                self.ensure_dir_async().await?;
                tokio::fs::write(&path, serde_json::to_string_pretty(&json)?).await?;
            }
            #[cfg(feature = "yaml")]
            Format::Yaml => {
                let new_val = serde_yaml_ng::to_value(value)?;
                let exists = tokio::fs::try_exists(&path)
                    .await
                    .map_err(DotCfgError::Io)?;

                let mut yaml: serde_yaml_ng::Value = if exists {
                    let content = tokio::fs::read_to_string(&path).await?;
                    serde_yaml_ng::from_str(&content)?
                } else {
                    serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new())
                };

                set_yaml_node(&mut yaml, key, new_val)?;
                self.ensure_dir_async().await?;
                tokio::fs::write(&path, serde_yaml_ng::to_string(&yaml)?).await?;
            }
        }

        Ok(())
    }

    /// Search ancestors for a project config asynchronously.
    #[cfg(feature = "async")]
    pub async fn find_in_ancestors_async(self) -> Result<Option<Self>, DotCfgError> {
        let cwd = std::env::current_dir().map_err(DotCfgError::Io)?;
        self.find_in_ancestors_from_async(cwd).await
    }

    /// Same as `find_in_ancestors_async` but starts from an explicit `start` directory.
    #[cfg(feature = "async")]
    pub async fn find_in_ancestors_from_async(
        self,
        start: impl AsRef<Path>,
    ) -> Result<Option<Self>, DotCfgError> {
        let ext = match &self.format {
            #[cfg(feature = "toml")]
            Format::Toml => "toml",
            #[cfg(feature = "json")]
            Format::Json => "json",
            #[cfg(feature = "yaml")]
            Format::Yaml => "yaml",
        };
        let file_name = format!("{}.{}", self.filename, ext);
        let dot_dir = format!(".{}", self.app_name);
        for ancestor in start.as_ref().ancestors() {
            let dir = ancestor.join(&dot_dir);
            let file = dir.join(&file_name);
            if tokio::fs::try_exists(&file).await.unwrap_or(false) {
                if let Ok(meta) = tokio::fs::metadata(&file).await {
                    if meta.is_file() {
                        return Ok(Some(Self {
                            app_name: self.app_name,
                            strategy: DirStrategy::Custom(dir),
                            format: self.format,
                            filename: self.filename,
                            env_prefix: self.env_prefix,
                        }));
                    }
                }
            }
        }
        Ok(None)
    }

    /// Returns true if the config file exists (async).
    #[cfg(feature = "async")]
    pub async fn exists_async(&self) -> Result<bool, DotCfgError> {
        tokio::fs::try_exists(self.file_path()?)
            .await
            .map_err(DotCfgError::Io)
    }

    /// Delete the config file asynchronously. The directory is kept.
    #[cfg(feature = "async")]
    pub async fn delete_file_async(&self) -> Result<(), DotCfgError> {
        let path = self.file_path()?;
        if tokio::fs::try_exists(&path)
            .await
            .map_err(DotCfgError::Io)?
        {
            tokio::fs::remove_file(path).await?;
        }
        Ok(())
    }

    /// Delete the entire config directory and all its contents asynchronously.
    #[cfg(feature = "async")]
    pub async fn delete_dir_async(&self) -> Result<(), DotCfgError> {
        let dir = self.dir()?;
        if tokio::fs::try_exists(&dir).await.map_err(DotCfgError::Io)? {
            tokio::fs::remove_dir_all(dir).await?;
        }
        Ok(())
    }
}

// Unit tests for private helpers
#[cfg(test)]
mod unit_tests {
    use super::*;

    /// Sets a uniquely-named env var for one test.
    ///
    /// SAFETY: every test here builds its var name from the process id plus its
    /// own suffix, so no other test in this binary reads or writes the same var.
    fn set_env(var: &str, val: &str) {
        unsafe { std::env::set_var(var, val) }
    }

    /// Unique env prefix per test, so parallel tests can't see each other's vars.
    fn env_prefix(suffix: &str) -> String {
        format!("dotcfg_t_{}_{}", suffix, std::process::id())
    }

    #[test]
    fn env_var_name_mapping() {
        let cfg = DotCfg::new("mytool").with_env_prefix("myapp");

        // flat key
        assert_eq!(cfg.env_var_name("port").unwrap(), "MYAPP_PORT");
        // nested key — dots become underscores
        assert_eq!(cfg.env_var_name("user.name").unwrap(), "MYAPP_USER_NAME");
        // already-uppercase input is left alone
        assert_eq!(cfg.env_var_name("PORT").unwrap(), "MYAPP_PORT");
        // the prefix is uppercased too
        assert_eq!(
            DotCfg::new("mytool")
                .with_env_prefix("MyApp")
                .env_var_name("port")
                .unwrap(),
            "MYAPP_PORT"
        );

        // no prefix configured — no env var to look at
        assert!(DotCfg::new("mytool").env_var_name("port").is_none());
    }

    #[test]
    fn env_override_three_way_fallback() {
        let prefix = env_prefix("threeway");
        let var = format!("{}_PORT", prefix.to_ascii_uppercase());

        // 1. no prefix configured — environment is never consulted
        set_env(&var, "9000");
        assert!(
            DotCfg::new("mytool")
                .env_override("port")
                .unwrap()
                .is_none()
        );

        let cfg = DotCfg::new("mytool").with_env_prefix(&prefix);

        // 2. prefix set and the var exists — override wins
        let (found_var, raw) = cfg.env_override("port").unwrap().unwrap();
        assert_eq!(found_var, var);
        assert_eq!(raw, "9000");

        // 3. prefix set but this key's var is unset — fall through to the file
        assert!(cfg.env_override("not_set_anywhere").unwrap().is_none());
    }

    #[test]
    fn env_get_returns_raw_string() {
        let prefix = env_prefix("get_raw");
        set_env(&format!("{}_USERNAME", prefix.to_ascii_uppercase()), "jane");

        // no config file exists for this app name — the override still resolves
        let cfg = DotCfg::new("dotcfg_no_such_app").with_env_prefix(&prefix);
        assert_eq!(cfg.get("username").unwrap(), "jane");

        // an unset key with no file still reports NotFound
        assert!(matches!(
            cfg.get("username_other").unwrap_err(),
            Error::NotFound
        ));
    }

    #[test]
    fn env_get_as_typed_values() {
        let prefix = env_prefix("get_as");
        let up = prefix.to_ascii_uppercase();
        set_env(&format!("{up}_PORT"), "9000");
        set_env(&format!("{up}_DEBUG"), "true");
        set_env(&format!("{up}_PLUGINS"), "fmt,lint");
        set_env(&format!("{up}_FEATURES_AUTO_UPDATE"), "false");
        set_env(&format!("{up}_BAD"), "notanumber");

        let cfg = DotCfg::new("dotcfg_no_such_app").with_env_prefix(&prefix);

        assert_eq!(cfg.get_as::<u16>("port").unwrap(), 9000);
        assert!(cfg.get_as::<bool>("debug").unwrap());
        assert_eq!(
            cfg.get_as::<Vec<String>>("plugins").unwrap(),
            vec!["fmt".to_string(), "lint".to_string()]
        );
        // nested key maps through the same transform
        assert!(!cfg.get_as::<bool>("features.auto_update").unwrap());

        // a malformed override is an error, not a panic and not a silent fallback
        assert!(matches!(
            cfg.get_as::<u16>("bad").unwrap_err(),
            Error::EnvParse(_, _)
        ));
    }
}
