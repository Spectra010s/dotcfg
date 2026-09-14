use dotcfg::DotCfg;
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

#[cfg(feature = "toml")]
const DEFAULT_EXT: &str = "toml";
#[cfg(all(feature = "json", not(feature = "toml")))]
const DEFAULT_EXT: &str = "json";
#[cfg(all(feature = "yaml", not(feature = "toml"), not(feature = "json")))]
const DEFAULT_EXT: &str = "yaml";

use std::path::PathBuf;

fn temp_custom_dir(suffix: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "dotcfg_test_custom_{}_{}_{}",
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

/// Custom dir is exactly what the user passes, so the test uses a fresh temp
/// dir to avoid touching real home. We verify `dir()` and `file_path()` point
/// to that custom dir (app_name is ignored) and that save/load and per-key ops
/// stay inside it. The user could also pass a home path if they wanted.
#[test]
fn custom_dir_isolated_temp_dir() {
    let dir = temp_custom_dir("isolated");
    // app_name is ignored when Custom is set, dir is exactly what we passed
    let cfg = DotCfg::new("ignored_app_name").at_dir(&dir);
    assert_eq!(cfg.dir().unwrap(), dir);
    assert_eq!(cfg.file_path().unwrap(), dir.join(format!("config.{}", DEFAULT_EXT)));

    let original = TestConfig {
        username: "tayo".into(),
        port: 8080,
        nested: None,
    };
    // save creates the custom dir + file
    cfg.save(&original).unwrap();
    assert!(cfg.exists().unwrap());
    // load must deserialize the same struct back
    let loaded: Option<TestConfig> = cfg.load().unwrap();
    assert_eq!(loaded, Some(original));

    // per-key ops must stay inside the same custom dir, not HOME
    cfg.set("username", "jane").unwrap();
    assert_eq!(cfg.get("username").unwrap(), "jane");
    cfg.set_val("port", 9090u16).unwrap();
    assert_eq!(cfg.get_as::<u16>("port").unwrap(), 9090);

    // cleanup — custom dir is removed, not home
    cfg.delete_dir().unwrap();
    assert!(!dir.exists());
}

/// Custom is flexible — any dir name works, not just `.toolname`. We test a dot
/// project dir (`.sendra` like Sendra), a different dot name (`.nottoolname`),
/// and a plain non-dot dir (`my-config`) to prove `at_dir` does not enforce a
/// prefix and just uses the final dir you give it.
#[test]
fn custom_dir_flexible_any_name() {
    for name in [".sendra", ".nottoolname", "my-config"] {
        let base = temp_custom_dir(&format!("flex_{}", name.replace('.', "_")));
        let dir = base.join(name);
        let cfg = DotCfg::new("mytool").at_dir(&dir);
        // each name must work — no enforced `.toolname` prefix
        cfg.set("username", "tayo").unwrap();
        assert_eq!(cfg.get("username").unwrap(), "tayo");
        // dir() must be exactly the custom dir we gave
        assert_eq!(cfg.dir().unwrap(), dir);
        cfg.delete_dir().unwrap();
    }
}

/// `at_dir` with a `.sendra` dot dir resolves exactly where you run it, with no
/// ancestor walk. This shows `Custom` is exact match only: from the project
/// root it finds `.sendra`, from a subdir it would not (that needs #24).
/// We simulate the project root by creating a temp dir with `.sendra` inside.

#[test]
fn custom_dir_relative_dot_dir() {
    let dir = temp_custom_dir("relative");
    std::fs::create_dir_all(&dir).unwrap();
    let dot_dir = dir.join(".sendra");
    // In real use this would be `at_dir(".sendra")` from `dir`; we pass absolute
    // to avoid mutating process cwd in parallel tests — same contract.
    let cfg = DotCfg::new("sendra").at_dir(&dot_dir);
    cfg.set("username", "alice").unwrap();
    assert_eq!(cfg.get("username").unwrap(), "alice");
    assert!(dot_dir.exists());
    cfg.delete_dir().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
