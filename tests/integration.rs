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

/// `dir()` and `file_path()` should point to expected locations and extensions.
#[test]
fn dir_and_file_path() {
    let cfg = unique_cfg("paths");
    let dir = cfg.dir().unwrap();
    assert!(dir.to_string_lossy().contains("dotcfg_test_paths"));
    let file = cfg.file_path().unwrap();
    assert_eq!(file.extension().unwrap(), "toml");
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

/// XDG strategy — `~/.config/<app>/` on Linux, platform-aware via etcetera.
#[test]
fn xdg_strategy() {
    let cfg = DotCfg::new(format!("dotcfg_test_xdg_{}", std::process::id())).xdg();
    let _ = cfg.delete_dir();
    let dir = cfg.dir().unwrap();
    assert!(dir.to_string_lossy().contains("config") || dir.to_string_lossy().contains(".config"));
    cfg.delete_dir().unwrap();
}
