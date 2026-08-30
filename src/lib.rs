//! # dotcfg
//!
//! Flexible config management for Rust apps.
//!
//! - Choose between `~/.toolname/` or `~/.config/toolname/`
//! - TOML, JSON or YAML format (feature-gated)
//! - Load, save, get, set — full or per-key
//! - Typed per-key access with `get_as` / `set_val` (numbers, bools, arrays, structs)
//! - Flat (`username`) and nested (`user.username`) key support
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

pub mod error;

use std::{fs, path::PathBuf};

use error::DotCfgError;
use serde::{Deserialize, Serialize};

#[cfg(not(any(feature = "toml", feature = "json", feature = "yaml")))]
compile_error!(
    "dotcfg requires at least one of the `toml`, `json` or `yaml` features to be enabled"
);

/// Where the config folder lives
pub enum DirStrategy {
    /// `~/.toolname/` — like `.cargo`, `.ssh`, `.git`
    Dot,
    /// `~/.config/toolname/` — XDG standard
    Xdg,
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

    /// Returns the config directory path
    pub fn dir(&self) -> Result<PathBuf, DotCfgError> {
        let dir = match self.strategy {
            DirStrategy::Dot => {
                // ~/.toolname/ — Unix convention, we resolve manually
                let home = home::home_dir().ok_or(DotCfgError::NoHomeDir)?;
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
                .map_err(|_| DotCfgError::NoHomeDir)?;
                strategy.config_dir()
            }
        };
        Ok(dir)
    }

    /// Returns the full config file path
    pub fn file_path(&self) -> Result<PathBuf, DotCfgError> {
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
    pub fn exists(&self) -> Result<bool, DotCfgError> {
        Ok(self.file_path()?.exists())
    }

    /// Ensures the config directory exists, creating it if needed
    fn ensure_dir(&self) -> Result<(), DotCfgError> {
        let dir = self.dir()?;
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        Ok(())
    }

    /// Load the config file.
    ///
    /// Returns `None` if the file doesn't exist — no auto-create.
    /// Use [`Self::load_or_default`] if you want auto-create behavior.
    pub fn load<T>(&self) -> Result<Option<T>, DotCfgError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let path = self.file_path()?;

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)?;

        let config = match self.format {
            #[cfg(feature = "toml")]
            Format::Toml => toml::from_str(&content)?,
            #[cfg(feature = "json")]
            Format::Json => serde_json::from_str(&content)?,
            #[cfg(feature = "yaml")]
            Format::Yaml => serde_yaml::from_str(&content)?,
        };

        Ok(Some(config))
    }

    /// Load the config or return an error if it doesn't exist.
    ///
    /// Useful when your CLI requires setup before use.
    pub fn load_or_error<T>(&self) -> Result<T, DotCfgError>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.load()?.ok_or(DotCfgError::NotFound)
    }

    /// Load the config or create it with default values if it doesn't exist.
    ///
    /// This is the confy-style behavior — opt-in.
    pub fn load_or_default<T>(&self) -> Result<T, DotCfgError>
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
    pub fn save<T: Serialize>(&self, config: &T) -> Result<(), DotCfgError> {
        self.ensure_dir()?;
        let path = self.file_path()?;

        let content = match self.format {
            #[cfg(feature = "toml")]
            Format::Toml => toml::to_string_pretty(config)?,
            #[cfg(feature = "json")]
            Format::Json => serde_json::to_string_pretty(config)?,
            #[cfg(feature = "yaml")]
            Format::Yaml => serde_yaml::to_string(config)?,
        };

        fs::write(&path, content)?;
        Ok(())
    }

    /// Get a single config value by key.
    ///
    /// Supports flat keys (`"username"`) and nested keys (`"user.username"`).
    ///
    /// Returns the value as a `String`.
    pub fn get(&self, key: &str) -> Result<String, DotCfgError> {
        let path = self.file_path()?;

        if !path.exists() {
            return Err(DotCfgError::NotFound);
        }

        let content = fs::read_to_string(&path)?;

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
                let value: serde_yaml::Value = serde_yaml::from_str(&content)?;
                get_yaml_value(&value, key)
            }
        }
    }

    /// Set a single config value by key.
    ///
    /// Supports flat keys (`"username"`) and nested keys (`"user.username"`).
    ///
    /// Creates the config file and directory if they don't exist.
    /// If the file exists, only the specified key is updated — everything else is preserved.
    pub fn set(&self, key: &str, value: &str) -> Result<(), DotCfgError> {
        let path = self.file_path()?;

        match self.format {
            #[cfg(feature = "toml")]
            Format::Toml => {
                let mut table: toml::Value = if path.exists() {
                    let content = fs::read_to_string(&path)?;
                    toml::from_str(&content)?
                } else {
                    toml::Value::Table(toml::map::Map::new())
                };

                set_toml_value(&mut table, key, value)?;
                self.ensure_dir()?;
                fs::write(&path, toml::to_string_pretty(&table)?)?;
            }
            #[cfg(feature = "json")]
            Format::Json => {
                let mut json: serde_json::Value = if path.exists() {
                    let content = fs::read_to_string(&path)?;
                    serde_json::from_str(&content)?
                } else {
                    serde_json::Value::Object(serde_json::Map::new())
                };

                set_json_value(&mut json, key, value)?;
                self.ensure_dir()?;
                fs::write(&path, serde_json::to_string_pretty(&json)?)?;
            }
            #[cfg(feature = "yaml")]
            Format::Yaml => {
                let mut yaml: serde_yaml::Value = if path.exists() {
                    let content = fs::read_to_string(&path)?;
                    serde_yaml::from_str(&content)?
                } else {
                    serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
                };

                set_yaml_value(&mut yaml, key, value)?;
                self.ensure_dir()?;
                fs::write(&path, serde_yaml::to_string(&yaml)?)?;
            }
        }

        Ok(())
    }

    /// Get a single config value by key, deserialized into `T`.
    ///
    /// Like [`Self::get`], but returns a typed value instead of a `String`.
    /// The value node is handed straight to serde in the config format's own
    /// representation — no stringify/re-parse round trip — so arrays, numbers
    /// and booleans deserialize cleanly.
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
    /// # Errors
    ///
    /// - [`DotCfgError::NotFound`] if the config file doesn't exist
    /// - [`DotCfgError::KeyNotFound`] if the key isn't present
    /// - the format's own (de)serialization error if the value isn't a `T`
    pub fn get_as<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<T, DotCfgError> {
        let path = self.file_path()?;

        if !path.exists() {
            return Err(DotCfgError::NotFound);
        }

        let content = fs::read_to_string(&path)?;

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
                let value: serde_yaml::Value = serde_yaml::from_str(&content)?;
                Ok(serde_yaml::from_value(get_yaml_node(&value, key)?.clone())?)
            }
        }
    }

    /// Set a single config value by key from any [`Serialize`] type.
    ///
    /// Like [`Self::set`], but writes a typed value instead of a string:
    /// `value` is serialized into the config format's own value representation
    /// and spliced into the tree, so `42u16` lands as a number and
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
    pub fn set_val<T: Serialize>(&self, key: &str, value: T) -> Result<(), DotCfgError> {
        let path = self.file_path()?;

        match self.format {
            #[cfg(feature = "toml")]
            Format::Toml => {
                let new_val = toml::Value::try_from(value)?;

                let mut table: toml::Value = if path.exists() {
                    let content = fs::read_to_string(&path)?;
                    toml::from_str(&content)?
                } else {
                    toml::Value::Table(toml::map::Map::new())
                };

                set_toml_node(&mut table, key, new_val)?;
                self.ensure_dir()?;
                fs::write(&path, toml::to_string_pretty(&table)?)?;
            }
            #[cfg(feature = "json")]
            Format::Json => {
                let new_val = serde_json::to_value(value)?;

                let mut json: serde_json::Value = if path.exists() {
                    let content = fs::read_to_string(&path)?;
                    serde_json::from_str(&content)?
                } else {
                    serde_json::Value::Object(serde_json::Map::new())
                };

                set_json_node(&mut json, key, new_val)?;
                self.ensure_dir()?;
                fs::write(&path, serde_json::to_string_pretty(&json)?)?;
            }
            #[cfg(feature = "yaml")]
            Format::Yaml => {
                let new_val = serde_yaml::to_value(value)?;

                let mut yaml: serde_yaml::Value = if path.exists() {
                    let content = fs::read_to_string(&path)?;
                    serde_yaml::from_str(&content)?
                } else {
                    serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
                };

                set_yaml_node(&mut yaml, key, new_val)?;
                self.ensure_dir()?;
                fs::write(&path, serde_yaml::to_string(&yaml)?)?;
            }
        }

        Ok(())
    }

    /// Delete the config file. The directory is kept.
    pub fn delete_file(&self) -> Result<(), DotCfgError> {
        let path = self.file_path()?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Delete the entire config directory and all its contents.
    pub fn delete_dir(&self) -> Result<(), DotCfgError> {
        let dir = self.dir()?;
        if dir.exists() {
            fs::remove_dir_all(dir)?;
        }
        Ok(())
    }
}

// TOML helpers

/// Look up the raw value node at `key`. Shared path logic behind
/// [`DotCfg::get`] (which stringifies the node) and [`DotCfg::get_as`]
/// (which deserializes it).
#[cfg(feature = "toml")]
fn get_toml_node<'a>(value: &'a toml::Value, key: &str) -> Result<&'a toml::Value, DotCfgError> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();

    match parts.as_slice() {
        [field] => value.get(field),

        [section, field] => value.get(section).and_then(|s| s.get(field)),

        _ => return Err(DotCfgError::InvalidKey(key.to_string())),
    }
    .ok_or_else(|| DotCfgError::KeyNotFound(key.to_string()))
}

#[cfg(feature = "toml")]
fn get_toml_value(value: &toml::Value, key: &str) -> Result<String, DotCfgError> {
    get_toml_node(value, key).map(toml_val_to_string)
}

/// Write a raw value node at `key`, creating the intermediate table for a
/// `section.field` key. Shared by [`DotCfg::set`] and [`DotCfg::set_val`].
#[cfg(feature = "toml")]
fn set_toml_node(
    value: &mut toml::Value,
    key: &str,
    new_val: toml::Value,
) -> Result<(), DotCfgError> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();
    let table = value
        .as_table_mut()
        .ok_or_else(|| DotCfgError::NotATable("root".to_string()))?;

    match parts.as_slice() {
        [field] => {
            table.insert(field.to_string(), new_val);
        }
        [section, field] => {
            let section_val = table
                .entry(section.to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

            let section_table = section_val
                .as_table_mut()
                .ok_or_else(|| DotCfgError::NotATable(section.to_string()))?;

            section_table.insert(field.to_string(), new_val);
        }
        _ => return Err(DotCfgError::InvalidKey(key.to_string())),
    }

    Ok(())
}

#[cfg(feature = "toml")]
fn set_toml_value(value: &mut toml::Value, key: &str, new_val: &str) -> Result<(), DotCfgError> {
    set_toml_node(value, key, toml::Value::String(new_val.to_string()))
}

#[cfg(feature = "toml")]
fn toml_val_to_string(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        toml::Value::Datetime(d) => d.to_string(),
        toml::Value::Array(a) => {
            toml::to_string(&toml::Value::Array(a.clone())).unwrap_or_default()
        }
        toml::Value::Table(t) => toml::to_string(t).unwrap_or_default(),
    }
}

// JSON helpers
/// JSON counterpart of [`get_toml_node`].
#[cfg(feature = "json")]
fn get_json_node<'a>(
    value: &'a serde_json::Value,
    key: &str,
) -> Result<&'a serde_json::Value, DotCfgError> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();

    match parts.as_slice() {
        [field] => value.get(field),

        [section, field] => value.get(section).and_then(|s| s.get(field)),

        _ => return Err(DotCfgError::InvalidKey(key.to_string())),
    }
    .ok_or_else(|| DotCfgError::KeyNotFound(key.to_string()))
}

#[cfg(feature = "json")]
fn get_json_value(value: &serde_json::Value, key: &str) -> Result<String, DotCfgError> {
    get_json_node(value, key).map(json_val_to_string)
}

/// JSON counterpart of [`set_toml_node`].
#[cfg(feature = "json")]
fn set_json_node(
    value: &mut serde_json::Value,
    key: &str,
    new_val: serde_json::Value,
) -> Result<(), DotCfgError> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();
    let obj = value
        .as_object_mut()
        .ok_or_else(|| DotCfgError::NotATable("root".to_string()))?;

    match parts.as_slice() {
        [field] => {
            obj.insert(field.to_string(), new_val);
        }
        [section, field] => {
            let section_val = obj
                .entry(section.to_string())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

            let section_obj = section_val
                .as_object_mut()
                .ok_or_else(|| DotCfgError::NotATable(section.to_string()))?;

            section_obj.insert(field.to_string(), new_val);
        }
        _ => return Err(DotCfgError::InvalidKey(key.to_string())),
    }

    Ok(())
}

#[cfg(feature = "json")]
fn set_json_value(
    value: &mut serde_json::Value,
    key: &str,
    new_val: &str,
) -> Result<(), DotCfgError> {
    set_json_node(value, key, serde_json::Value::String(new_val.to_string()))
}

#[cfg(feature = "json")]
fn json_val_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_default()
        }
    }
}

// YAML helpers
/// YAML counterpart of [`get_toml_node`].
#[cfg(feature = "yaml")]
fn get_yaml_node<'a>(
    value: &'a serde_yaml::Value,
    key: &str,
) -> Result<&'a serde_yaml::Value, DotCfgError> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();

    match parts.as_slice() {
        [field] => value.get(field),

        [section, field] => value.get(section).and_then(|s| s.get(field)),

        _ => return Err(DotCfgError::InvalidKey(key.to_string())),
    }
    .ok_or_else(|| DotCfgError::KeyNotFound(key.to_string()))
}

#[cfg(feature = "yaml")]
fn get_yaml_value(value: &serde_yaml::Value, key: &str) -> Result<String, DotCfgError> {
    get_yaml_node(value, key).map(yaml_val_to_string)
}

/// YAML counterpart of [`set_toml_node`].
#[cfg(feature = "yaml")]
fn set_yaml_node(
    value: &mut serde_yaml::Value,
    key: &str,
    new_val: serde_yaml::Value,
) -> Result<(), DotCfgError> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();
    let map = value
        .as_mapping_mut()
        .ok_or_else(|| DotCfgError::NotATable("root".to_string()))?;

    match parts.as_slice() {
        [field] => {
            map.insert(serde_yaml::Value::String(field.to_string()), new_val);
        }
        [section, field] => {
            let section_val = map
                .entry(serde_yaml::Value::String(section.to_string()))
                .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));

            let section_map = section_val
                .as_mapping_mut()
                .ok_or_else(|| DotCfgError::NotATable(section.to_string()))?;

            section_map.insert(serde_yaml::Value::String(field.to_string()), new_val);
        }
        _ => return Err(DotCfgError::InvalidKey(key.to_string())),
    }

    Ok(())
}

#[cfg(feature = "yaml")]
fn set_yaml_value(
    value: &mut serde_yaml::Value,
    key: &str,
    new_val: &str,
) -> Result<(), DotCfgError> {
    set_yaml_node(value, key, serde_yaml::Value::String(new_val.to_string()))
}

#[cfg(feature = "yaml")]
fn yaml_val_to_string(value: &serde_yaml::Value) -> String {
    match value {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Null => "null".to_string(),
        // Sequences, mappings and tagged values are re-emitted as YAML;
        // `to_string` appends a trailing newline we don't want in a `get()` result.
        _ => serde_yaml::to_string(value)
            .unwrap_or_default()
            .trim_end()
            .to_string(),
    }
}

// Unit tests for private helpers
#[cfg(test)]
mod unit_tests {
    use super::*;

    #[cfg(feature = "toml")]
    #[test]
    fn toml_val_to_string_variants() {
        // `get()` returns String for all types, so non-strings are stringified
        assert_eq!(toml_val_to_string(&toml::Value::String("hi".into())), "hi");
        assert_eq!(toml_val_to_string(&toml::Value::Integer(42)), "42");
        assert_eq!(toml_val_to_string(&toml::Value::Boolean(true)), "true");
    }

    #[cfg(feature = "toml")]
    #[test]
    fn get_set_toml_helper() {
        // Helpers are tested in-memory to avoid creating temp files
        // and to keep tests fast and isolated from the filesystem.
        let mut val = toml::Value::Table(toml::map::Map::new());
        set_toml_value(&mut val, "username", "tayo").unwrap();
        assert_eq!(get_toml_value(&val, "username").unwrap(), "tayo");
        set_toml_value(&mut val, "user.username", "jane").unwrap();
        assert_eq!(get_toml_value(&val, "user.username").unwrap(), "jane");
        assert!(get_toml_value(&val, "missing").is_err());
    }

    #[cfg(feature = "toml")]
    #[test]
    fn toml_node_helpers_roundtrip_typed_values() {
        // `set_toml_node`/`get_toml_node` keep the native value type, which is
        // what lets `set_val`/`get_as` avoid a string round trip.
        let mut val = toml::Value::Table(toml::map::Map::new());

        set_toml_node(&mut val, "port", toml::Value::Integer(8080)).unwrap();
        set_toml_node(&mut val, "features.auto_update", toml::Value::Boolean(true)).unwrap();

        let port: u16 = get_toml_node(&val, "port")
            .unwrap()
            .clone()
            .try_into()
            .unwrap();
        assert_eq!(port, 8080);
        assert!(
            get_toml_node(&val, "features.auto_update")
                .unwrap()
                .as_bool()
                .unwrap()
        );

        // stringifying still works on the same nodes — `get()` is unchanged
        assert_eq!(get_toml_value(&val, "port").unwrap(), "8080");
        assert!(get_toml_node(&val, "missing").is_err());
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_val_to_string_variants() {
        assert_eq!(
            json_val_to_string(&serde_json::Value::String("hi".into())),
            "hi"
        );
        assert_eq!(
            json_val_to_string(&serde_json::Value::Number(42.into())),
            "42"
        );
        assert_eq!(json_val_to_string(&serde_json::Value::Bool(false)), "false");
    }

    #[cfg(feature = "json")]
    #[test]
    fn get_set_json_helper() {
        let mut val = serde_json::Value::Object(serde_json::Map::new());
        set_json_value(&mut val, "username", "tayo").unwrap();
        assert_eq!(get_json_value(&val, "username").unwrap(), "tayo");
        set_json_value(&mut val, "user.username", "jane").unwrap();
        assert_eq!(get_json_value(&val, "user.username").unwrap(), "jane");
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_node_helpers_roundtrip_typed_values() {
        let mut val = serde_json::Value::Object(serde_json::Map::new());

        set_json_node(&mut val, "weights", serde_json::json!([1, 2, 3])).unwrap();
        set_json_node(
            &mut val,
            "features.auto_update",
            serde_json::Value::Bool(true),
        )
        .unwrap();

        let weights: Vec<i32> =
            serde_json::from_value(get_json_node(&val, "weights").unwrap().clone()).unwrap();
        assert_eq!(weights, vec![1, 2, 3]);
        assert!(
            get_json_node(&val, "features.auto_update")
                .unwrap()
                .as_bool()
                .unwrap()
        );
        assert!(get_json_node(&val, "missing").is_err());
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn yaml_val_to_string_variants() {
        assert_eq!(
            yaml_val_to_string(&serde_yaml::Value::String("hi".into())),
            "hi"
        );
        assert_eq!(
            yaml_val_to_string(&serde_yaml::Value::Number(42.into())),
            "42"
        );
        assert_eq!(yaml_val_to_string(&serde_yaml::Value::Bool(false)), "false");
        assert_eq!(yaml_val_to_string(&serde_yaml::Value::Null), "null");
    }

    #[cfg(feature = "yaml")]
    #[test]
    fn get_set_yaml_helper() {
        let mut val = serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
        set_yaml_value(&mut val, "username", "tayo").unwrap();
        assert_eq!(get_yaml_value(&val, "username").unwrap(), "tayo");
        set_yaml_value(&mut val, "user.username", "jane").unwrap();
        assert_eq!(get_yaml_value(&val, "user.username").unwrap(), "jane");
        assert!(get_yaml_value(&val, "missing").is_err());
    }
    #[cfg(feature = "yaml")]
    #[test]
    fn yaml_node_helpers_roundtrip_typed_values() {
        let mut val = serde_yaml::Value::Mapping(serde_yaml::Mapping::new());

        set_yaml_node(
            &mut val,
            "plugins",
            serde_yaml::Value::Sequence(vec![
                serde_yaml::Value::String("fmt".into()),
                serde_yaml::Value::String("lint".into()),
            ]),
        )
        .unwrap();
        set_yaml_node(
            &mut val,
            "features.auto_update",
            serde_yaml::Value::Bool(true),
        )
        .unwrap();

        let plugins: Vec<String> =
            serde_yaml::from_value(get_yaml_node(&val, "plugins").unwrap().clone()).unwrap();
        assert_eq!(plugins, vec!["fmt", "lint"]);
        assert!(
            get_yaml_node(&val, "features.auto_update")
                .unwrap()
                .as_bool()
                .unwrap()
        );
        assert!(get_yaml_node(&val, "missing").is_err());
    }
}
