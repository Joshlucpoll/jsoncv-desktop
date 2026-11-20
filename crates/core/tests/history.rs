use jsoncv_core::{library::Library, Author, CoreError};
use serde_json::json;
use tempfile::TempDir;

fn cv(name: &str) -> String {
    format!("{{\"basics\":{{\"name\":\"{name}\"}}}}\n")
}

fn open(d: &TempDir) -> Library {
    Library::open(d.path(), &d.path().join(".history.db")).unwrap()
}

/// Write `name` to main.cv.json as an outside edit and record it; returns the new version id.
fn external(lib: &Library, name: &str) -> String {
    std::fs::write(lib.root().join("cvs/my-cv.cv.json"), cv(name)).unwrap();
    lib.sweep_external_locked().unwrap().unwrap()
}

fn messages(lib: &Library, name: &str) -> Vec<String> {
    lib.history(name, 100)
        .unwrap()
        .into_iter()
        .map(|v| v.message)
        .collect()
}

#[test]
fn open_records_create_library() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    let h = lib.history("my-cv", 10).unwrap();
    assert_eq!(h.len(), 1);
    assert_eq!(
        (h[0].author.as_str(), h[0].message.as_str()),
        ("You", "Create library")
    );
    // Reopening does not record again.
    assert_eq!(open(&d).history("my-cv", 10).unwrap().len(), 1);
}

#[test]
fn save_noop_when_unchanged() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    // The first replace may reformat the sample's bytes; the second cannot change anything.
    for _ in 0..2 {
        let doc = lib.read_cv("my-cv").unwrap();
        let s = lib
            .replace_cv("my-cv", doc.cv, &doc.base, Author::You, None)
            .unwrap();
        if s.commit.is_empty() {
            return;
        }
    }
    panic!("an unchanged save recorded a version");
}

#[test]
fn sweep_records_external_edit() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    std::fs::write(d.path().join("exports/x.pdf"), "x").unwrap();
    let id = external(&lib, "Ada");
    let top = &lib.history("my-cv", 1).unwrap()[0];
    assert_eq!(top.commit, id);
    assert_eq!(
        (top.author.as_str(), top.message.as_str()),
        ("External", "External changes")
    );
    assert_eq!(lib.sweep_external_locked().unwrap(), None);
}

#[test]
fn sweep_skips_invalid_file() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    std::fs::write(d.path().join("cvs/my-cv.cv.json"), "{").unwrap();
    assert_eq!(lib.sweep_external_locked().unwrap(), None);
    assert_eq!(lib.history("my-cv", 10).unwrap().len(), 1);
}

#[test]
fn sweep_records_outside_delete() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    std::fs::write(d.path().join("cvs/a.cv.json"), cv("A")).unwrap();
    lib.sweep_external_locked().unwrap().unwrap();
    std::fs::remove_file(d.path().join("cvs/a.cv.json")).unwrap();
    assert!(lib.sweep_external_locked().unwrap().is_some());
    assert_eq!(lib.history("a", 10).unwrap().len(), 2);
    assert_eq!(lib.sweep_external_locked().unwrap(), None);
}

#[test]
fn history_filters_by_cv() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    std::fs::write(d.path().join("cvs/a.cv.json"), cv("A")).unwrap();
    lib.sweep_external_locked().unwrap();
    let p: json_patch::Patch =
        serde_json::from_value(json!([{"op":"replace","path":"/basics/name","value":"Ada"}]))
            .unwrap();
    lib.patch_cv("a", &p, Author::Agent, Some("make a"))
        .unwrap();
    external(&lib, "Grace");
    assert_eq!(messages(&lib, "a"), ["make a", "External changes"]);
    assert_eq!(
        messages(&lib, "my-cv"),
        ["External changes", "Create library"]
    );
    assert_eq!(lib.history("my-cv", 1).unwrap().len(), 1);
}

#[test]
fn diff_shows_changed_field() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    let old = external(&lib, "Ada");
    external(&lib, "Grace");
    let diff = lib.diff("my-cv", &old).unwrap();
    assert!(diff.contains("-    \"name\": \"Ada\""), "{diff}");
    assert!(diff.contains("+    \"name\": \"Grace\""), "{diff}");
    assert!(diff.contains(&format!("--- v{old}")), "{diff}");
    assert_eq!(
        lib.read_cv_at("my-cv", &old).unwrap()["basics"]["name"],
        "Ada"
    );
    assert!(matches!(
        lib.read_cv_at("zz", &old),
        Err(CoreError::NotFound(_))
    ));
}

#[test]
fn change_diff_shows_only_that_version() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    external(&lib, "Ada");
    let mid = external(&lib, "Grace");
    external(&lib, "Hedy");
    let diff = lib.change_diff("my-cv", &mid).unwrap();
    assert!(diff.contains("-    \"name\": \"Ada\""), "{diff}");
    assert!(diff.contains("+    \"name\": \"Grace\""), "{diff}");
    assert!(!diff.contains("Hedy"), "{diff}");
    let oldest = lib.history("my-cv", 100).unwrap().pop().unwrap().commit;
    assert!(lib
        .change_diff("my-cv", &oldest)
        .unwrap()
        .contains("--- empty"));
}

#[test]
fn restore_adds_a_version() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    let old = external(&lib, "Ada");
    external(&lib, "Grace");
    lib.restore("my-cv", &old, Author::You).unwrap();
    assert_eq!(lib.read_cv("my-cv").unwrap().cv["basics"]["name"], "Ada");
    assert_eq!(
        messages(&lib, "my-cv")[0],
        format!("Restored version v{old}")
    );
}

#[test]
fn lock_times_out() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    let (tx, rx) = std::sync::mpsc::channel();
    let other = lib.clone();
    let h = std::thread::spawn(move || {
        other.locked(|_| {
            tx.send(()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2600));
            Ok(())
        })
    });
    rx.recv().unwrap();
    let t = std::time::Instant::now();
    assert!(matches!(lib.locked(|_| Ok(())), Err(CoreError::Locked)));
    assert!(t.elapsed() >= std::time::Duration::from_millis(1900));
    h.join().unwrap().unwrap();
    lib.locked(|_| Ok(())).unwrap();
}

#[test]
fn unknown_version_is_not_found() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    for bad in ["999999", "abc", ""] {
        assert!(
            matches!(lib.read_cv_at("my-cv", bad), Err(CoreError::NotFound(_))),
            "{bad}"
        );
        assert!(
            matches!(lib.diff("my-cv", bad), Err(CoreError::NotFound(_))),
            "{bad}"
        );
    }
}

#[test]
fn history_follows_moved_folder_and_libraries_do_not_mix() {
    let d = TempDir::new().unwrap();
    let db = d.path().join("history.db");
    let a = d.path().join("a");
    let lib = Library::open(&a, &db).unwrap();
    std::fs::write(a.join("cvs/my-cv.cv.json"), cv("Ada")).unwrap();
    lib.sweep_external_locked().unwrap().unwrap();
    // A second library in the same database starts with its own history.
    let other = Library::open(&d.path().join("other"), &db).unwrap();
    assert_eq!(other.history("my-cv", 10).unwrap().len(), 1);
    // Moving the folder keeps its history, because `.jsoncv-id` moves with it.
    let moved = d.path().join("moved");
    std::fs::rename(&a, &moved).unwrap();
    let lib = Library::open(&moved, &db).unwrap();
    assert_eq!(
        messages(&lib, "my-cv"),
        ["External changes", "Create library"]
    );
}

#[test]
fn missing_history_folder_is_created_and_unwritable_is_an_error() {
    let d = TempDir::new().unwrap();
    let db = d.path().join("deep/nested/history.db");
    Library::open(&d.path().join("cvs"), &db).unwrap();
    assert!(db.exists());
    let file = d.path().join("plain-file");
    std::fs::write(&file, "x").unwrap();
    assert!(Library::open(&d.path().join("cvs"), &file.join("history.db")).is_err());
}

#[test]
fn agent_has_edited_tracks_agent_versions() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    std::fs::write(d.path().join("cvs/a.cv.json"), cv("A")).unwrap();
    lib.sweep_external_locked().unwrap();
    assert!(!lib.agent_has_edited().unwrap());
    let p: json_patch::Patch =
        serde_json::from_value(json!([{"op":"replace","path":"/basics/name","value":"Ada"}]))
            .unwrap();
    lib.patch_cv("a", &p, Author::You, None).unwrap();
    assert!(!lib.agent_has_edited().unwrap());
    let p: json_patch::Patch =
        serde_json::from_value(json!([{"op":"replace","path":"/basics/name","value":"Grace"}]))
            .unwrap();
    lib.patch_cv("a", &p, Author::Agent, None).unwrap();
    assert!(lib.agent_has_edited().unwrap());
}
