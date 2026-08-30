//! Integration tests for dotcfg — derived from manual `../cfgdot` testing.
//! Each test uses a unique `dotcfg_test_*` app name and cleans up via `delete_dir()`.

use dotcfg::{DotCfg, error::DotCfgError};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
struct TestConfig {
    username: String,
    port: u16,
    #[serde(default)]
    nested: Option<Nested>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
struct Nested {
    val: String,
}

/// Helper: unique config handle per test to avoid dir collisions.
/// Uses process id + suffix, cleans any leftover dir from prior runs.
fn unique_cfg(suffix: &str) -> DotCfg {
    let name = format!("dotcfg_test_{}_{}", suffix, std::process::id());
    let cfg = DotCfg::new(&name);
    let _ = cfg.delete_dir();
    cfg
}

/// Full save/load roundtrip — file should exist and deserialize to original.
#[test]
fn save_and_load_roundtrip() {
    let cfg = unique_cfg("save_load");
    let original = TestConfig {
        username: "tayo".into(),
        port: 8080,
        nested: None,
    };
    cfg.save(&original).expect("save");
    assert!(cfg.exists().unwrap());
    let loaded: Option<TestConfig> = cfg.load().unwrap();
    assert_eq!(loaded, Some(original));
    cfg.delete_dir().unwrap();
}

/// `load()` returns None when file doesn't exist — no auto-create.
#[test]
fn load_none_when_missing() {
    let cfg = unique_cfg("load_none");
    let loaded: Option<TestConfig> = cfg.load().unwrap();
    assert!(loaded.is_none());
}

/// `load_or_default()` creates file with Default and returns it.
#[test]
fn load_or_default_creates_file() {
    let cfg = unique_cfg("load_default");
    let loaded: TestConfig = cfg.load_or_default().unwrap();
    assert_eq!(loaded, TestConfig::default());
    assert!(cfg.exists().unwrap());
    cfg.delete_dir().unwrap();
}

/// `load_or_error()` should error with NotFound when missing.
#[test]
fn load_or_error_fails_when_missing() {
    let cfg = unique_cfg("load_error");
    let res: Result<TestConfig, _> = cfg.load_or_error();
    assert!(matches!(res.unwrap_err(), DotCfgError::NotFound));
}

/// Flat key get/set — set should update value and preserve other keys as strings.
#[test]
fn get_set_flat_key() {
    let cfg = unique_cfg("flat");
    cfg.save(&TestConfig {
        username: "alice".into(),
        port: 3000,
        nested: None,
    })
    .unwrap();
    assert_eq!(cfg.get("username").unwrap(), "alice");
    cfg.set("username", "bob").unwrap();
    assert_eq!(cfg.get("username").unwrap(), "bob");
    // numeric fields are stringified via get()
    assert_eq!(cfg.get("port").unwrap(), "3000");
    cfg.delete_dir().unwrap();
}

/// Nested `section.field` get/set — used for `[user]` tables / nested JSON.
#[test]
fn get_set_nested_key() {
    let cfg = unique_cfg("nested");
    cfg.set("user.username", "tayo").unwrap();
    assert_eq!(cfg.get("user.username").unwrap(), "tayo");
    cfg.set("user.username", "jane").unwrap();
    assert_eq!(cfg.get("user.username").unwrap(), "jane");
    cfg.delete_dir().unwrap();
}

/// `set()` must preserve other keys — only target key is mutated.
#[test]
fn set_preserves_other_keys() {
    let cfg = unique_cfg("preserve");
    cfg.save(&TestConfig {
        username: "keep".into(),
        port: 9090,
        nested: None,
    })
    .unwrap();
    cfg.set("username", "changed").unwrap();
    let loaded: TestConfig = cfg.load().unwrap().unwrap();
    assert_eq!(loaded.username, "changed");
    assert_eq!(loaded.port, 9090);
    cfg.delete_dir().unwrap();
}

/// Delete helpers — file vs entire dir.
#[test]
fn delete_file_and_dir() {
    let cfg = unique_cfg("delete");
    cfg.save(&TestConfig::default()).unwrap();
    assert!(cfg.exists().unwrap());
    cfg.delete_file().unwrap();
    assert!(!cfg.exists().unwrap());
    assert!(cfg.dir().unwrap().exists());
    cfg.delete_dir().unwrap();
    assert!(!cfg.dir().unwrap().exists());
}

/// The extension `DotCfg::new()` picks by default, which follows whichever
/// format feature is compiled in (toml > json > yaml).
#[cfg(feature = "toml")]
const DEFAULT_EXT: &str = "toml";
#[cfg(all(feature = "json", not(feature = "toml")))]
const DEFAULT_EXT: &str = "json";
#[cfg(all(feature = "yaml", not(feature = "toml"), not(feature = "json")))]
const DEFAULT_EXT: &str = "yaml";

/// `dir()` and `file_path()` should point to expected locations and extensions.
#[test]
fn dir_and_file_path() {
    let cfg = unique_cfg("paths");
    let dir = cfg.dir().unwrap();
    assert!(dir.to_string_lossy().contains("dotcfg_test_paths"));
    let file = cfg.file_path().unwrap();
    assert_eq!(file.extension().unwrap(), DEFAULT_EXT);
    cfg.delete_dir().unwrap();
}

/// Custom filename via `.filename()` changes the file name and preserves extension.
#[test]
fn filename_custom() {
    let cfg = unique_cfg("filename").filename("settings");
    let file = cfg.file_path().unwrap();
    assert!(
        file.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("settings")
    );
    cfg.save(&TestConfig::default()).unwrap();
    assert!(cfg.exists().unwrap());
    cfg.delete_dir().unwrap();
}

/// YAML save/load roundtrip — file should be `config.yaml` and deserialize back.
#[cfg(feature = "yaml")]
#[test]
fn yaml_save_and_load_roundtrip() {
    let cfg = unique_cfg("yaml_roundtrip").yaml();
    let original = TestConfig {
        username: "tayo".into(),
        port: 8080,
        nested: Some(Nested { val: "deep".into() }),
    };
    cfg.save(&original).expect("save");
    assert!(cfg.exists().unwrap());
    assert_eq!(cfg.file_path().unwrap().extension().unwrap(), "yaml");
    let loaded: Option<TestConfig> = cfg.load().unwrap();
    assert_eq!(loaded, Some(original));
    cfg.delete_dir().unwrap();
}

/// YAML flat + nested get/set, and `set()` preserving untouched keys.
#[cfg(feature = "yaml")]
#[test]
fn yaml_get_set_keys() {
    let cfg = unique_cfg("yaml_keys").yaml();
    cfg.save(&TestConfig {
        username: "alice".into(),
        port: 3000,
        nested: None,
    })
    .unwrap();

    assert_eq!(cfg.get("username").unwrap(), "alice");
    // numeric fields are stringified via get()
    assert_eq!(cfg.get("port").unwrap(), "3000");

    cfg.set("username", "bob").unwrap();
    assert_eq!(cfg.get("username").unwrap(), "bob");
    // other keys survive a per-key set
    assert_eq!(cfg.get("port").unwrap(), "3000");

    // nested `section.field` creates the mapping on demand
    cfg.set("user.email", "bob@example.com").unwrap();
    assert_eq!(cfg.get("user.email").unwrap(), "bob@example.com");
    cfg.set("user.email", "bob@other.test").unwrap();
    assert_eq!(cfg.get("user.email").unwrap(), "bob@other.test");

    let loaded: TestConfig = cfg.load().unwrap().unwrap();
    assert_eq!(loaded.username, "bob");
    assert_eq!(loaded.port, 3000);

    assert!(matches!(
        cfg.get("nope").unwrap_err(),
        DotCfgError::KeyNotFound(_)
    ));

    cfg.delete_dir().unwrap();
}

/// YAML `set()` on a missing file creates dir + file from scratch.
#[cfg(feature = "yaml")]
#[test]
fn yaml_set_creates_file() {
    let cfg = unique_cfg("yaml_create").yaml();
    assert!(!cfg.exists().unwrap());
    cfg.set("user.username", "tayo").unwrap();
    assert!(cfg.exists().unwrap());
    assert_eq!(cfg.get("user.username").unwrap(), "tayo");
    cfg.delete_dir().unwrap();
}

/// XDG strategy — `~/.config/<app>/` on Linux, platform-aware via etcetera.
#[test]
fn xdg_strategy() {
    let cfg = DotCfg::new(format!("dotcfg_test_xdg_{}", std::process::id())).xdg();
    let _ = cfg.delete_dir();
    let dir = cfg.dir().unwrap();
    assert!(dir.to_string_lossy().contains("config") || dir.to_string_lossy().contains(".config"));
    cfg.delete_dir().unwrap();
}

// ---------------------------------------------------------------------------
// Typed accessors: `get_as` / `set_val`
// ---------------------------------------------------------------------------

/// Every type we promise, exercised against whichever format `cfg` uses.
/// `set_val` writes native values (numbers, bools, arrays) and `get_as`
/// deserializes them back without a string round trip.
fn assert_typed_accessors(cfg: &DotCfg) {
    cfg.set_val("port", 8080u16).unwrap();
    cfg.set_val("retries", -3i32).unwrap();
    cfg.set_val("ratio", 0.75f64).unwrap();
    cfg.set_val("debug", true).unwrap();
    cfg.set_val("username", "tayo").unwrap();
    cfg.set_val("plugins", vec!["fmt".to_string(), "lint".to_string()])
        .unwrap();
    cfg.set_val("weights", vec![1i32, 2, 3]).unwrap();
    // nested dotted path — the intermediate table/map is created on demand
    cfg.set_val("features.auto_update", true).unwrap();

    assert_eq!(cfg.get_as::<u16>("port").unwrap(), 8080);
    assert_eq!(cfg.get_as::<i32>("retries").unwrap(), -3);
    assert_eq!(cfg.get_as::<f64>("ratio").unwrap(), 0.75);
    assert!(cfg.get_as::<bool>("debug").unwrap());
    assert_eq!(cfg.get_as::<String>("username").unwrap(), "tayo");
    assert_eq!(
        cfg.get_as::<Vec<String>>("plugins").unwrap(),
        vec!["fmt".to_string(), "lint".to_string()]
    );
    assert_eq!(cfg.get_as::<Vec<i32>>("weights").unwrap(), vec![1, 2, 3]);
    assert!(cfg.get_as::<bool>("features.auto_update").unwrap());

    // a whole section deserializes into a struct
    cfg.set_val("nested", Nested { val: "deep".into() })
        .unwrap();
    assert_eq!(
        cfg.get_as::<Nested>("nested").unwrap(),
        Nested { val: "deep".into() }
    );

    // `get()` still stringifies the same nodes — existing behavior unchanged
    assert_eq!(cfg.get("port").unwrap(), "8080");
    assert_eq!(cfg.get("username").unwrap(), "tayo");
    assert_eq!(cfg.get("features.auto_update").unwrap(), "true");

    // missing key / missing section both report KeyNotFound, not a panic
    assert!(matches!(
        cfg.get_as::<u16>("nope").unwrap_err(),
        DotCfgError::KeyNotFound(_)
    ));
    assert!(matches!(
        cfg.get_as::<u16>("nope.nope").unwrap_err(),
        DotCfgError::KeyNotFound(_)
    ));
}

/// TOML typed accessors across every supported value type.
#[cfg(feature = "toml")]
#[test]
fn toml_typed_accessors() {
    let cfg = unique_cfg("toml_typed").toml();
    assert_typed_accessors(&cfg);
    cfg.delete_dir().unwrap();
}

/// JSON typed accessors across every supported value type.
#[cfg(feature = "json")]
#[test]
fn json_typed_accessors() {
    let cfg = unique_cfg("json_typed").json();
    assert_typed_accessors(&cfg);
    cfg.delete_dir().unwrap();
}

/// YAML typed accessors across every supported value type.
#[cfg(feature = "yaml")]
#[test]
fn yaml_typed_accessors() {
    let cfg = unique_cfg("yaml_typed").yaml();
    assert_typed_accessors(&cfg);
    cfg.delete_dir().unwrap();
}

/// Round trip: `set_val` a value, `get_as` it back, and confirm the file still
/// loads as a whole struct with the typed values intact.
#[test]
fn set_val_get_as_roundtrip() {
    let cfg = unique_cfg("typed_roundtrip");

    cfg.set_val("username", "tayo").unwrap();
    cfg.set_val("port", 8080u16).unwrap();

    assert_eq!(cfg.get_as::<String>("username").unwrap(), "tayo");
    assert_eq!(cfg.get_as::<u16>("port").unwrap(), 8080);

    // the port landed as a number, so the full struct load still works
    let loaded: TestConfig = cfg.load().unwrap().unwrap();
    assert_eq!(
        loaded,
        TestConfig {
            username: "tayo".into(),
            port: 8080,
            nested: None,
        }
    );

    cfg.delete_dir().unwrap();
}

/// `set_val` only touches its own key — everything else survives.
#[test]
fn set_val_preserves_other_keys() {
    let cfg = unique_cfg("typed_preserve");
    cfg.save(&TestConfig {
        username: "keep".into(),
        port: 9090,
        nested: None,
    })
    .unwrap();

    cfg.set_val("port", 1234u16).unwrap();

    let loaded: TestConfig = cfg.load().unwrap().unwrap();
    assert_eq!(loaded.username, "keep");
    assert_eq!(loaded.port, 1234);
    cfg.delete_dir().unwrap();
}

/// `set_val` on a missing file creates dir + file from scratch.
#[test]
fn set_val_creates_file() {
    let cfg = unique_cfg("typed_create");
    assert!(!cfg.exists().unwrap());
    cfg.set_val("features.auto_update", true).unwrap();
    assert!(cfg.exists().unwrap());
    assert!(cfg.get_as::<bool>("features.auto_update").unwrap());
    cfg.delete_dir().unwrap();
}

/// A type mismatch is an error, never a panic.
#[test]
fn get_as_type_mismatch_errors() {
    let cfg = unique_cfg("typed_mismatch");
    cfg.set("username", "alice").unwrap();

    let res = cfg.get_as::<u16>("username");
    assert!(res.is_err(), "expected Err for non-numeric value");
    // and it is not one of the lookup errors — it comes from serde
    assert!(!matches!(
        res.unwrap_err(),
        DotCfgError::KeyNotFound(_) | DotCfgError::NotFound
    ));

    cfg.delete_dir().unwrap();
}

/// `get_as` on a missing config file reports NotFound, like `get`.
#[test]
fn get_as_missing_file_errors() {
    let cfg = unique_cfg("typed_missing");
    let res = cfg.get_as::<u16>("port");
    assert!(matches!(res.unwrap_err(), DotCfgError::NotFound));
}
