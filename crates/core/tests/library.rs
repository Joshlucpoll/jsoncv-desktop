use jsoncv_core::{config::is_in_sync_folder, library::Library, schema, CoreError};
use tempfile::TempDir;

#[test]
fn open_creates_layout() {
    let d = TempDir::new().unwrap();
    Library::open(d.path()).unwrap();
    let main = std::fs::read_to_string(d.path().join("main.cv.json")).unwrap();
    assert_eq!(main, include_str!("../resources/sample.cv.json"));
    schema::validate(&serde_json::from_str(&main).unwrap()).unwrap();
    for f in ["tailored", "themes", "assets", "exports"] {
        assert!(d.path().join(f).is_dir(), "{f}");
    }
    let gi = std::fs::read_to_string(d.path().join(".gitignore")).unwrap();
    assert!(gi.lines().any(|l| l == "exports/"));
    git2::Repository::open(d.path()).unwrap();
}

#[test]
fn open_existing_folder_preserves_content() {
    let d = TempDir::new().unwrap();
    let repo = git2::Repository::init(d.path()).unwrap();
    std::fs::write(d.path().join("main.cv.json"), "{\"basics\":{\"name\":\"Z\"}}\n").unwrap();
    std::fs::write(d.path().join("notes.txt"), "hi").unwrap();
    let sig = git2::Signature::now("You", "jsoncv@localhost").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(std::path::Path::new("main.cv.json")).unwrap();
    let tree = repo.find_tree(idx.write_tree().unwrap()).unwrap();
    let head = repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[]).unwrap();

    Library::open(d.path()).unwrap();
    assert_eq!(std::fs::read(d.path().join("main.cv.json")).unwrap(), b"{\"basics\":{\"name\":\"Z\"}}\n");
    assert_eq!(std::fs::read(d.path().join("notes.txt")).unwrap(), b"hi");
    let repo = git2::Repository::open(d.path()).unwrap();
    assert_eq!(repo.head().unwrap().peel_to_commit().unwrap().id(), head);
}

#[test]
fn list_reports_broken_cv() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path()).unwrap();
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
    let lib = Library::open(d.path()).unwrap();
    let a = lib.read_cv("main").unwrap().base;
    assert_eq!(a, lib.read_cv("main").unwrap().base);
    std::fs::write(d.path().join("main.cv.json"), "{}\n").unwrap();
    assert_ne!(a, lib.read_cv("main").unwrap().base);
}

#[test]
fn bad_names_rejected() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path()).unwrap();
    for n in ["../x", "tailored/A B", "other"] {
        assert!(matches!(lib.read_cv(n), Err(CoreError::BadName(_))), "{n}");
    }
}

#[test]
fn sync_folder_detection() {
    let home = directories::BaseDirs::new().unwrap().home_dir().to_path_buf();
    for p in ["Library/Mobile Documents/x", "Library/CloudStorage/Dropbox/x", "Dropbox/x"] {
        assert!(is_in_sync_folder(&home.join(p)), "{p}");
    }
    assert!(!is_in_sync_folder(&home.join("Documents/CVs")));
}

#[test]
fn open_appends_to_existing_gitignore() {
    let d = TempDir::new().unwrap();
    std::fs::write(d.path().join(".gitignore"), "node_modules/").unwrap();
    Library::open(d.path()).unwrap();
    let gi = std::fs::read_to_string(d.path().join(".gitignore")).unwrap();
    assert!(gi.starts_with("node_modules/"));
    assert!(gi.lines().any(|l| l == "exports/"));
}

#[test]
fn list_reports_bad_file_name() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path()).unwrap();
    std::fs::write(d.path().join("tailored/My CV.cv.json"), "{}").unwrap();
    let l = lib.list_cvs().unwrap();
    let e = l.iter().find(|c| c.name == "tailored/My CV").unwrap();
    assert!(e.error.as_deref().unwrap().starts_with("Invalid file name"));
}

#[test]
fn list_reports_missing_main() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path()).unwrap();
    std::fs::remove_file(d.path().join("main.cv.json")).unwrap();
    let l = lib.list_cvs().unwrap();
    assert_eq!(l[0].name, "main");
    assert_eq!(l[0].error.as_deref(), Some("main.cv.json is missing"));
}
