//! Error types for `dotcfg`.

use thiserror::Error;

/// Errors returned by [`DotCfg`](crate::DotCfg) operations.
#[derive(Error, Debug)]
pub enum DotCfgError {
    /// Could not find home directory (`$HOME` not set or resolvable).
    #[error("Could not find home directory")]
    NoHomeDir,

    /// Config file does not exist at the expected path.
    #[error("Config not found. Run setup first.")]
    NotFound,

    /// Requested key does not exist in the config file.
    #[error("Key not found: '{0}'")]
    KeyNotFound(String),

    /// Key syntax is invalid. Use `field` or `section.field`.
    #[error("Invalid key: '{0}'. Use 'field' or 'section.field'")]
    InvalidKey(String),

    /// Expected a table/section but found a different value type.
    #[error("'{0}' is not a table")]
    NotATable(String),

    /// Underlying IO error (read/write/create dir).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parsing failed.
    #[cfg(feature = "toml")]
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// TOML serialization failed.
    #[cfg(feature = "toml")]
    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    /// JSON (de)serialization failed.
    #[cfg(feature = "json")]
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// YAML (de)serialization failed.
    #[cfg(feature = "yaml")]
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}
