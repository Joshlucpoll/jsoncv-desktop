use jsoncv_core::{library::Library, schema, CoreError};
use tempfile::TempDir;

fn open(d: &TempDir) -> Library {
    Library::open(d.path(), &d.path().join(".history.db")).unwrap()
}

fn put(d: &TempDir, rel: &str, body: &str) {
    let p = d.path().join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

#[test]
fn new_library_gets_sample_in_cvs() {
    let d = TempDir::new().unwrap();
    open(&d);
    let s = std::fs::read_to_string(d.path().join("cvs/my-cv.cv.json")).unwrap();
    assert_eq!(s, include_str!("../resources/sample.cv.json"));
    schema::validate(&serde_json::from_str(&s).unwrap()).unwrap();
    assert!(!d.path().join("tailored").exists());
    assert!(!d.path().join("main.cv.json").exists());
    for f in ["cvs", "themes", "assets", "exports"] {
        assert!(d.path().join(f).is_dir(), "{f}");
    }
}

#[test]
fn sample_not_readded() {
    let d = TempDir::new().unwrap();
    open(&d);
    std::fs::remove_file(d.path().join("cvs/my-cv.cv.json")).unwrap();
    assert!(open(&d).list_cvs().unwrap().is_empty());
    assert!(!d.path().join("cvs/my-cv.cv.json").exists());
}

#[test]
fn finds_cv_in_folder() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    put(&d, "cvs/jobs/stripe.cv.json", "{}");
    assert_eq!(lib.read_cv("stripe").unwrap().cv, serde_json::json!({}));
    let l = lib.list_cvs().unwrap();
    let s = l.iter().find(|c| c.name == "stripe").unwrap();
    assert_eq!(s.folder.as_deref(), Some("jobs"));
    assert_eq!(s.error, None);
    let m = l.iter().find(|c| c.name == "my-cv").unwrap();
    assert_eq!(m.folder, None);
    assert_eq!(lib.list_folders().unwrap(), vec!["jobs".to_string()]);
}

#[test]
fn duplicate_slug_is_one_error_entry() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    put(&d, "cvs/a/stripe.cv.json", "{}");
    put(&d, "cvs/b/stripe.cv.json", "{}");
    let l = lib.list_cvs().unwrap();
    let s: Vec<_> = l.iter().filter(|c| c.name == "stripe").collect();
    assert_eq!(s.len(), 1);
    assert_eq!(
        s[0].error.as_deref(),
        Some("stripe.cv.json is in both a/ and b/; rename or remove one")
    );
    assert!(matches!(lib.read_cv("stripe"), Err(CoreError::BadName(_))));
}

#[test]
fn duplicate_slug_with_top_level_names_cvs() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    put(&d, "cvs/a/my-cv.cv.json", "{}");
    let l = lib.list_cvs().unwrap();
    let s = l.iter().find(|c| c.name == "my-cv").unwrap();
    assert_eq!(
        s.error.as_deref(),
        Some("my-cv.cv.json is in both cvs/ and a/; rename or remove one")
    );
}

#[test]
fn finder_move_keeps_history() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    let p: json_patch::Patch = serde_json::from_value(
        serde_json::json!([{"op":"replace","path":"/basics/name","value":"Moved"}]),
    )
    .unwrap();
    lib.patch_cv("my-cv", &p, jsoncv_core::Author::You, None)
        .unwrap();
    let before = lib.history("my-cv", 10).unwrap().len();
    std::fs::create_dir(d.path().join("cvs/x")).unwrap();
    std::fs::rename(
        d.path().join("cvs/my-cv.cv.json"),
        d.path().join("cvs/x/my-cv.cv.json"),
    )
    .unwrap();
    lib.sweep_external_locked().unwrap();
    assert_eq!(lib.history("my-cv", 10).unwrap().len(), before);
    assert_eq!(lib.read_cv("my-cv").unwrap().cv["basics"]["name"], "Moved");
}

#[test]
fn deeper_folders_ignored() {
    let d = TempDir::new().unwrap();
    let lib = open(&d);
    put(&d, "cvs/a/b/deep.cv.json", "{}");
    assert!(lib.list_cvs().unwrap().iter().all(|c| c.name != "deep"));
}
