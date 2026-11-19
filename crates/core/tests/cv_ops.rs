use jsoncv_core::{library::Library, Author, CoreError};
use serde_json::json;
use tempfile::TempDir;

fn setup() -> (TempDir, Library) {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    (d, lib)
}

fn edit(lib: &Library, name: &str) {
    let p: json_patch::Patch =
        serde_json::from_value(json!([{"op":"add","path":"/basics","value":{"name":"E"}}]))
            .unwrap();
    lib.patch_cv(name, &p, Author::You, None).unwrap();
}

#[test]
fn duplicate_goes_to_source_folder() {
    let (d, lib) = setup();
    lib.create_cv("a", Some("jobs"), Author::You).unwrap();
    let info = lib.duplicate_cv("b", "a", Author::You).unwrap();
    assert_eq!(info.name, "b");
    assert_eq!(info.folder.as_deref(), Some("jobs"));
    assert!(d.path().join("cvs/jobs/b.cv.json").exists());
    assert!(lib.read_cv("b").unwrap().cv["meta"]
        .get("tailoredFrom")
        .is_none());
    assert_eq!(lib.history("b", 1).unwrap()[0].message, "Duplicated a as b");
}

#[test]
fn duplicate_copies_content() {
    let (_d, lib) = setup();
    lib.duplicate_cv("copy", "my-cv", Author::You).unwrap();
    let a = lib.read_cv("my-cv").unwrap().cv;
    let b = lib.read_cv("copy").unwrap().cv;
    assert_eq!(a, b);
}

#[test]
fn duplicate_existing_slug_fails() {
    let (_d, lib) = setup();
    lib.create_cv("a", Some("jobs"), Author::You).unwrap();
    assert!(matches!(
        lib.duplicate_cv("a", "my-cv", Author::You),
        Err(CoreError::BadName(_))
    ));
    assert!(lib.duplicate_cv("Bad Slug", "my-cv", Author::You).is_err());
}

#[test]
fn create_cv_is_empty_object() {
    let (d, lib) = setup();
    let info = lib.create_cv("blank", Some("drafts"), Author::You).unwrap();
    assert_eq!(info.folder.as_deref(), Some("drafts"));
    assert_eq!(lib.read_cv("blank").unwrap().cv, json!({}));
    assert!(d.path().join("cvs/drafts/blank.cv.json").exists());
    assert_eq!(lib.history("blank", 1).unwrap()[0].message, "Created blank");
    lib.create_cv("top", None, Author::You).unwrap();
    assert!(d.path().join("cvs/top.cv.json").exists());
}

#[test]
fn rename_keeps_folder_and_history() {
    let (d, lib) = setup();
    lib.create_cv("a", Some("jobs"), Author::You).unwrap();
    edit(&lib, "a");
    let n = lib.history("a", 10).unwrap().len();
    let info = lib.rename_cv("a", "b", Author::You).unwrap();
    assert_eq!(info.folder.as_deref(), Some("jobs"));
    assert!(d.path().join("cvs/jobs/b.cv.json").exists());
    assert_eq!(lib.history("b", 10).unwrap().len(), n + 1);
    // Any CV can be renamed, including the one a library starts with.
    lib.rename_cv("my-cv", "general", Author::You).unwrap();
    assert!(lib.read_cv("general").is_ok());
}

#[test]
fn delete_last_cv_refused() {
    let (_d, lib) = setup();
    assert!(matches!(
        lib.delete_cv("my-cv", Author::You),
        Err(CoreError::ReadOnly(m)) if m == "You can't delete the last CV"
    ));
    lib.create_cv("other", None, Author::You).unwrap();
    lib.delete_cv("my-cv", Author::You).unwrap();
    assert!(lib.read_cv("my-cv").is_err());
}

#[test]
fn move_cv_keeps_history() {
    let (d, lib) = setup();
    edit(&lib, "my-cv");
    let n = lib.history("my-cv", 10).unwrap().len();
    let info = lib.move_cv("my-cv", Some("jobs")).unwrap();
    assert_eq!(info.folder.as_deref(), Some("jobs"));
    assert!(d.path().join("cvs/jobs/my-cv.cv.json").exists());
    assert!(!d.path().join("cvs/my-cv.cv.json").exists());
    assert_eq!(lib.history("my-cv", 10).unwrap().len(), n);
    let info = lib.move_cv("my-cv", None).unwrap();
    assert_eq!(info.folder, None);
    assert!(d.path().join("cvs/my-cv.cv.json").exists());
    assert!(matches!(
        lib.move_cv("my-cv", Some("Bad Folder")),
        Err(CoreError::BadName(_))
    ));
}

#[test]
fn create_folder_rules() {
    let (d, lib) = setup();
    lib.create_folder("jobs").unwrap();
    assert!(d.path().join("cvs/jobs").is_dir());
    assert_eq!(lib.list_folders().unwrap(), ["jobs"]);
    assert!(matches!(
        lib.create_folder("jobs"),
        Err(CoreError::BadName(_))
    ));
    assert!(matches!(
        lib.create_folder("A B"),
        Err(CoreError::BadName(_))
    ));
}

#[test]
fn delete_folder_ignores_ds_store() {
    let (d, lib) = setup();
    lib.create_folder("empty").unwrap();
    std::fs::write(d.path().join("cvs/empty/.DS_Store"), "x").unwrap();
    lib.delete_folder("empty").unwrap();
    assert!(!d.path().join("cvs/empty").exists());
    lib.create_cv("a", Some("full"), Author::You).unwrap();
    assert!(matches!(
        lib.delete_folder("full"),
        Err(CoreError::BadName(m)) if m == "Folder full is not empty"
    ));
}

#[test]
fn rename_folder_moves_cvs() {
    let (d, lib) = setup();
    lib.create_cv("a", Some("old"), Author::You).unwrap();
    let n = lib.history("a", 10).unwrap().len();
    lib.rename_folder("old", "new").unwrap();
    assert!(d.path().join("cvs/new/a.cv.json").exists());
    assert_eq!(lib.read_cv("a").unwrap().cv, json!({}));
    assert_eq!(lib.history("a", 10).unwrap().len(), n);
    assert_eq!(
        lib.list_cvs()
            .unwrap()
            .iter()
            .find(|c| c.name == "a")
            .unwrap()
            .folder
            .as_deref(),
        Some("new")
    );
}
