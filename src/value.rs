//! The internal, format-independent value tree.
//!
//! Every supported format is parsed into one [`Value`], and every read or
//! mutation (`get`, `get_as`, `set`, `set_val`, `load`) works on that tree.
//! Only [`parse`] and [`render`] know about TOML, JSON or YAML.
//!
//! The tree is [`serde_json::Value`]: it is Serde-native, covers everything
//! dotcfg needs (null, bool, integers/floats, strings, arrays, nested maps),
//! and stays internal — it never appears in the public API.
//!
//! Format quirks:
//!
//! - **Empty documents** (zero bytes, whitespace only, a bare YAML `---`, a
//!   `null` document) become an empty map. Malformed documents stay errors.
//! - **TOML datetimes** are kept in the tree under toml's private one-key map
//!   representation, so they survive a read/modify/write cycle unchanged.
//! - **`null`** has no TOML equivalent, so map entries holding it are omitted
//!   when rendering TOML — the same thing toml does for `None` struct fields.

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Map;

use crate::{Error, Format};

pub(crate) use serde_json::Value;

/// Key toml uses to smuggle a datetime through Serde's data model.
#[cfg(feature = "toml")]
const TOML_DATETIME_KEY: &str = "$__toml_private_datetime";

/// The empty tree: a configuration that exists but holds no values.
pub(crate) fn empty() -> Value {
    Value::Object(Map::new())
}

/// Parse `content` in `format` into the common tree.
pub(crate) fn parse(format: &Format, content: &str) -> Result<Value, Error> {
    if content.trim().is_empty() {
        return Ok(empty());
    }

    let value: Value = match format {
        #[cfg(feature = "toml")]
        Format::Toml => toml::from_str(content)?,
        #[cfg(feature = "json")]
        Format::Json => serde_json::from_str(content)?,
        #[cfg(feature = "yaml")]
        Format::Yaml => serde_yaml_ng::from_str(content)?,
    };

    // A document that is only `null` (YAML `~`, `---`) is empty, not invalid.
    Ok(if value.is_null() { empty() } else { value })
}

/// Serialize the tree into `format`'s text.
pub(crate) fn render(format: &Format, value: &Value) -> Result<String, Error> {
    Ok(match format {
        #[cfg(feature = "toml")]
        Format::Toml => toml::to_string_pretty(&to_toml(value)?)?,
        #[cfg(feature = "json")]
        Format::Json => serde_json::to_string_pretty(value)?,
        #[cfg(feature = "yaml")]
        Format::Yaml => serde_yaml_ng::to_string(value)?,
    })
}

/// Serialize any `T` into the tree.
pub(crate) fn to_value<T: Serialize>(value: T) -> Result<Value, Error> {
    serde_json::to_value(value).map_err(|e| Error::Serialize(e.to_string()))
}

/// Deserialize the tree (or a node of it) into `T`.
pub(crate) fn from_value<T: DeserializeOwned>(value: Value) -> Result<T, Error> {
    serde_json::from_value(value).map_err(|e| Error::Deserialize(e.to_string()))
}

/// Look up the node at `key`: `field` or `section.field`.
pub(crate) fn get_node<'a>(value: &'a Value, key: &str) -> Result<&'a Value, Error> {
    match key.split_once('.') {
        None => value.get(key),
        Some((section, field)) => value.get(section).and_then(|s| s.get(field)),
    }
    .ok_or_else(|| Error::KeyNotFound(key.to_string()))
}

/// Write `new_val` at `key`, creating the intermediate map for `section.field`.
/// Everything else in the tree is preserved.
pub(crate) fn set_node(value: &mut Value, key: &str, new_val: Value) -> Result<(), Error> {
    let root = value
        .as_object_mut()
        .ok_or_else(|| Error::NotATable("root".to_string()))?;

    match key.split_once('.') {
        None => {
            root.insert(key.to_string(), new_val);
        }
        Some((section, field)) => {
            let section_map = root
                .entry(section.to_string())
                .or_insert_with(empty)
                .as_object_mut()
                .ok_or_else(|| Error::NotATable(section.to_string()))?;

            section_map.insert(field.to_string(), new_val);
        }
    }

    Ok(())
}

/// The `String` that `DotCfg::get` returns for a node.
///
/// Scalars are stringified; arrays and maps are re-emitted in `format`.
pub(crate) fn display(format: &Format, value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        #[cfg(feature = "toml")]
        Value::Object(map) if datetime(map).is_some() => {
            datetime(map).unwrap_or_default().to_string()
        }
        Value::Array(_) | Value::Object(_) => match format {
            #[cfg(feature = "toml")]
            Format::Toml => match to_toml(value) {
                // Arrays have no top-level TOML document form; use the inline form.
                Ok(v @ toml::Value::Array(_)) => v.to_string(),
                Ok(v) => toml::to_string(&v).unwrap_or_default(),
                Err(_) => String::new(),
            },
            #[cfg(feature = "json")]
            Format::Json => serde_json::to_string(value).unwrap_or_default(),
            // `to_string` appends a trailing newline we don't want in a `get()` result.
            #[cfg(feature = "yaml")]
            Format::Yaml => serde_yaml_ng::to_string(value)
                .unwrap_or_default()
                .trim_end()
                .to_string(),
        },
    }
}

#[cfg(feature = "toml")]
fn datetime(map: &Map<String, Value>) -> Option<&str> {
    match (map.len(), map.get(TOML_DATETIME_KEY)) {
        (1, Some(Value::String(s))) => Some(s),
        _ => None,
    }
}

/// Convert the tree to a `toml::Value`, honouring TOML's lack of `null`.
#[cfg(feature = "toml")]
fn to_toml(value: &Value) -> Result<toml::Value, Error> {
    use serde::ser::Error as _;

    Ok(match value {
        Value::Null => return Err(toml::ser::Error::custom("TOML has no null value").into()),
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => toml::Value::Integer(i),
            _ if n.is_u64() => {
                return Err(toml::ser::Error::custom(format!(
                    "{n} is out of range for a TOML integer"
                ))
                .into());
            }
            (None, Some(f)) => toml::Value::Float(f),
            (None, None) => unreachable!("a JSON number is an integer or a float"),
        },
        Value::String(s) => toml::Value::String(s.clone()),
        Value::Array(items) => toml::Value::Array(
            items
                .iter()
                .map(to_toml)
                .collect::<Result<Vec<_>, Error>>()?,
        ),
        Value::Object(map) => match datetime(map).map(str::parse) {
            Some(Ok(dt)) => toml::Value::Datetime(dt),
            _ => {
                let mut table = toml::map::Map::new();
                for (k, v) in map {
                    if !v.is_null() {
                        table.insert(k.clone(), to_toml(v)?);
                    }
                }
                toml::Value::Table(table)
            }
        },
    })
}
