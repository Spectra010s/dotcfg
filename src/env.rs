//! Deserializing environment-variable overrides.
//!
//! Environment variables are untyped strings, so unlike the file path — where a
//! value already carries its format's native type — an override has to be
//! interpreted on the way out. This module implements a tiny [`serde`]
//! deserializer over a raw `&str` so `get_as::<T>()` can produce the same types
//! from an env var as it does from the config file, without depending on any
//! one format's crate (all three are optional features).
//!
//! Supported targets:
//!
//! - strings and `char`
//! - integers and floats (parsed with [`str::parse`], surrounding space trimmed)
//! - `bool` — `true` / `false`, case-insensitive, plus `1` / `0`
//! - unit-variant enums — the variant name
//! - `Option<T>` — a var that is set is always `Some`
//! - sequences (`Vec<T>`, arrays) — **comma-separated**, e.g. `MYAPP_TAGS=cli,fast`,
//!   with each element trimmed; an empty var is an empty sequence
//!
//! Maps and structs are rejected with a clear error rather than silently
//! failing — express those as separate nested keys instead.

use std::fmt;

use serde::de::{
    self, DeserializeSeed, Deserializer, IntoDeserializer, SeqAccess, Visitor,
    value::StrDeserializer,
};

use crate::error::DotCfgError;

/// Deserialize the raw value of env var `var` into `T`.
///
/// `var` is only carried through for the error message.
pub(crate) fn from_env_str<T>(var: &str, raw: &str) -> Result<T, DotCfgError>
where
    T: serde::de::DeserializeOwned,
{
    T::deserialize(EnvStr { raw })
        .map_err(|err| DotCfgError::EnvParse(var.to_string(), err.to_string()))
}

/// Error raised while interpreting a raw env value.
#[derive(Debug)]
pub(crate) struct EnvError(String);

impl fmt::Display for EnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EnvError {}

impl de::Error for EnvError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        EnvError(msg.to_string())
    }
}

/// A [`Deserializer`] over one raw environment value.
struct EnvStr<'a> {
    raw: &'a str,
}

impl EnvStr<'_> {
    fn parse<T>(&self, what: &str) -> Result<T, EnvError>
    where
        T: std::str::FromStr,
    {
        self.raw
            .trim()
            .parse()
            .map_err(|_| EnvError(format!("expected {}, got '{}'", what, self.raw)))
    }

    fn unsupported<T>(&self, what: &str) -> Result<T, EnvError> {
        Err(EnvError(format!(
            "cannot read {} from an environment variable; use nested keys instead",
            what
        )))
    }
}

/// `deserialize_$method` → `str::parse` → `visit_$visit`.
macro_rules! parsed {
    ($method:ident, $visit:ident, $ty:ty) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value, EnvError>
        where
            V: Visitor<'de>,
        {
            let parsed: $ty = self.parse(stringify!($ty))?;
            visitor.$visit(parsed)
        }
    };
}

impl<'de> Deserializer<'de> for EnvStr<'_> {
    type Error = EnvError;

    /// Env values have no inherent type — self-describing formats get the string.
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.raw)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        match self.raw.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => visitor.visit_bool(true),
            "false" | "0" => visitor.visit_bool(false),
            _ => Err(EnvError(format!("expected bool, got '{}'", self.raw))),
        }
    }

    parsed!(deserialize_i8, visit_i8, i8);
    parsed!(deserialize_i16, visit_i16, i16);
    parsed!(deserialize_i32, visit_i32, i32);
    parsed!(deserialize_i64, visit_i64, i64);
    parsed!(deserialize_i128, visit_i128, i128);
    parsed!(deserialize_u8, visit_u8, u8);
    parsed!(deserialize_u16, visit_u16, u16);
    parsed!(deserialize_u32, visit_u32, u32);
    parsed!(deserialize_u64, visit_u64, u64);
    parsed!(deserialize_u128, visit_u128, u128);
    parsed!(deserialize_f32, visit_f32, f32);
    parsed!(deserialize_f64, visit_f64, f64);
    parsed!(deserialize_char, visit_char, char);

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.raw)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.raw)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bytes(self.raw.as_bytes())
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bytes(self.raw.as_bytes())
    }

    /// A var that is set is always `Some` — absence is handled by the caller.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    /// Comma-separated: `MYAPP_TAGS=cli,fast` → `["cli", "fast"]`.
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        let parts: Vec<&str> = if self.raw.trim().is_empty() {
            Vec::new()
        } else {
            self.raw.split(',').map(str::trim).collect()
        };

        visitor.visit_seq(CommaSeparated {
            iter: parts.into_iter(),
        })
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, _visitor: V) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        self.unsupported("a map")
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        self.unsupported(&format!("struct `{name}`"))
    }

    /// Unit variants only — the raw value is the variant name.
    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        let de: StrDeserializer<'_, EnvError> = self.raw.trim().into_deserializer();
        visitor.visit_enum(de)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.raw)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, EnvError>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

/// Sequence access over the comma-separated parts of one env value.
struct CommaSeparated<'a> {
    iter: std::vec::IntoIter<&'a str>,
}

impl<'de> SeqAccess<'de> for CommaSeparated<'_> {
    type Error = EnvError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, EnvError>
    where
        T: DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(part) => seed.deserialize(EnvStr { raw: part }).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

#[cfg(test)]
mod tests {
    use super::from_env_str;

    #[test]
    fn primitives() {
        assert_eq!(from_env_str::<String>("V", "hello").unwrap(), "hello");
        assert_eq!(from_env_str::<u16>("V", "9000").unwrap(), 9000);
        assert_eq!(from_env_str::<i32>("V", "-3").unwrap(), -3);
        assert_eq!(from_env_str::<f64>("V", "0.75").unwrap(), 0.75);
        // surrounding whitespace is tolerated on parsed types
        assert_eq!(from_env_str::<u16>("V", "  9000 ").unwrap(), 9000);
    }

    #[test]
    fn bools() {
        for raw in ["true", "TRUE", "1"] {
            assert!(from_env_str::<bool>("V", raw).unwrap(), "{raw}");
        }
        for raw in ["false", "False", "0"] {
            assert!(!from_env_str::<bool>("V", raw).unwrap(), "{raw}");
        }
        assert!(from_env_str::<bool>("V", "yes").is_err());
    }

    #[test]
    fn comma_separated_sequences() {
        assert_eq!(
            from_env_str::<Vec<String>>("V", "cli,fast").unwrap(),
            vec!["cli".to_string(), "fast".to_string()]
        );
        // elements are trimmed
        assert_eq!(
            from_env_str::<Vec<String>>("V", "cli, fast").unwrap(),
            vec!["cli".to_string(), "fast".to_string()]
        );
        assert_eq!(
            from_env_str::<Vec<i32>>("V", "1,2,3").unwrap(),
            vec![1, 2, 3]
        );
        // an empty var is an empty sequence, not `[""]`
        assert_eq!(
            from_env_str::<Vec<String>>("V", "").unwrap(),
            Vec::<String>::new()
        );
        // a bad element fails the whole read
        assert!(from_env_str::<Vec<i32>>("V", "1,nope").is_err());
    }

    #[test]
    fn options_and_errors() {
        assert_eq!(
            from_env_str::<Option<u16>>("V", "8080").unwrap(),
            Some(8080)
        );

        // malformed values are an error, never a panic, and name the var
        let err = from_env_str::<u16>("MYAPP_PORT", "notanumber").unwrap_err();
        assert!(err.to_string().contains("MYAPP_PORT"), "{err}");
        assert!(err.to_string().contains("notanumber"), "{err}");
    }

    #[test]
    fn structs_are_rejected_with_a_clear_error() {
        #[derive(serde::Deserialize, Debug)]
        #[allow(dead_code)]
        struct Server {
            host: String,
        }

        let err = from_env_str::<Server>("V", "host=x").unwrap_err();
        assert!(err.to_string().contains("nested keys"), "{err}");
    }
}
