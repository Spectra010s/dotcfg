//! # dotcfg
//!
//! Flexible config management for Rust apps.
//!
//! - Choose between `~/.toolname/` or `~/.config/toolname/`
//! - TOML or JSON format (feature-gated)
//! - Load, save, get, set — full or per-key
//! - Flat (`username`) and nested (`user.username`) key support
//! - Returns `None` if config doesn't exist — no magic auto-create unless you want it
//!
//! ## Features
//!
//! - `toml` (default) — enables TOML support
//! - `json` — enables JSON support
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
//!     Ok(())
//! }
//! ```

pub mod error;

use std::{fs, path::PathBuf};

use error::DotCfgError;
use serde::{Deserialize, Serialize};

#[cfg(not(any(feature = "toml", feature = "json")))]
compile_error!("dotcfg requires at least one of the `toml` or `json` features to be enabled");

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

#[cfg(feature = "toml")]
fn get_toml_value(value: &toml::Value, key: &str) -> Result<String, DotCfgError> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();

    match parts.as_slice() {
        [field] => value
            .get(field)
            .map(toml_val_to_string)
            .ok_or_else(|| DotCfgError::KeyNotFound(key.to_string())),

        [section, field] => value
            .get(section)
            .and_then(|s| s.get(field))
            .map(toml_val_to_string)
            .ok_or_else(|| DotCfgError::KeyNotFound(key.to_string())),

        _ => Err(DotCfgError::InvalidKey(key.to_string())),
    }
}

#[cfg(feature = "toml")]
fn set_toml_value(value: &mut toml::Value, key: &str, new_val: &str) -> Result<(), DotCfgError> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();
    let table = value
        .as_table_mut()
        .ok_or_else(|| DotCfgError::NotATable("root".to_string()))?;

    match parts.as_slice() {
        [field] => {
            table.insert(field.to_string(), toml::Value::String(new_val.to_string()));
        }
        [section, field] => {
            let section_val = table
                .entry(section.to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

            let section_table = section_val
                .as_table_mut()
                .ok_or_else(|| DotCfgError::NotATable(section.to_string()))?;

            section_table.insert(field.to_string(), toml::Value::String(new_val.to_string()));
        }
        _ => return Err(DotCfgError::InvalidKey(key.to_string())),
    }

    Ok(())
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
#[cfg(feature = "json")]
fn get_json_value(value: &serde_json::Value, key: &str) -> Result<String, DotCfgError> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();

    match parts.as_slice() {
        [field] => value
            .get(field)
            .map(json_val_to_string)
            .ok_or_else(|| DotCfgError::KeyNotFound(key.to_string())),

        [section, field] => value
            .get(section)
            .and_then(|s| s.get(field))
            .map(json_val_to_string)
            .ok_or_else(|| DotCfgError::KeyNotFound(key.to_string())),

        _ => Err(DotCfgError::InvalidKey(key.to_string())),
    }
}

#[cfg(feature = "json")]
fn set_json_value(
    value: &mut serde_json::Value,
    key: &str,
    new_val: &str,
) -> Result<(), DotCfgError> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();
    let obj = value
        .as_object_mut()
        .ok_or_else(|| DotCfgError::NotATable("root".to_string()))?;

    match parts.as_slice() {
        [field] => {
            obj.insert(
                field.to_string(),
                serde_json::Value::String(new_val.to_string()),
            );
        }
        [section, field] => {
            let section_val = obj
                .entry(section.to_string())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

            let section_obj = section_val
                .as_object_mut()
                .ok_or_else(|| DotCfgError::NotATable(section.to_string()))?;

            section_obj.insert(
                field.to_string(),
                serde_json::Value::String(new_val.to_string()),
            );
        }
        _ => return Err(DotCfgError::InvalidKey(key.to_string())),
    }

    Ok(())
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
}
