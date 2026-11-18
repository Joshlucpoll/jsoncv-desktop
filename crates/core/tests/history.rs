use jsoncv_core::history::{commit, with_lock};
use jsoncv_core::{library::Library, Author, CoreError};
use std::path::Path;
use tempfile::TempDir;

fn cv(name: &str) -> String {
    format!("{{\"basics\":{{\"name\":\"{name}\"}}}}\n")
}

fn head(d: &Path) -> (String, String, String) {
    let r = git2::Repository::open(d).unwrap();
    let c = r.head().unwrap().peel_to_commit().unwrap();
    let a = c.author();
    (a.name().unwrap().into(), a.email().unwrap().into(), c.message().unwrap().into())
}

#[test]
fn open_makes_initial_commit() {
    let d = TempDir::new().unwrap();
    Library::open(d.path()).unwrap();
    assert_eq!(head(d.path()), ("You".into(), "jsoncv@localhost".into(), "Create library".into()));
    let n = git2::Repository::open(d.path()).unwrap().head().unwrap().peel_to_commit().unwrap().parent_count();
    assert_eq!(n, 0);
}

#[test]
fn commit_noop_when_unchanged() {
    let d = TempDir::new().unwrap();
    Library::open(d.path()).unwrap();
    std::fs::write(d.path().join("main.cv.json"), cv("Ada")).unwrap();
    let p = Path::new("main.cv.json");
    assert!(!commit(d.path(), &[p], Author::You, "edit").unwrap().is_empty());
    assert_eq!(commit(d.path(), &[p], Author::You, "edit").unwrap(), "");
}

#[test]
fn sweep_commits_external_edit() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path()).unwrap();
    std::fs::write(d.path().join("main.cv.json"), cv("Ada")).unwrap();
    std::fs::write(d.path().join("exports/x.pdf"), "x").unwrap();
    let sha = lib.sweep_external_locked().unwrap().unwrap();
    let r = git2::Repository::open(d.path()).unwrap();
    let c = r.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(c.id().to_string(), sha);
    assert_eq!(c.author().name().unwrap(), "External");
    assert_eq!(c.message().unwrap(), "External changes");
    assert_eq!(lib.sweep_external_locked().unwrap(), None);
}

#[test]
fn sweep_skips_invalid_file() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path()).unwrap();
    std::fs::write(d.path().join("main.cv.json"), "{").unwrap();
    assert_eq!(lib.sweep_external_locked().unwrap(), None);
    let r = git2::Repository::open(d.path()).unwrap();
    let st = r.status_file(Path::new("main.cv.json")).unwrap();
    assert!(st.contains(git2::Status::WT_MODIFIED));
    assert!(!st.intersects(git2::Status::INDEX_MODIFIED));
}

#[test]
fn history_filters_by_file() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path()).unwrap();
    std::fs::write(d.path().join("tailored/a.cv.json"), cv("A")).unwrap();
    commit(d.path(), &[Path::new("tailored/a.cv.json")], Author::Agent, "make a").unwrap();
    std::fs::write(d.path().join("main.cv.json"), cv("Ada")).unwrap();
    commit(d.path(), &[Path::new("main.cv.json")], Author::You, "edit main").unwrap();
    let h = lib.history("tailored/a", 10).unwrap();
    assert_eq!(h.len(), 1);
    assert_eq!((h[0].author.as_str(), h[0].message.as_str()), ("Agent", "make a"));
    let m = lib.history("main", 10).unwrap();
    assert_eq!(m.iter().map(|v| v.message.as_str()).collect::<Vec<_>>(), ["edit main", "Create library"]);
    assert_eq!(lib.history("main", 1).unwrap().len(), 1);
}

#[test]
fn diff_shows_changed_field() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path()).unwrap();
    std::fs::write(d.path().join("main.cv.json"), cv("Ada")).unwrap();
    let old = commit(d.path(), &[Path::new("main.cv.json")], Author::You, "ada").unwrap();
    std::fs::write(d.path().join("main.cv.json"), cv("Grace")).unwrap();
    commit(d.path(), &[Path::new("main.cv.json")], Author::You, "grace").unwrap();
    let diff = lib.diff("main", &old).unwrap();
    assert!(diff.contains("-    \"name\": \"Ada\""), "{diff}");
    assert!(diff.contains("+    \"name\": \"Grace\""), "{diff}");
    assert_eq!(lib.read_cv_at("main", &old).unwrap()["basics"]["name"], "Ada");
    let first = lib.history("main", 10).unwrap().pop().unwrap().commit;
    assert!(matches!(lib.read_cv_at("tailored/zz", &first), Err(CoreError::NotFound(_))));
}

#[test]
fn lock_times_out() {
    let d = TempDir::new().unwrap();
    Library::open(d.path()).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let root = d.path().to_path_buf();
    let h = std::thread::spawn(move || {
        with_lock(&root, || {
            tx.send(()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2600));
            Ok(())
        })
    });
    rx.recv().unwrap();
    let t = std::time::Instant::now();
    let r = with_lock(d.path(), || Ok(()));
    assert!(matches!(r, Err(CoreError::Locked)));
    assert!(t.elapsed() >= std::time::Duration::from_millis(1900));
    h.join().unwrap().unwrap();
    with_lock(d.path(), || Ok(())).unwrap();
}

#[test]
fn commit_ignores_unrelated_staged_files() {
    let d = TempDir::new().unwrap();
    Library::open(d.path()).unwrap();
    std::fs::write(d.path().join("notes.txt"), "hi").unwrap();
    let r = git2::Repository::open(d.path()).unwrap();
    let mut idx = r.index().unwrap();
    idx.add_path(Path::new("notes.txt")).unwrap();
    idx.write().unwrap();
    std::fs::write(d.path().join("main.cv.json"), cv("Ada")).unwrap();
    commit(d.path(), &[Path::new("main.cv.json")], Author::You, "edit").unwrap();
    let tree = r.head().unwrap().peel_to_commit().unwrap().tree().unwrap();
    assert!(tree.get_path(Path::new("notes.txt")).is_err());
    assert!(tree.get_path(Path::new("main.cv.json")).is_ok());
    // still staged, and main is clean
    assert!(r.status_file(Path::new("notes.txt")).unwrap().contains(git2::Status::INDEX_NEW));
    assert!(r.status_file(Path::new("main.cv.json")).unwrap().is_empty());
}

#[test]
fn sweep_stages_deletion() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path()).unwrap();
    std::fs::write(d.path().join("themes/t.html"), "x").unwrap();
    lib.sweep_external_locked().unwrap().unwrap();
    std::fs::remove_file(d.path().join("themes/t.html")).unwrap();
    assert!(lib.sweep_external_locked().unwrap().is_some());
    let r = git2::Repository::open(d.path()).unwrap();
    let tree = r.head().unwrap().peel_to_commit().unwrap().tree().unwrap();
    assert!(tree.get_path(Path::new("themes/t.html")).is_err());
}

#[test]
fn unknown_commit_is_not_found() {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path()).unwrap();
    let bad = "0123456789abcdef0123456789abcdef01234567";
    assert!(matches!(lib.read_cv_at("main", bad), Err(CoreError::NotFound(_))));
    assert!(matches!(lib.diff("main", bad), Err(CoreError::NotFound(_))));
}
