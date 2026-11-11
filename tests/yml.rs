#![cfg(feature = "yaml")]

use dotcfg::DotCfg;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Settings {
    name: String,
}

#[test]
fn yaml_and_yml_selection_and_writes() {
    let temp = tempfile::tempdir().unwrap();
    let cfg = DotCfg::new("mytool").yaml().at_dir(temp.path());
    let yaml = temp.path().join("config.yaml");
    let yml = temp.path().join("config.yml");

    // Neither exists: writes create the canonical .yaml path.
    assert_eq!(cfg.file_path().unwrap(), yaml);
    cfg.save(&Settings { name: "new".into() }).unwrap();
    assert!(yaml.exists());
    assert!(!yml.exists());

    // Only .yaml exists.
    assert_eq!(cfg.load::<Settings>().unwrap().unwrap().name, "new");

    // Both exist: .yaml wins for reads and writes.
    std::fs::write(&yml, "name: alternate\n").unwrap();
    assert_eq!(cfg.file_path().unwrap(), yaml);
    assert_eq!(cfg.load::<Settings>().unwrap().unwrap().name, "new");
    cfg.save(&Settings {
        name: "updated".into(),
    })
    .unwrap();
    assert_eq!(std::fs::read_to_string(&yml).unwrap(), "name: alternate\n");

    // Only .yml exists: reads and writes use that existing file.
    std::fs::remove_file(&yaml).unwrap();
    assert_eq!(cfg.file_path().unwrap(), yml);
    assert_eq!(cfg.load::<Settings>().unwrap().unwrap().name, "alternate");
    cfg.set("name", "from_set").unwrap();
    assert!(!yaml.exists());
    assert_eq!(cfg.load::<Settings>().unwrap().unwrap().name, "from_set");
}

#[test]
fn ancestor_search_finds_yml_and_prefers_yaml_in_same_directory() {
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path().join(".mytool");
    let nested = temp.path().join("nested");
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::create_dir_all(&nested).unwrap();
    let yml = app_dir.join("config.yml");
    let yaml = app_dir.join("config.yaml");
    std::fs::write(&yml, "name: alternate\n").unwrap();

    let found = DotCfg::new("mytool")
        .yaml()
        .find_in_ancestors_from(&nested)
        .unwrap()
        .unwrap();
    assert_eq!(found.file_path().unwrap(), yml);

    std::fs::write(&yaml, "name: canonical\n").unwrap();
    let found = DotCfg::new("mytool")
        .yaml()
        .find_in_ancestors_from(&nested)
        .unwrap()
        .unwrap();
    assert_eq!(found.file_path().unwrap(), yaml);
}

#[cfg(feature = "async")]
#[tokio::test]
async fn async_paths_find_and_update_yml() {
    let temp = tempfile::tempdir().unwrap();
    let app_dir = temp.path().join(".mytool");
    let nested = temp.path().join("nested");
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::create_dir_all(&nested).unwrap();
    let yml = app_dir.join("config.yml");
    let yaml = app_dir.join("config.yaml");
    std::fs::write(&yml, "name: original\n").unwrap();

    let cfg = DotCfg::new("mytool")
        .yaml()
        .find_in_ancestors_from_async(&nested)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cfg.file_path().unwrap(), yml);
    assert_eq!(
        cfg.load_async::<Settings>().await.unwrap().unwrap().name,
        "original"
    );
    cfg.set_async("name", "updated").await.unwrap();
    assert!(!yaml.exists());
    assert_eq!(
        cfg.load_async::<Settings>().await.unwrap().unwrap().name,
        "updated"
    );
}
