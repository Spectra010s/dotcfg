use dotcfg::{DotCfg, Error};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
struct Settings {
    name: String,
}

#[test]
fn exact_path_reads_and_writes_in_a_nested_directory() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("nested/settings.conf");
    let cfg = DotCfg::new("ignored")
        .at_file(&path)
        .filename("also_ignored");

    assert_eq!(cfg.file_path().unwrap(), path);
    assert_eq!(cfg.dir().unwrap(), temp.path().join("nested"));
    assert_eq!(cfg.load::<Settings>().unwrap(), None);

    let settings = Settings { name: "one".into() };
    cfg.save(&settings).unwrap();
    assert_eq!(cfg.load::<Settings>().unwrap(), Some(settings));
    cfg.set("name", "two").unwrap();
    assert_eq!(cfg.get("name").unwrap(), "two");
    assert!(
        matches!(cfg.delete_dir(), Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::InvalidInput)
    );
    assert!(path.exists());
    cfg.delete_file().unwrap();
    assert!(!path.exists());
}

#[test]
fn directory_selection_after_exact_path_restores_derived_path() {
    let temp = tempfile::tempdir().unwrap();
    let cfg = DotCfg::new("mytool")
        .at_file(temp.path().join("special.conf"))
        .at_dir(temp.path())
        .filename("settings");
    assert_eq!(cfg.file_path().unwrap().file_stem().unwrap(), "settings");

    let project_dir = temp.path().join(".mytool");
    DotCfg::new("mytool")
        .at_dir(&project_dir)
        .save(&Settings::default())
        .unwrap();
    let found = DotCfg::new("mytool")
        .at_file(temp.path().join("special.conf"))
        .find_in_ancestors_from(temp.path())
        .unwrap()
        .unwrap();
    assert_eq!(found.dir().unwrap(), project_dir);
    assert_eq!(found.file_path().unwrap().parent().unwrap(), project_dir);
}

#[test]
fn relative_exact_path_uses_current_directory_as_parent() {
    let cfg = DotCfg::new("ignored").at_file("settings.conf");
    assert_eq!(
        cfg.file_path().unwrap(),
        std::path::PathBuf::from("settings.conf")
    );
    assert_eq!(cfg.dir().unwrap(), std::path::PathBuf::from("."));
}

#[cfg(unix)]
#[test]
fn exact_path_preserves_non_utf8_filename() {
    use std::os::unix::ffi::OsStringExt;

    let temp = tempfile::tempdir().unwrap();
    let name = std::ffi::OsString::from_vec(b"settings-\xff.conf".to_vec());
    let path = temp.path().join(name);
    let cfg = DotCfg::new("ignored").at_file(&path);
    cfg.save(&Settings::default()).unwrap();
    assert_eq!(cfg.file_path().unwrap(), path);
    assert_eq!(cfg.load::<Settings>().unwrap(), Some(Settings::default()));
}

#[cfg(feature = "async")]
#[tokio::test]
async fn exact_path_works_with_async_io() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("nested/settings.conf");
    let cfg = DotCfg::new("ignored").at_file(&path);
    cfg.save_async(&Settings { name: "one".into() })
        .await
        .unwrap();
    assert_eq!(
        cfg.load_async::<Settings>().await.unwrap().unwrap().name,
        "one"
    );
    cfg.set_async("name", "two").await.unwrap();
    assert_eq!(cfg.get_async("name").await.unwrap(), "two");
    assert!(
        matches!(cfg.delete_dir_async().await, Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::InvalidInput)
    );
    assert!(path.exists());
}
