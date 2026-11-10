//! Integration tests for the optional Tokio async API.

#![cfg(feature = "async")]

use dotcfg::{DotCfg, Error};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
struct TestConfig {
    username: String,
    port: u16,
}

fn isolated_cfg() -> (tempfile::TempDir, DotCfg) {
    let dir = tempfile::tempdir().expect("temporary directory");
    let cfg = DotCfg::new("dotcfg_async_test").at_dir(dir.path());
    (dir, cfg)
}

#[tokio::test]
async fn async_save_load_and_typed_accessors_roundtrip() {
    let (_dir, cfg) = isolated_cfg();
    let original = TestConfig {
        username: "tayo".into(),
        port: 8080,
    };

    cfg.save_async(&original).await.expect("save");
    assert!(cfg.exists_async().await.unwrap());
    assert_eq!(
        cfg.load_async::<TestConfig>().await.unwrap(),
        Some(original)
    );

    cfg.set_async("user.name", "alice").await.unwrap();
    assert_eq!(cfg.get_async("user.name").await.unwrap(), "alice");

    cfg.set_val_async("server.port", 9000_u16).await.unwrap();
    assert_eq!(cfg.get_as_async::<u16>("server.port").await.unwrap(), 9000);

    let loaded: TestConfig = cfg.load_or_error_async().await.unwrap();
    assert_eq!(loaded.port, 8080);
}

#[tokio::test]
async fn async_missing_load_and_default_semantics() {
    let (_dir, cfg) = isolated_cfg();

    assert!(cfg.load_async::<TestConfig>().await.unwrap().is_none());
    assert!(matches!(
        cfg.load_or_error_async::<TestConfig>().await,
        Err(Error::NotFound)
    ));

    let config = cfg.load_or_default_async::<TestConfig>().await.unwrap();
    assert_eq!(config, TestConfig::default());
    assert!(cfg.exists_async().await.unwrap());
}

#[tokio::test]
async fn async_set_preserves_existing_keys_and_errors_on_missing_key() {
    let (_dir, cfg) = isolated_cfg();

    cfg.set_val_async("server.port", 8080_u16).await.unwrap();
    cfg.set_async("server.host", "localhost").await.unwrap();

    assert_eq!(cfg.get_as_async::<u16>("server.port").await.unwrap(), 8080);
    assert_eq!(cfg.get_async("server.host").await.unwrap(), "localhost");
    assert!(matches!(
        cfg.get_async("missing").await,
        Err(Error::KeyNotFound(_))
    ));
}

#[tokio::test]
async fn async_file_and_directory_deletion_are_idempotent() {
    let (_dir, cfg) = isolated_cfg();

    cfg.save_async(&TestConfig::default()).await.unwrap();
    cfg.delete_file_async().await.unwrap();
    assert!(!cfg.exists_async().await.unwrap());

    cfg.delete_file_async().await.unwrap();
    cfg.delete_dir_async().await.unwrap();
    cfg.delete_dir_async().await.unwrap();
}

#[cfg(feature = "toml")]
#[tokio::test]
async fn async_ancestor_lookup_finds_project_config() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("parent");
    let child = parent.join("child");
    tokio::fs::create_dir_all(&child).await.unwrap();

    let config_dir = parent.join(".dotcfg_async_project");
    tokio::fs::create_dir_all(&config_dir).await.unwrap();
    tokio::fs::write(config_dir.join("config.toml"), "username = \"project\"\n")
        .await
        .unwrap();

    let found = DotCfg::new("dotcfg_async_project")
        .find_in_ancestors_from_async(&child)
        .await
        .unwrap()
        .expect("project config should be found");

    assert_eq!(found.get_async("username").await.unwrap(), "project");
}

#[cfg(feature = "toml")]
#[tokio::test]
async fn async_ancestor_lookup_returns_non_not_found_metadata_errors() {
    let root = tempfile::tempdir().unwrap();
    let child = root.path().join("child");
    tokio::fs::create_dir_all(&child).await.unwrap();

    // A file where the config directory should be makes metadata fail with
    // NotADirectory. The search must report that error instead of skipping it.
    tokio::fs::write(child.join(".dotcfg_async_project"), "not a directory")
        .await
        .unwrap();

    let result = DotCfg::new("dotcfg_async_project")
        .find_in_ancestors_from_async(&child)
        .await;
    assert!(matches!(
        result,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotADirectory
    ));
}

#[cfg(feature = "json")]
#[tokio::test]
async fn async_json_uses_shared_value_handling() {
    let (_dir, cfg) = isolated_cfg();
    let cfg = cfg.json();
    let original = TestConfig {
        username: "json-user".into(),
        port: 8123,
    };

    cfg.save_async(&original).await.unwrap();
    assert_eq!(
        cfg.load_async::<TestConfig>().await.unwrap(),
        Some(original)
    );
    cfg.set_val_async("server.port", 9000_u16).await.unwrap();
    assert_eq!(cfg.get_as_async::<u16>("server.port").await.unwrap(), 9000);
}

#[cfg(feature = "yaml")]
#[tokio::test]
async fn async_yaml_uses_shared_value_handling() {
    let (_dir, cfg) = isolated_cfg();
    let cfg = cfg.yaml();
    let original = TestConfig {
        username: "yaml-user".into(),
        port: 8124,
    };

    cfg.save_async(&original).await.unwrap();
    assert_eq!(
        cfg.load_async::<TestConfig>().await.unwrap(),
        Some(original)
    );
    cfg.set_val_async("server.port", 9001_u16).await.unwrap();
    assert_eq!(cfg.get_as_async::<u16>("server.port").await.unwrap(), 9001);
}
