use jsoncv_core::{config::is_in_sync_folder, library::Library, schema, CoreError};
use tempfile::TempDir;

#[test]
fn open_creates_layout() {
    let d = TempDir::new().unwrap();
    Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    let main = std::fs::read_to_string(d.path().join("main.cv.json")).unwrap();
    assert_eq!(main, include_str!("../resources/sample.cv.json"));
    schema::validate(&serde_json::from_str(&main).unwrap()).unwrap();
    for f in ["tailored", "themes", "assets", "exports"] {
        assert!(d.path().join(f).is_dir(), "{f}");
    }
    assert!(!d.path().join(".git").exists());
    assert_eq!(
        std::fs::read_to_string(d.path().join(".jsoncv-id"))
            .unwrap()
            .trim()
            .len(),
        32
    );
}

#[test]
fn open_existing_folder_preserves_content() {
    let d = TempDir::new().unwrap();
    std::fs::create_dir(d.path().join(".git")).unwrap();
    std::fs::write(d.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    std::fs::write(
        d.path().join("main.cv.json"),
        "{\"basics\":{\"name\":\"Z\"}}\n",
    )
    .unwrap();
    std::fs::write(d.path().join("notes.txt"), "hi").unwrap();
    Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    assert_eq!(
        std::fs::read(d.path().join("main.cv.json")).unwrap(),
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
    std::fs::write(d.path().join("tailored/bad.cv.json"), "{").unwrap();
    let l = lib.list_cvs().unwrap();
    assert_eq!(l.len(), 2);
    assert_eq!(l[0].name, "main");
    assert!(l[0].error.is_none());
    assert_eq!(l[1].name, "tailored/bad");
    assert!(l[1].error.is_some());
}

#[test]
fn read_returns_stable_base() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    let a = lib.read_cv("main").unwrap().base;
    assert_eq!(a, lib.read_cv("main").unwrap().base);
    std::fs::write(d.path().join("main.cv.json"), "{}\n").unwrap();
    assert_ne!(a, lib.read_cv("main").unwrap().base);
}

#[test]
fn bad_names_rejected() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    for n in ["../x", "tailored/A B", "other"] {
        assert!(matches!(lib.read_cv(n), Err(CoreError::BadName(_))), "{n}");
    }
}

#[test]
fn sync_folder_detection() {
    let home = directories::BaseDirs::new()
        .unwrap()
        .home_dir()
        .to_path_buf();
    for p in [
        "Library/Mobile Documents/x",
        "Library/CloudStorage/Dropbox/x",
        "Dropbox/x",
    ] {
        assert!(is_in_sync_folder(&home.join(p)), "{p}");
    }
    assert!(!is_in_sync_folder(&home.join("Documents/CVs")));
}

#[test]
fn list_reports_bad_file_name() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    std::fs::write(d.path().join("tailored/My CV.cv.json"), "{}").unwrap();
    let l = lib.list_cvs().unwrap();
    let e = l.iter().find(|c| c.name == "tailored/My CV").unwrap();
    assert!(e.error.as_deref().unwrap().starts_with("Invalid file name"));
}

#[test]
fn list_reports_missing_main() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    std::fs::remove_file(d.path().join("main.cv.json")).unwrap();
    let l = lib.list_cvs().unwrap();
    assert_eq!(l[0].name, "main");
    assert_eq!(l[0].error.as_deref(), Some("main.cv.json is missing"));
}

#[test]
fn open_does_not_commit_invalid_main() {
    let d = TempDir::new().unwrap();
    std::fs::write(d.path().join("main.cv.json"), "{").unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    assert!(lib.history("main", 5).unwrap().is_empty());
    assert_eq!(std::fs::read(d.path().join("main.cv.json")).unwrap(), b"{");
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
    assert!(matches!(
        Library::open_configured(&default, &default, &d.path().join("gone/history.db")),
        Err(CoreError::NotFound(m)) if m.starts_with("history folder")
    ));
    let other = d.path().join("other");
    std::fs::create_dir(&other).unwrap();
    assert_eq!(
        Library::open_configured(&other, &default, &db)
            .unwrap()
            .root(),
        other
    );
}
