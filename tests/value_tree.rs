//! Behavior of the unified value tree across TOML, JSON and YAML:
//! missing / empty / whitespace-only / populated / malformed files, typed and
//! nested values, mutation and round trips.

use dotcfg::{DotCfg, Error};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
struct AllOptional {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    port: Option<u16>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Required {
    name: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Nested {
    name: String,
    nested: Inner,
    tags: Vec<String>,
    ratio: f64,
    on: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Inner {
    depth: u8,
}

fn temp_dir(suffix: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "dotcfg_test_tree_{}_{}_{}",
        suffix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&path);
    path
}

/// Generates the same suite for one format. `$empty_docs` are documents that
/// are valid in that format and hold no values; `$bad` is malformed.
macro_rules! tree_suite {
    ($m:ident, $feature:literal, $build:expr, $empty_docs:expr, $populated:expr, $bad:expr) => {
        #[cfg(feature = $feature)]
        mod $m {
            use super::*;

            fn cfg(suffix: &str) -> (DotCfg, PathBuf) {
                let dir = temp_dir(&format!("{}_{}", stringify!($m), suffix));
                let cfg = $build(DotCfg::new("dotcfg_tree_test").at_dir(&dir));
                (cfg, dir)
            }

            fn write(cfg: &DotCfg, content: &str) {
                let path = cfg.file_path().unwrap();
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, content).unwrap();
            }

            #[test]
            fn missing_file() {
                let (cfg, dir) = cfg("missing");
                assert!(cfg.load::<AllOptional>().unwrap().is_none());
                assert!(matches!(
                    cfg.load_or_error::<AllOptional>().unwrap_err(),
                    Error::NotFound
                ));
                assert!(matches!(cfg.get("name").unwrap_err(), Error::NotFound));
                assert!(matches!(
                    cfg.get_as::<String>("name").unwrap_err(),
                    Error::NotFound
                ));
                // load_or_default still creates the file
                let d: AllOptional = cfg.load_or_default().unwrap();
                assert_eq!(d, AllOptional::default());
                assert!(cfg.exists().unwrap());
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn empty_documents_are_an_empty_tree() {
                let (cfg, dir) = cfg("empty");
                for doc in $empty_docs {
                    write(&cfg, doc);
                    // typed load deserializes the empty tree normally
                    assert_eq!(
                        cfg.load::<AllOptional>().unwrap(),
                        Some(AllOptional::default()),
                        "doc {doc:?}"
                    );
                    // ...and fails only because `Required` needs a field
                    assert!(
                        matches!(cfg.load::<Required>().unwrap_err(), Error::Deserialize(_)),
                        "doc {doc:?}"
                    );
                    // per-key reads use the ordinary missing-key behavior
                    assert!(matches!(
                        cfg.get("name").unwrap_err(),
                        Error::KeyNotFound(_)
                    ));
                    assert!(matches!(
                        cfg.get_as::<u16>("a.b").unwrap_err(),
                        Error::KeyNotFound(_)
                    ));
                    // and mutating works, starting from nothing
                    cfg.set("name", "x").unwrap();
                    assert_eq!(cfg.get("name").unwrap(), "x");
                }
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn malformed_is_not_empty() {
                let (cfg, dir) = cfg("bad");
                write(&cfg, $bad);
                assert!(cfg.load::<AllOptional>().is_err());
                assert!(cfg.get("name").is_err());
                assert!(cfg.get_as::<String>("name").is_err());
                // a mutation must not silently clobber a file it can't parse
                assert!(cfg.set("name", "x").is_err());
                assert!(cfg.set_val("port", 1u16).is_err());
                assert_eq!(
                    std::fs::read_to_string(cfg.file_path().unwrap()).unwrap(),
                    $bad
                );
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn populated_document() {
                let (cfg, dir) = cfg("populated");
                write(&cfg, $populated);
                assert_eq!(
                    cfg.load::<AllOptional>().unwrap(),
                    Some(AllOptional {
                        name: Some("app".into()),
                        port: Some(8080)
                    })
                );
                assert_eq!(cfg.get("name").unwrap(), "app");
                assert_eq!(cfg.get_as::<u16>("port").unwrap(), 8080);
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn typed_and_nested_values_round_trip() {
                let (cfg, dir) = cfg("roundtrip");
                let value = Nested {
                    name: "app".into(),
                    nested: Inner { depth: 3 },
                    tags: vec!["a".into(), "b".into()],
                    ratio: 0.5,
                    on: true,
                };
                cfg.save(&value).unwrap();
                assert_eq!(cfg.load::<Nested>().unwrap(), Some(value));
                assert_eq!(cfg.get_as::<u8>("nested.depth").unwrap(), 3);
                assert_eq!(cfg.get_as::<Vec<String>>("tags").unwrap(), ["a", "b"]);
                assert_eq!(cfg.get_as::<f64>("ratio").unwrap(), 0.5);
                assert_eq!(cfg.get_as::<Inner>("nested").unwrap(), Inner { depth: 3 });
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn mutation_preserves_unrelated_values() {
                let (cfg, dir) = cfg("mutate");
                cfg.set_val("port", 8080u16).unwrap();
                cfg.set_val("tags", vec!["x", "y"]).unwrap();
                cfg.set_val("features.auto", true).unwrap();
                cfg.set("features.name", "f").unwrap();
                cfg.set_val("port", 9090u16).unwrap();

                assert_eq!(cfg.get_as::<u16>("port").unwrap(), 9090);
                assert_eq!(cfg.get_as::<Vec<String>>("tags").unwrap(), ["x", "y"]);
                assert!(cfg.get_as::<bool>("features.auto").unwrap());
                assert_eq!(cfg.get("features.name").unwrap(), "f");
                assert!(matches!(
                    cfg.get("nope").unwrap_err(),
                    Error::KeyNotFound(_)
                ));

                // setting under a scalar is a clear error, and changes nothing
                assert!(matches!(
                    cfg.set("port.sub", "x").unwrap_err(),
                    Error::NotATable(_)
                ));
                assert_eq!(cfg.get_as::<u16>("port").unwrap(), 9090);
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn none_fields_are_omitted_not_errors() {
                let (cfg, dir) = cfg("none");
                let section = AllOptional {
                    name: None,
                    port: Some(1),
                };
                cfg.set_val("section", &section).unwrap();
                assert_eq!(cfg.get_as::<u16>("section.port").unwrap(), 1);
                assert_eq!(cfg.get_as::<AllOptional>("section").unwrap(), section);
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    };
}

tree_suite!(
    toml_fmt,
    "toml",
    |c: DotCfg| c.toml(),
    ["", "  \n\t\n", "# just a comment\n"],
    "name = \"app\"\nport = 8080\n",
    "name = = broken"
);

tree_suite!(
    json_fmt,
    "json",
    |c: DotCfg| c.json(),
    ["", "  \n\t\n", "{}", "null", " { } "],
    "{\"name\": \"app\", \"port\": 8080}",
    "{\"name\": "
);

tree_suite!(
    yaml_fmt,
    "yaml",
    |c: DotCfg| c.yaml(),
    ["", "  \n\t\n", "{}", "---\n", "~", "# comment only\n"],
    "name: app\nport: 8080\n",
    "name: [unclosed"
);

#[cfg(feature = "toml")]
#[test]
fn toml_datetime_survives_mutation() {
    let dir = temp_dir("datetime");
    let cfg = DotCfg::new("dotcfg_tree_test").at_dir(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(cfg.file_path().unwrap(), "when = 1979-05-27T07:32:00Z\n").unwrap();

    assert_eq!(cfg.get("when").unwrap(), "1979-05-27T07:32:00Z");
    cfg.set("other", "x").unwrap();

    let raw = std::fs::read_to_string(cfg.file_path().unwrap()).unwrap();
    assert!(raw.contains("when = 1979-05-27T07:32:00Z"), "{raw}");
    let _ = std::fs::remove_dir_all(dir);
}
