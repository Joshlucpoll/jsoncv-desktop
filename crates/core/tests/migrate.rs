//! Libraries from before `cvs/`: `main.cv.json` at the root, copies in `tailored/<slug>.cv.json`.
use jsoncv_core::library::Library;
use rusqlite::Connection;
use tempfile::TempDir;

const ID: &str = "0123456789abcdef0123456789abcdef";

fn db(d: &TempDir) -> std::path::PathBuf {
    d.path().join(".history.db")
}

fn put(d: &TempDir, rel: &str, body: &str) {
    let p = d.path().join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

fn cv(name: &str) -> String {
    format!("{{\"basics\":{{\"name\":\"{name}\"}}}}\n")
}

/// Add an old-style history row for `cv`, as the previous app version would have.
fn row(d: &TempDir, cv: &str, body: Option<&str>) {
    let c = Connection::open(db(d)).unwrap();
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS versions(id INTEGER PRIMARY KEY, library TEXT NOT NULL,
         cv TEXT NOT NULL, author TEXT NOT NULL, message TEXT NOT NULL, time_ms INTEGER NOT NULL,
         sha TEXT, body BLOB);",
    )
    .unwrap();
    c.execute(
        "INSERT INTO versions(library, cv, author, message, time_ms, sha, body)
         VALUES (?1, ?2, 'You', 'old', 1, NULL, ?3)",
        rusqlite::params![ID, cv, body.map(|b| b.as_bytes().to_vec())],
    )
    .unwrap();
}

fn rows(d: &TempDir, cv: &str) -> i64 {
    Connection::open(db(d))
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM versions WHERE library = ?1 AND cv = ?2",
            [ID, cv],
            |r| r.get(0),
        )
        .unwrap()
}

fn old_library(d: &TempDir) {
    put(d, ".jsoncv-id", &format!("{ID}\n"));
    put(d, "main.cv.json", &cv("Main"));
    put(d, "tailored/acme.cv.json", &cv("Acme"));
    row(d, "main", Some(&cv("Main")));
    row(d, "tailored/acme", Some(&cv("Acme")));
}

fn open(d: &TempDir) -> Library {
    Library::open(d.path(), &db(d)).unwrap()
}

#[test]
fn migrates_old_layout_with_history() {
    let d = TempDir::new().unwrap();
    old_library(&d);
    let lib = open(&d);
    assert_eq!(
        std::fs::read_to_string(d.path().join("cvs/main.cv.json")).unwrap(),
        cv("Main")
    );
    assert_eq!(
        std::fs::read_to_string(d.path().join("cvs/acme.cv.json")).unwrap(),
        cv("Acme")
    );
    assert!(!d.path().join("main.cv.json").exists());
    assert!(!d.path().join("tailored").exists());
    assert!(!d.path().join("cvs/my-cv.cv.json").exists());
    assert!(lib
        .history("acme", 10)
        .unwrap()
        .iter()
        .any(|v| v.message == "old"));
    assert_eq!(rows(&d, "tailored/acme"), 0);
    let names: Vec<_> = lib
        .list_cvs()
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(names, ["acme", "main"]);
}

#[test]
fn migration_rerun_is_noop() {
    let d = TempDir::new().unwrap();
    old_library(&d);
    open(&d);
    let before = (rows(&d, "main"), rows(&d, "acme"));
    open(&d);
    assert_eq!((rows(&d, "main"), rows(&d, "acme")), before);
    assert!(d.path().join("cvs/acme.cv.json").exists());
}

#[test]
fn tailored_main_collides() {
    let d = TempDir::new().unwrap();
    old_library(&d);
    put(&d, "tailored/main.cv.json", &cv("Other"));
    row(&d, "tailored/main", Some(&cv("Other")));
    open(&d);
    assert_eq!(
        std::fs::read_to_string(d.path().join("cvs/main.cv.json")).unwrap(),
        cv("Main")
    );
    assert_eq!(
        std::fs::read_to_string(d.path().join("cvs/main-2.cv.json")).unwrap(),
        cv("Other")
    );
    assert_eq!(rows(&d, "tailored/main"), 0);
    assert!(rows(&d, "main-2") >= 1);
}

#[test]
fn recreated_root_main_becomes_main_2() {
    let d = TempDir::new().unwrap();
    old_library(&d);
    open(&d);
    // An agent still running the old code writes main.cv.json at the root again.
    put(&d, "main.cv.json", &cv("Again"));
    open(&d);
    assert_eq!(
        std::fs::read_to_string(d.path().join("cvs/main.cv.json")).unwrap(),
        cv("Main")
    );
    assert_eq!(
        std::fs::read_to_string(d.path().join("cvs/main-2.cv.json")).unwrap(),
        cv("Again")
    );
    assert!(!d.path().join("main.cv.json").exists());
}

#[test]
fn history_only_migration() {
    let d = TempDir::new().unwrap();
    // A second Mac: the files were already moved, its own history still has old names.
    put(&d, ".jsoncv-id", &format!("{ID}\n"));
    put(&d, "cvs/acme.cv.json", &cv("Acme"));
    row(&d, "tailored/acme", Some(&cv("Acme")));
    let lib = open(&d);
    assert!(lib
        .history("acme", 10)
        .unwrap()
        .iter()
        .any(|v| v.message == "old"));
    assert_eq!(rows(&d, "tailored/acme"), 0);
}

#[test]
fn deleted_cv_rows_renamed() {
    let d = TempDir::new().unwrap();
    old_library(&d);
    row(&d, "tailored/gone", Some(&cv("Gone")));
    row(&d, "tailored/gone", None);
    open(&d);
    assert_eq!(rows(&d, "tailored/gone"), 0);
    assert_eq!(rows(&d, "gone"), 2);
    assert!(!d.path().join("cvs/gone.cv.json").exists());
}

#[test]
fn concurrent_opens_both_succeed() {
    // The app and an MCP server can start together on an old library; both opens must work.
    for _ in 0..20 {
        let d = TempDir::new().unwrap();
        old_library(&d);
        for i in 0..30 {
            put(&d, &format!("tailored/t{i}.cv.json"), &cv("T"));
        }
        let root = d.path().to_path_buf();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let (root, b) = (root.clone(), barrier.clone());
                std::thread::spawn(move || {
                    b.wait();
                    Library::open(&root, &root.join(".history.db")).map(|_| ())
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap().unwrap();
        }
        assert!(d.path().join("cvs/t29.cv.json").exists());
    }
}
