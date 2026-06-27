use thiserror::Error;

#[derive(Error, Debug)]
pub enum DotCfgError {
    #[error("Could not find home directory")]
    NoHomeDir,

    #[error("Config not found. Run setup first.")]
    NotFound,

    #[error("Key not found: '{0}'")]
    KeyNotFound(String),

    #[error("Invalid key: '{0}'. Use 'field' or 'section.field'")]
    InvalidKey(String),

    #[error("'{0}' is not a table")]
    NotATable(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(feature = "toml")]
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[cfg(feature = "toml")]
    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[cfg(feature = "json")]
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
