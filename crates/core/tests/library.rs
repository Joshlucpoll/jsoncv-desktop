use jsoncv_core::{library::Library, CoreError};
use tempfile::TempDir;

#[test]
fn open_existing_folder_preserves_content() {
    let d = TempDir::new().unwrap();
    std::fs::create_dir(d.path().join(".git")).unwrap();
    std::fs::write(d.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    std::fs::create_dir(d.path().join("cvs")).unwrap();
    std::fs::write(
        d.path().join("cvs/my-cv.cv.json"),
        "{\"basics\":{\"name\":\"Z\"}}\n",
    )
    .unwrap();
    std::fs::write(d.path().join("notes.txt"), "hi").unwrap();
    Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    assert_eq!(
        std::fs::read(d.path().join("cvs/my-cv.cv.json")).unwrap(),
        b"{\"basics\":{\"name\":\"Z\"}}\n"
    );
    assert_eq!(std::fs::read(d.path().join("notes.txt")).unwrap(), b"hi");
    // An old Git folder is left untouched.
    assert_eq!(
        std::fs::read(d.path().join(".git/HEAD")).unwrap(),
        b"ref: refs/heads/main\n"
    );
}

#[test]
fn list_reports_broken_cv() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    std::fs::write(d.path().join("cvs/bad.cv.json"), "{").unwrap();
    let l = lib.list_cvs().unwrap();
    assert_eq!(l.len(), 2);
    assert_eq!(l[0].name, "bad");
    assert!(l[0].error.is_some());
    assert_eq!(l[1].name, "my-cv");
    assert!(l[1].error.is_none());
}

#[test]
fn read_returns_stable_base() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    let a = lib.read_cv("my-cv").unwrap().base;
    assert_eq!(a, lib.read_cv("my-cv").unwrap().base);
    std::fs::write(d.path().join("cvs/my-cv.cv.json"), "{}\n").unwrap();
    assert_ne!(a, lib.read_cv("my-cv").unwrap().base);
}

#[test]
fn bad_names_rejected() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    for n in ["../x", "A B", "tailored/x", "main.cv"] {
        assert!(matches!(lib.read_cv(n), Err(CoreError::BadName(_))), "{n}");
    }
}

#[test]
fn list_reports_bad_file_name() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    std::fs::write(d.path().join("cvs/My CV.cv.json"), "{}").unwrap();
    let l = lib.list_cvs().unwrap();
    let e = l.iter().find(|c| c.name == "My CV").unwrap();
    assert!(e.error.as_deref().unwrap().starts_with("Invalid file name"));
}

#[test]
fn open_does_not_commit_invalid_main() {
    let d = TempDir::new().unwrap();
    std::fs::create_dir(d.path().join("cvs")).unwrap();
    std::fs::write(d.path().join("cvs/my-cv.cv.json"), "{").unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    assert!(lib.history("my-cv", 5).unwrap().is_empty());
    assert_eq!(
        std::fs::read(d.path().join("cvs/my-cv.cv.json")).unwrap(),
        b"{"
    );
}

#[test]
fn open_configured_refuses_missing_non_default() {
    let d = TempDir::new().unwrap();
    let default = d.path().join("default");
    let gone = d.path().join("unplugged-drive/CVs");
    let db = d.path().join("history.db");
    assert!(matches!(
        Library::open_configured(&gone, &default, &db),
        Err(CoreError::NotFound(_))
    ));
    assert!(!gone.exists());
    // The default is created on first run; an existing configured folder opens.
    assert_eq!(
        Library::open_configured(&default, &default, &db)
            .unwrap()
            .root(),
        default
    );
    let other = d.path().join("other");
    std::fs::create_dir(&other).unwrap();
    assert_eq!(
        Library::open_configured(&other, &default, &db)
            .unwrap()
            .root(),
        other
    );
}
