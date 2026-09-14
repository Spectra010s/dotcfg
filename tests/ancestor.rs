use dotcfg::DotCfg;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
struct TestConfig {
    username: String,
    port: u16,
}

fn temp_root(suffix: &str) -> PathBuf {
    // Unique temp root per test, avoids collisions when tests run in parallel.
    let path = std::env::temp_dir().join(format!(
        "dotcfg_test_ancestor_{}_{}_{}",
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

/// Walk should find config in the start directory's own `.app` dir.
///
/// Why: most common case is running from project root. We create a temp
/// project with `.mytool/config.toml` at its root and start the search from
/// that root. It must return Some with Custom pointing to that `.mytool` dir.
#[test]
fn ancestor_finds_in_start_dir() {
    let root = temp_root("start");
    let dot_dir = root.join(".mytool");
    // Create a real config file so find_in_ancestors_from can discover it.
    let cfg_at_root = DotCfg::new("mytool").at_dir(&dot_dir);
    cfg_at_root
        .save(&TestConfig {
            username: "tayo".into(),
            port: 8080,
        })
        .unwrap();

    // Search from root itself, should find root/.mytool
    let finder = DotCfg::new("mytool");
    let found = finder.find_in_ancestors_from(&root).unwrap().unwrap();
    // found dir must be exactly root/.mytool, not HOME
    assert_eq!(found.dir().unwrap(), dot_dir);
    // and it must be able to load the same config
    let loaded: TestConfig = found.load().unwrap().unwrap();
    assert_eq!(loaded.username, "tayo");

    let _ = std::fs::remove_dir_all(&root);
}

/// Walk should find config in a parent when start is a nested subdir.
///
/// Why: running from `t/s/a/b` should still find `t/s/.mytool` like git
/// finds `.git`. We create `t/s/.mytool` and start from `t/s/a/b`.
#[test]
fn ancestor_finds_in_parent() {
    let root = temp_root("parent");
    let dot_dir = root.join(".mytool");
    DotCfg::new("mytool")
        .at_dir(&dot_dir)
        .save(&TestConfig {
            username: "parent".into(),
            port: 3000,
        })
        .unwrap();

    // Nested subdir with no config of its own
    let nested = root.join("a").join("b");
    std::fs::create_dir_all(&nested).unwrap();

    let finder = DotCfg::new("mytool");
    let found = finder.find_in_ancestors_from(&nested).unwrap().unwrap();
    assert_eq!(found.dir().unwrap(), dot_dir);

    let _ = std::fs::remove_dir_all(&root);
}

/// Nearest `.app` wins when both parent and grandparent have one.
///
/// Why: if `t/.mytool` and `t/s/.mytool` both exist, a run from `t/s/a`
/// must resolve `t/s/.mytool`, not `t/.mytool`. This is the git-like
/// nearest-wins rule that Sendra's find_project_config uses.
#[test]
fn ancestor_nearest_wins() {
    let root = temp_root("nearest");
    // t/.mytool
    let outer_dir = root.join(".mytool");
    DotCfg::new("mytool")
        .at_dir(&outer_dir)
        .save(&TestConfig {
            username: "outer".into(),
            port: 1,
        })
        .unwrap();
    // t/s/.mytool
    let inner_root = root.join("s");
    let inner_dir = inner_root.join(".mytool");
    DotCfg::new("mytool")
        .at_dir(&inner_dir)
        .save(&TestConfig {
            username: "inner".into(),
            port: 2,
        })
        .unwrap();

    // Start deep inside s: s/a/b/c/d/e/f/g/h/i/j
    let deep = inner_root.join("a/b/c/d/e/f/g/h/i/j");
    std::fs::create_dir_all(&deep).unwrap();

    let finder = DotCfg::new("mytool");
    let found = finder.find_in_ancestors_from(&deep).unwrap().unwrap();
    // must be inner, not outer
    assert_eq!(found.dir().unwrap(), inner_dir);
    let loaded: TestConfig = found.load().unwrap().unwrap();
    assert_eq!(loaded.username, "inner");

    // Also check that from inner_root itself we still get inner
    let finder2 = DotCfg::new("mytool");
    let found2 = finder2.find_in_ancestors_from(&inner_root).unwrap().unwrap();
    assert_eq!(found2.dir().unwrap(), inner_dir);

    let _ = std::fs::remove_dir_all(&root);
}

/// Returns None when no ancestor has `.mytool`, with no fallback to home.
///
/// Why: dotcfg is no-magic, missing project config is not an error and must
/// not fall back to `~/.mytool`. The caller decides to use global or defaults.
#[test]
fn ancestor_returns_none_when_missing() {
    let root = temp_root("missing");
    std::fs::create_dir_all(&root).unwrap();
    let nested = root.join("a/b");
    std::fs::create_dir_all(&nested).unwrap();

    let finder = DotCfg::new("mytool");
    let found = finder.find_in_ancestors_from(&nested).unwrap();
    assert!(found.is_none(), "should be None when no .mytool in ancestors");

    let _ = std::fs::remove_dir_all(&root);
}

/// Respects filename and format when searching.
///
/// Why: `DotCfg::new("mytool").yaml().filename("settings")` looks for
/// `.mytool/settings.yaml`, not `.mytool/config.toml`. An ancestor with the
/// wrong filename or ext must not be matched.
#[cfg(feature = "yaml")]
#[test]
fn ancestor_respects_filename_and_format() {
    let root = temp_root("filename");
    // Create .mytool/config.toml (default filename, toml)
    let dot_dir = root.join(".mytool");
    DotCfg::new("mytool")
        .at_dir(&dot_dir)
        .save(&TestConfig {
            username: "toml".into(),
            port: 10,
        })
        .unwrap();

    // Search with yaml and settings should not find the toml config
    let finder = DotCfg::new("mytool").yaml().filename("settings");
    let found = finder.find_in_ancestors_from(&root).unwrap();
    assert!(found.is_none(), "wrong filename/ext should not match");

    // Now create the matching .mytool/settings.yaml in the same dir
    let cfg_settings = DotCfg::new("mytool")
        .yaml()
        .filename("settings")
        .at_dir(&dot_dir);
    cfg_settings
        .save(&TestConfig {
            username: "yaml".into(),
            port: 20,
        })
        .unwrap();
    // Now the same finder should find it
    let finder2 = DotCfg::new("mytool").yaml().filename("settings");
    let found2 = finder2.find_in_ancestors_from(&root).unwrap().unwrap();
    assert_eq!(found2.dir().unwrap(), dot_dir);

    let _ = std::fs::remove_dir_all(&root);
}
