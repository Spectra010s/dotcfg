//! Layered configuration: precedence, merging, environment overrides,
//! writes, source access, mixed formats, and empty/missing/malformed sources.

use dotcfg::{ConfigStack, DotCfg, Error};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Database {
    host: String,
    port: u16,
    pool_size: u16,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct AppConfig {
    database: Database,
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Flat {
    api_key: String,
    port: u16,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
struct StrictPort {
    #[allow(dead_code)]
    port: u16,
}

fn nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

fn temp_dir(suffix: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "dotcfg_test_stack_{suffix}_{}_{}",
        std::process::id(),
        nanos()
    ));
    let _ = std::fs::remove_dir_all(&path);
    path
}

fn write_file(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn set_env(var: &str, val: &str) {
    unsafe { std::env::set_var(var, val) }
}

/// Unique env prefix per test, so parallel tests can't see each other's vars.
fn prefix(suffix: &str) -> String {
    format!(
        "DOTCFG_T_STACK_{}_{}_{}",
        suffix.to_ascii_uppercase(),
        std::process::id(),
        nanos()
    )
}

/// Same layered suite for one format. `$defaults` is the full lower layer,
/// `$project` overrides one nested key, `$project_tags` also replaces the
/// array, `$bad` is malformed, `$scalar` a scalar document for the
/// scalar-blocking-nesting case.
macro_rules! stack_suite {
    ($m:ident, $feature:literal, $build:expr, $defaults:expr, $project:expr, $project_tags:expr, $bad:expr, $scalar:expr) => {
        #[cfg(feature = $feature)]
        mod $m {
            use super::*;

            fn layer(dir: &Path, name: &str, content: &str) -> (String, DotCfg) {
                let cfg = $build(DotCfg::new("dotcfg_stack_test").at_dir(dir.join(name)));
                write_file(&cfg.file_path().unwrap(), content);
                (name.to_string(), cfg)
            }

            fn stack(defaults: &str, project: &str) -> (ConfigStack, PathBuf) {
                let dir = temp_dir(stringify!($m));
                let (dname, dcfg) = layer(&dir, "defaults", defaults);
                let (pname, pcfg) = layer(&dir, "project", project);
                let stack = ConfigStack::new()
                    .layer(dname, dcfg)
                    .layer(pname, pcfg)
                    .write_to("project")
                    .build()
                    .unwrap();
                (stack, dir)
            }

            #[test]
            fn later_layers_override_key_by_key() {
                let (stack, dir) = stack($defaults, $project);
                let cfg: AppConfig = stack.load_or_error().unwrap();
                assert_eq!(cfg.database.host, "db.internal");
                // keys the project says nothing about survive from defaults
                assert_eq!(cfg.database.port, 5432);
                assert_eq!(cfg.database.pool_size, 10);
                assert_eq!(cfg.tags, vec!["a".to_string(), "b".to_string()]);
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn arrays_replace_instead_of_merging() {
                let (stack, dir) = stack($defaults, $project_tags);
                let cfg: AppConfig = stack.load_or_error().unwrap();
                assert_eq!(cfg.tags, vec!["c".to_string()]);
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn get_reads_the_resolved_view() {
                let (stack, dir) = stack($defaults, $project);
                assert_eq!(stack.get("database.host").unwrap(), "db.internal");
                assert_eq!(stack.get("database.port").unwrap(), "5432");
                // canonical form regardless of source format
                assert_eq!(stack.get("tags").unwrap(), "[\"a\",\"b\"]");
                assert!(matches!(
                    stack.get("database.missing").unwrap_err(),
                    Error::KeyNotFound(_)
                ));
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn writes_go_to_the_target_only() {
                let (stack, dir) = stack($defaults, $project);
                stack.set("database.host", "edited").unwrap();
                stack.set_val("added", 7u16).unwrap();

                // the target file changed...
                assert_eq!(
                    stack
                        .source("project")
                        .unwrap()
                        .get("database.host")
                        .unwrap(),
                    "edited"
                );
                // ...the lower layer did not
                assert_eq!(
                    stack
                        .source("defaults")
                        .unwrap()
                        .get("database.host")
                        .unwrap(),
                    "localhost"
                );
                // ...and resolution follows the target
                assert_eq!(stack.get("database.host").unwrap(), "edited");
                assert_eq!(stack.get_as::<u16>("added").unwrap(), 7);
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn read_only_without_write_to() {
                let dir = temp_dir("readonly");
                let (_, dcfg) = layer(&dir, "defaults", $defaults);
                let stack = ConfigStack::new().layer("defaults", dcfg).build().unwrap();
                assert!(matches!(
                    stack.set("a", "b").unwrap_err(),
                    Error::NoWriteTarget
                ));
                assert!(matches!(
                    stack.set_val("a", 1u16).unwrap_err(),
                    Error::NoWriteTarget
                ));
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn source_access_and_unknown_source() {
                let (stack, dir) = stack($defaults, $project);
                assert_eq!(
                    stack
                        .source("project")
                        .unwrap()
                        .get("database.host")
                        .unwrap(),
                    "db.internal"
                );
                assert!(matches!(stack.source("nope"), Err(Error::UnknownSource(_))));
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn missing_files_are_skipped() {
                let dir = temp_dir("missing");
                let (_, pcfg) = layer(&dir, "project", $project);
                let missing = DotCfg::new("dotcfg_stack_test").at_dir(dir.join("nowhere"));
                let stack = ConfigStack::new()
                    .layer("missing", missing)
                    .layer("project", pcfg)
                    .build()
                    .unwrap();
                assert_eq!(stack.get("database.host").unwrap(), "db.internal");
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn empty_files_contribute_nothing() {
                let dir = temp_dir("empty");
                let (_, dcfg) = layer(&dir, "defaults", $defaults);
                let (_, pcfg) = layer(&dir, "project", "");
                let stack = ConfigStack::new()
                    .layer("defaults", dcfg)
                    .layer("project", pcfg)
                    .write_to("project")
                    .build()
                    .unwrap();
                let cfg: AppConfig = stack.load_or_error().unwrap();
                assert_eq!(cfg.database.host, "localhost");
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn malformed_files_are_errors() {
                let dir = temp_dir("bad");
                let (_, dcfg) = layer(&dir, "defaults", $defaults);
                let (_, pcfg) = layer(&dir, "project", $bad);
                let stack = ConfigStack::new()
                    .layer("defaults", dcfg)
                    .layer("project", pcfg)
                    .build()
                    .unwrap();
                assert!(stack.load::<AppConfig>().is_err());
                assert!(stack.get("database.host").is_err());
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn scalar_blocking_nested_env_is_an_error() {
                let p = prefix("notatable");
                set_env(&format!("{p}_PORT__X"), "1");
                let dir = temp_dir("notatable");
                let (_, dcfg) = layer(&dir, "defaults", $scalar);
                let stack = ConfigStack::new()
                    .layer("defaults", dcfg)
                    .with_env_prefix(&p)
                    .build()
                    .unwrap();
                assert!(matches!(
                    stack.load::<AppConfig>().unwrap_err(),
                    Error::NotATable(_)
                ));
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn env_override_wins_over_both_layers() {
                let p = prefix("wins");
                set_env(&format!("{p}_DATABASE__PORT"), "9999");
                set_env(&format!("{p}_DATABASE__HOST"), "envhost");
                let dir = temp_dir("winvenv");
                let (_, dcfg) = layer(&dir, "defaults", $defaults);
                let (_, pcfg) = layer(&dir, "project", $project);
                let stack = ConfigStack::new()
                    .layer("defaults", dcfg)
                    .layer("project", pcfg)
                    .write_to("project")
                    .with_env_prefix(&p)
                    .build()
                    .unwrap();
                // nested env discovery applies to every typed read: get, get_as and load
                // all share the `__` convention
                let cfg: AppConfig = stack.load_or_error().unwrap();
                assert_eq!(cfg.database.port, 9999);
                assert_eq!(cfg.database.host, "envhost");
                // untouched keys still resolve from files
                assert_eq!(cfg.database.pool_size, 10);
                // top-level env vars work identically in get_as and load
                set_env(&format!("{p}_TAGS"), "x,y");
                assert_eq!(
                    stack.get_as::<Vec<String>>("tags").unwrap(),
                    vec!["x".to_string(), "y".to_string()]
                );
                let cfg: AppConfig = stack.load_or_error().unwrap();
                assert_eq!(cfg.tags, vec!["x".to_string(), "y".to_string()]);
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn shadowed_writes() {
                let p = prefix("shadow");
                set_env(&format!("{p}_DATABASE__PORT"), "4000");
                let dir = temp_dir("shadowenv");
                let (_, dcfg) = layer(&dir, "defaults", $defaults);
                let (_, pcfg) = layer(&dir, "project", $project);
                let stack = ConfigStack::new()
                    .layer("defaults", dcfg)
                    .layer("project", pcfg)
                    .write_to("project")
                    .with_env_prefix(&p)
                    .build()
                    .unwrap();
                stack.set_val("database.port", 3000u16).unwrap();
                // the file got the write...
                assert_eq!(
                    stack
                        .source("project")
                        .unwrap()
                        .get_as::<u16>("database.port")
                        .unwrap(),
                    3000
                );
                // ...but env still shadows resolution through load
                let cfg: AppConfig = stack.load_or_error().unwrap();
                assert_eq!(cfg.database.port, 4000);
                let _ = std::fs::remove_dir_all(dir);
            }

            #[test]
            fn save_roundtrip_through_the_target() {
                let (stack, dir) = stack($defaults, $project);
                let saved = AppConfig {
                    database: Database {
                        host: "saved".to_string(),
                        port: 1,
                        pool_size: 2,
                    },
                    tags: vec!["s".to_string()],
                };
                stack.save(&saved).unwrap();
                // the target holds exactly what was given — no merged flattening
                let back: AppConfig = stack.source("project").unwrap().load_or_error().unwrap();
                assert_eq!(back, saved);
                let _ = std::fs::remove_dir_all(dir);
            }

            /// Async reads resolve the same layered view as the sync API.
            #[cfg(feature = "async")]
            #[tokio::test]
            async fn async_reads_match_the_layered_sync_view() {
                let (stack, dir) = stack($defaults, $project_tags);

                assert_eq!(
                    stack.get_async("database.host").await.unwrap(),
                    stack.get("database.host").unwrap()
                );
                assert_eq!(
                    stack.get_as_async::<u16>("database.port").await.unwrap(),
                    5432
                );
                assert_eq!(
                    stack.get_async("tags").await.unwrap(),
                    stack.get("tags").unwrap()
                );

                let loaded: AppConfig = stack.load_async().await.unwrap().unwrap();
                let loaded_or_error: AppConfig = stack.load_or_error_async().await.unwrap();
                let sync_loaded: AppConfig = stack.load().unwrap().unwrap();
                assert_eq!(loaded, sync_loaded);
                assert_eq!(loaded_or_error, sync_loaded);
                assert_eq!(loaded.database.host, "db.internal");
                // keys absent from the project layer survive from defaults
                assert_eq!(loaded.database.pool_size, 10);
                // a project array replaces the lower one instead of appending
                assert_eq!(loaded.tags, vec!["c".to_string()]);

                let _ = std::fs::remove_dir_all(dir);
            }

            /// Environment overrides apply to async reads with the same
            /// precedence and `__` convention as the sync reads.
            #[cfg(feature = "async")]
            #[tokio::test]
            async fn async_env_overrides_win_over_layers() {
                let p = prefix("asyncenv");
                set_env(&format!("{p}_DATABASE__PORT"), "9999");
                let dir = temp_dir("asyncenv");
                let (_, dcfg) = layer(&dir, "defaults", $defaults);
                let (_, pcfg) = layer(&dir, "project", $project);
                let stack = ConfigStack::new()
                    .layer("defaults", dcfg)
                    .layer("project", pcfg)
                    .write_to("project")
                    .with_env_prefix(&p)
                    .build()
                    .unwrap();

                assert_eq!(stack.get_async("database.port").await.unwrap(), "9999");
                assert_eq!(
                    stack.get_as_async::<u16>("database.port").await.unwrap(),
                    9999
                );
                let loaded: AppConfig = stack.load_async().await.unwrap().unwrap();
                assert_eq!(loaded.database.port, 9999);
                // untouched keys still come from the files
                assert_eq!(loaded.database.host, "db.internal");
                assert_eq!(loaded.database.pool_size, 10);

                let _ = std::fs::remove_dir_all(dir);
            }

            /// Async writes go only to the configured target, never to the
            /// other layers, and `save_async` does not flatten the view.
            #[cfg(feature = "async")]
            #[tokio::test]
            async fn async_writes_target_the_selected_source_only() {
                let (stack, dir) = stack($defaults, $project);

                stack
                    .set_async("database.host", "async-edited")
                    .await
                    .unwrap();
                stack.set_val_async("added", 7u16).await.unwrap();

                assert_eq!(
                    stack
                        .source("project")
                        .unwrap()
                        .get("database.host")
                        .unwrap(),
                    "async-edited"
                );
                // the lower layer is untouched
                assert_eq!(
                    stack
                        .source("defaults")
                        .unwrap()
                        .get("database.host")
                        .unwrap(),
                    "localhost"
                );
                assert_eq!(
                    stack.get_async("database.host").await.unwrap(),
                    "async-edited"
                );
                assert_eq!(stack.get_as_async::<u16>("added").await.unwrap(), 7);

                let saved = AppConfig {
                    database: Database {
                        host: "async-saved".to_string(),
                        port: 1,
                        pool_size: 2,
                    },
                    tags: vec!["s".to_string()],
                };
                stack.save_async(&saved).await.unwrap();
                let back: AppConfig = stack.source("project").unwrap().load_or_error().unwrap();
                assert_eq!(back, saved);
                // the defaults file still holds its original values
                assert_eq!(
                    stack
                        .source("defaults")
                        .unwrap()
                        .get("database.host")
                        .unwrap(),
                    "localhost"
                );

                let _ = std::fs::remove_dir_all(dir);
            }

            /// Async writes on a read-only stack report `NoWriteTarget`, and
            /// an empty stack still returns `None`/`NotFound`.
            #[cfg(feature = "async")]
            #[tokio::test]
            async fn async_read_only_and_empty_stack_semantics() {
                let dir = temp_dir("asyncreadonly");
                let (_, dcfg) = layer(&dir, "defaults", $defaults);
                let stack = ConfigStack::new().layer("defaults", dcfg).build().unwrap();

                assert!(matches!(
                    stack.set_async("a", "b").await.unwrap_err(),
                    Error::NoWriteTarget
                ));
                assert!(matches!(
                    stack.set_val_async("a", 1u16).await.unwrap_err(),
                    Error::NoWriteTarget
                ));
                assert!(matches!(
                    stack.save_async(&0u8).await.unwrap_err(),
                    Error::NoWriteTarget
                ));

                let missing = DotCfg::new("dotcfg_stack_test").at_dir(dir.join("missing-async"));
                let empty = ConfigStack::new()
                    .layer("missing", missing)
                    .build()
                    .unwrap();
                assert!(empty.load_async::<AppConfig>().await.unwrap().is_none());
                assert!(matches!(
                    empty.load_or_error_async::<AppConfig>().await,
                    Err(Error::NotFound)
                ));

                let _ = std::fs::remove_dir_all(dir);
            }

            #[cfg(feature = "async")]
            #[tokio::test]
            async fn async_malformed_layer_is_an_error() {
                let (stack, dir) = stack($defaults, $bad);
                assert!(stack.load_async::<AppConfig>().await.is_err());
                assert!(stack.get_async("database.host").await.is_err());
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    };
}

stack_suite!(
    toml_fmt,
    "toml",
    |c: DotCfg| c.toml(),
    "tags = [\"a\", \"b\"]\n\n[database]\nhost = \"localhost\"\nport = 5432\npool_size = 10\n",
    "[database]\nhost = \"db.internal\"\n",
    "tags = [\"c\"]\n\n[database]\nhost = \"db.internal\"\n",
    "database = = broken",
    "port = 1\n"
);

stack_suite!(
    json_fmt,
    "json",
    |c: DotCfg| c.json(),
    "{\"database\": {\"host\": \"localhost\", \"port\": 5432, \"pool_size\": 10}, \"tags\": [\"a\", \"b\"]}",
    "{\"database\": {\"host\": \"db.internal\"}}",
    "{\"database\": {\"host\": \"db.internal\"}, \"tags\": [\"c\"]}",
    "{\"database\": ",
    "{\"port\": 1}"
);

stack_suite!(
    yaml_fmt,
    "yaml",
    |c: DotCfg| c.yaml(),
    "database:\n  host: localhost\n  port: 5432\n  pool_size: 10\ntags:\n  - a\n  - b\n",
    "database:\n  host: db.internal\n",
    "database:\n  host: db.internal\ntags:\n  - c\n",
    "database: [unclosed",
    "port: 1\n"
);

/// No files at all: env-only keys resolve, including nested ones, and flat
/// underscore keys stay flat rather than splitting.
#[test]
fn env_only_without_files() {
    let p = prefix("envonly");
    set_env(&format!("{p}_PORT"), "8080");
    set_env(&format!("{p}_API_KEY"), "k");
    set_env(&format!("{p}_DATABASE__HOST"), "envdb");
    set_env(&format!("{p}_DATABASE__PORT"), "5433");
    set_env(&format!("{p}_DATABASE__POOL_SIZE"), "5");
    set_env(&format!("{p}_TAGS"), "a,b");
    let missing = DotCfg::new("dotcfg_stack_test").at_dir(temp_dir("envonly-missing"));
    let stack = ConfigStack::new()
        .layer("missing", missing)
        .with_env_prefix(&p)
        .build()
        .unwrap();
    let flat: Flat = stack.load_or_error().unwrap();
    assert_eq!(
        flat,
        Flat {
            api_key: "k".to_string(),
            port: 8080
        }
    );
    let app: AppConfig = stack.load_or_error().unwrap();
    assert_eq!(app.database.host, "envdb");
    assert_eq!(app.database.port, 5433);
}

/// A flat underscore key must not split: `API_KEY` is `api_key`, never
/// `api.key`.
#[test]
fn flat_underscore_keys_do_not_nest() {
    #[derive(Deserialize, Debug)]
    struct NestedApi {
        #[allow(dead_code)]
        api: NestedKey,
    }
    #[derive(Deserialize, Debug)]
    struct NestedKey {
        #[allow(dead_code)]
        key: String,
    }
    let p = prefix("flatkey");
    set_env(&format!("{p}_API_KEY"), "k");
    let missing = DotCfg::new("dotcfg_stack_test").at_dir(temp_dir("flatkey-missing"));
    let stack = ConfigStack::new()
        .layer("missing", missing)
        .with_env_prefix(&p)
        .build()
        .unwrap();
    // `api_key` is present but `api.key` is not: the nested load fails on
    // the missing field instead of splitting the flat key.
    assert!(stack.load::<NestedApi>().is_err());
}

/// Nothing configured anywhere: `None`, or `NotFound` for the strict reads.
#[test]
fn all_missing_no_env() {
    let missing = DotCfg::new("dotcfg_stack_test").at_dir(temp_dir("allmissing"));
    let stack = ConfigStack::new()
        .layer("missing", missing)
        .build()
        .unwrap();
    assert!(stack.load::<Flat>().unwrap().is_none());
    assert!(matches!(
        stack.load_or_error::<Flat>().unwrap_err(),
        Error::NotFound
    ));
    assert!(matches!(stack.get("port").unwrap_err(), Error::NotFound));
    assert!(matches!(
        stack.get_as::<u16>("port").unwrap_err(),
        Error::NotFound
    ));
}

#[test]
fn build_rejects_duplicates_and_unknown_targets() {
    let dir = temp_dir("build");
    let a = DotCfg::new("dotcfg_stack_test").at_dir(&dir);
    let b = DotCfg::new("dotcfg_stack_test").at_dir(&dir);
    assert!(matches!(
        ConfigStack::new().layer("a", a).layer("a", b).build(),
        Err(Error::DuplicateSource(_))
    ));
    let c = DotCfg::new("dotcfg_stack_test").at_dir(&dir);
    assert!(matches!(
        ConfigStack::new().layer("a", c).write_to("nope").build(),
        Err(Error::UnknownSource(_))
    ));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn set_val_roundtrip_without_fixtures() {
    let dir = temp_dir("setval");
    let target = DotCfg::new("dotcfg_stack_test").at_dir(&dir);
    let stack = ConfigStack::new()
        .layer("target", target)
        .write_to("target")
        .build()
        .unwrap();
    stack.set_val("port", 3001u16).unwrap();
    assert_eq!(stack.get_as::<u16>("port").unwrap(), 3001);
    let _ = std::fs::remove_dir_all(dir);
}

/// A nested env variable is observed identically by get, get_as and load.
#[test]
fn nested_env_consistent_across_reads() {
    #[derive(Deserialize, Debug)]
    struct Db {
        port: u16,
    }
    #[derive(Deserialize, Debug)]
    struct Cfg {
        database: Db,
    }
    let p = prefix("consistent");
    set_env(&format!("{p}_DATABASE__PORT"), "9999");
    let missing = DotCfg::new("dotcfg_stack_test").at_dir(temp_dir("consistent-missing"));
    let stack = ConfigStack::new()
        .layer("missing", missing)
        .with_env_prefix(&p)
        .build()
        .unwrap();
    assert_eq!(stack.get("database.port").unwrap(), "9999");
    assert_eq!(stack.get_as::<u16>("database.port").unwrap(), 9999);
    assert_eq!(stack.load::<Cfg>().unwrap().unwrap().database.port, 9999);
}

/// Newtypes preserve typed env parsing: numeric and boolean fields set via
/// env must not arrive as strings.
#[test]
fn newtype_typed_env_fields() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Inner {
        port: u16,
        enabled: bool,
    }
    #[derive(Deserialize, Debug, PartialEq)]
    struct Wrapper(Inner);
    let p = prefix("newtype");
    set_env(&format!("{p}_PORT"), "8080");
    set_env(&format!("{p}_ENABLED"), "true");
    let missing = DotCfg::new("dotcfg_stack_test").at_dir(temp_dir("newtype-missing"));
    let stack = ConfigStack::new()
        .layer("missing", missing)
        .with_env_prefix(&p)
        .build()
        .unwrap();
    assert_eq!(
        stack.load::<Wrapper>().unwrap().unwrap(),
        Wrapper(Inner {
            port: 8080,
            enabled: true
        })
    );
}

/// Externally tagged enums resolve from maps with typed env payloads.
#[cfg(feature = "toml")]
#[test]
fn externally_tagged_enum_typed_env_payload() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Endpoint {
        port: u16,
    }
    #[derive(Deserialize, Debug, PartialEq)]
    enum Mode {
        #[serde(rename = "fast")]
        Fast(Endpoint),
        #[serde(rename = "slow")]
        Slow,
    }
    #[derive(Deserialize, Debug, PartialEq)]
    struct Cfg {
        mode: Mode,
    }
    let p = prefix("taggedenum");
    set_env(&format!("{p}_MODE__FAST__PORT"), "9000");
    let dir = temp_dir("taggedenum");
    let cfg = DotCfg::new("dotcfg_stack_test").at_dir(&dir);
    write_file(&cfg.file_path().unwrap(), "[mode.fast]\n");
    let stack = ConfigStack::new()
        .layer("file", cfg)
        .with_env_prefix(&p)
        .build()
        .unwrap();
    assert_eq!(
        stack.load::<Cfg>().unwrap().unwrap(),
        Cfg {
            mode: Mode::Fast(Endpoint { port: 9000 })
        }
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// Strict structs fail loudly on stray env vars rather than ignoring them.
#[test]
fn stray_env_breaks_strict_loads() {
    let p = prefix("strict");
    set_env(&format!("{p}_UNRELATED"), "1");
    let missing = DotCfg::new("dotcfg_stack_test").at_dir(temp_dir("strict-missing"));
    let stack = ConfigStack::new()
        .layer("missing", missing)
        .with_env_prefix(&p)
        .build()
        .unwrap();
    assert!(stack.load::<StrictPort>().is_err());
}

/// Sources may mix formats: merging happens on the unified tree.
#[cfg(all(feature = "toml", feature = "json", feature = "yaml"))]
#[test]
fn mixed_formats_merge() {
    let dir = temp_dir("mixed");
    let tcfg = DotCfg::new("dotcfg_stack_test")
        .at_dir(dir.join("toml"))
        .toml();
    write_file(
        &tcfg.file_path().unwrap(),
        "tags = [\"a\", \"b\"]\n\n[database]\nhost = \"localhost\"\nport = 5432\npool_size = 10\n",
    );
    let jcfg = DotCfg::new("dotcfg_stack_test")
        .at_dir(dir.join("json"))
        .json();
    write_file(
        &jcfg.file_path().unwrap(),
        "{\"database\": {\"host\": \"db.internal\"}}",
    );
    let stack = ConfigStack::new()
        .layer("toml", tcfg)
        .layer("json", jcfg)
        .build()
        .unwrap();
    let cfg: AppConfig = stack.load_or_error().unwrap();
    assert_eq!(cfg.database.host, "db.internal");
    assert_eq!(cfg.database.port, 5432);
    let _ = std::fs::remove_dir_all(dir);
}
