//! Integration tests for dotcfg async I/O.

#![cfg(feature = "async")]

use dotcfg::{DotCfg, error::DotCfgError};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Default)]
struct TestConfig {
    username: String,
    port: u16,
}

fn unique_cfg(suffix: &str) -> DotCfg {
    let name = format!("dotcfg_test_async_{}_{}", suffix, std::process::id());
    let cfg = DotCfg::new(&name);
    let _ = cfg.delete_dir();
    cfg
}

#[tokio::test]
async fn async_save_and_load_roundtrip() {
    let cfg = unique_cfg("save_load");
    let original = TestConfig {
        username: "tayo".into(),
        port: 8080,
    };
    cfg.save_async(&original).await.expect("save_async");
    assert!(cfg.exists_async().await.unwrap());
    let loaded: Option<TestConfig> = cfg.load_async().await.unwrap();
    assert_eq!(loaded, Some(original));
    cfg.delete_dir_async().await.unwrap();
}

#[tokio::test]
async fn async_load_none_when_missing() {
    let cfg = unique_cfg("load_none");
    let loaded: Option<TestConfig> = cfg.load_async().await.unwrap();
    assert!(loaded.is_none());
}

#[tokio::test]
async fn async_load_or_default_creates_file() {
    let cfg = unique_cfg("load_default");
    let loaded: TestConfig = cfg.load_or_default_async().await.unwrap();
    assert_eq!(loaded, TestConfig::default());
    assert!(cfg.exists_async().await.unwrap());
    cfg.delete_dir_async().await.unwrap();
}

#[tokio::test]
async fn async_load_or_error_fails_when_missing() {
    let cfg = unique_cfg("load_error");
    let res: Result<TestConfig, _> = cfg.load_or_error_async().await;
    assert!(matches!(res.unwrap_err(), DotCfgError::NotFound));
}

#[tokio::test]
async fn async_get_set_flat_and_nested() {
    let cfg = unique_cfg("get_set");
    cfg.set_async("user.username", "alice").await.unwrap();
    assert_eq!(cfg.get_async("user.username").await.unwrap(), "alice");
    cfg.set_async("user.username", "bob").await.unwrap();
    assert_eq!(cfg.get_async("user.username").await.unwrap(), "bob");
    cfg.delete_dir_async().await.unwrap();
}

#[tokio::test]
async fn async_typed_accessors() {
    let cfg = unique_cfg("typed");
    cfg.set_val_async("port", 9000u16).await.unwrap();
    let port: u16 = cfg.get_as_async("port").await.unwrap();
    assert_eq!(port, 9000);
    cfg.delete_dir_async().await.unwrap();
}

#[tokio::test]
async fn async_delete_file_and_dir() {
    let cfg = unique_cfg("delete");
    cfg.save_async(&TestConfig {
        username: "test".into(),
        port: 1234,
    })
    .await
    .unwrap();
    assert!(cfg.exists_async().await.unwrap());
    cfg.delete_file_async().await.unwrap();
    assert!(!cfg.exists_async().await.unwrap());
    cfg.delete_dir_async().await.unwrap();
}

#[tokio::test]
async fn async_find_in_ancestors() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("a");
    let child = parent.join("b");
    tokio::fs::create_dir_all(&child).await.unwrap();

    let project_dir = parent.join(".dotcfg_test_async_proj");
    tokio::fs::create_dir_all(&project_dir).await.unwrap();
    let config_file = project_dir.join("config.toml");
    tokio::fs::write(&config_file, "username = \"proj\"\n")
        .await
        .unwrap();

    let found = DotCfg::new("dotcfg_test_async_proj")
        .find_in_ancestors_from_async(&child)
        .await
        .unwrap();

    assert!(found.is_some());
    let cfg = found.unwrap();
    assert_eq!(cfg.get_async("username").await.unwrap(), "proj");
}
