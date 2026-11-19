use jsoncv_core::{library::Library, Author, CoreError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn patch(v: Value) -> json_patch::Patch {
    serde_json::from_value(v).unwrap()
}

/// A library whose `my-cv` is a fixed fixture: these tests rely on its shape (two jobs, no awards).
fn setup() -> (TempDir, Library) {
    let d = TempDir::new().unwrap();
    std::fs::create_dir(d.path().join("cvs")).unwrap();
    std::fs::write(
        d.path().join("cvs/my-cv.cv.json"),
        include_str!("fixtures/mara.cv.json"),
    )
    .unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    (d, lib)
}

fn head_author(lib: &Library) -> String {
    lib.history("my-cv", 1).unwrap()[0].author.clone()
}

#[test]
fn patch_writes_and_commits() {
    let (_d, lib) = setup();
    let p = patch(json!([{"op":"replace","path":"/basics/name","value":"Grace"}]));
    let s = lib.patch_cv("my-cv", &p, Author::Agent, None).unwrap();
    let doc = lib.read_cv("my-cv").unwrap();
    assert_eq!(doc.cv["basics"]["name"], "Grace");
    assert_eq!(doc.base, s.base);
    assert_eq!(head_author(&lib), "Agent");
    assert_eq!(lib.history("my-cv", 1).unwrap()[0].message, "Edited basics");
}

#[test]
fn invalid_patch_result_not_written() {
    let (d, lib) = setup();
    let before = std::fs::read(d.path().join("cvs/my-cv.cv.json")).unwrap();
    let p = patch(json!([{"op":"replace","path":"/work/0/startDate","value":"March"}]));
    assert!(matches!(
        lib.patch_cv("my-cv", &p, Author::Agent, None),
        Err(CoreError::Invalid(_))
    ));
    assert_eq!(
        std::fs::read(d.path().join("cvs/my-cv.cv.json")).unwrap(),
        before
    );
}

#[test]
fn replace_rejects_stale_base() {
    let (_d, lib) = setup();
    let doc = lib.read_cv("my-cv").unwrap();
    let p = patch(json!([{"op":"replace","path":"/basics/name","value":"Grace"}]));
    lib.patch_cv("my-cv", &p, Author::Agent, None).unwrap();
    let r = lib.replace_cv("my-cv", doc.cv, &doc.base, Author::You, None);
    assert!(matches!(r, Err(CoreError::Stale)));
}

#[test]
fn draft_edits_on_different_fields_both_survive() {
    let (_d, lib) = setup();
    let base = lib.read_cv("my-cv").unwrap().cv;
    let p = patch(json!([{"op":"replace","path":"/basics/label","value":"Boss"}]));
    lib.patch_cv("my-cv", &p, Author::Agent, None).unwrap();
    let mut draft = base.clone();
    draft["basics"]["name"] = json!("Grace");
    lib.save_draft("my-cv", &base, &draft).unwrap();
    let cv = lib.read_cv("my-cv").unwrap().cv;
    assert_eq!(cv["basics"]["name"], "Grace");
    assert_eq!(cv["basics"]["label"], "Boss");
}

#[test]
fn draft_detects_shifted_array() {
    let (_d, lib) = setup();
    let base = lib.read_cv("my-cv").unwrap().cv;
    let mut item = base["work"][0].clone();
    item["name"] = json!("Inserted");
    let p = patch(json!([{"op":"add","path":"/work/0","value":item}]));
    lib.patch_cv("my-cv", &p, Author::Agent, None).unwrap();
    let mut draft = base.clone();
    draft["work"][1]["position"] = json!("Changed");
    assert!(matches!(
        lib.save_draft("my-cv", &base, &draft),
        Err(CoreError::PatchFailed(_))
    ));
    assert_eq!(
        lib.read_cv("my-cv").unwrap().cv["work"][0]["name"],
        "Inserted"
    );
    assert_eq!(
        lib.read_cv("my-cv").unwrap().cv["work"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn tailored_copy_records_origin() {
    let (d, lib) = setup();
    let info = lib.create_tailored("stripe", "my-cv", Author::You).unwrap();
    assert_eq!(info.name, "stripe");
    let doc = lib.read_cv("stripe").unwrap();
    assert_eq!(doc.cv["meta"]["tailoredFrom"]["cv"], "my-cv");
    assert!(d.path().join("cvs/stripe.cv.json").exists());
    assert!(matches!(
        lib.create_tailored("stripe", "my-cv", Author::You),
        Err(CoreError::BadName(_))
    ));
    assert!(lib
        .create_tailored("Bad Slug", "my-cv", Author::You)
        .is_err());
}

#[test]
fn rename_and_delete_tailored() {
    let (d, lib) = setup();
    lib.create_tailored("a", "my-cv", Author::You).unwrap();
    let info = lib.rename_cv("a", "b", Author::You).unwrap();
    assert_eq!(info.name, "b");
    assert!(!d.path().join("cvs/a.cv.json").exists());
    assert_eq!(lib.history("b", 5).unwrap()[0].message, "Renamed a to b");
    lib.delete_cv("b", Author::You).unwrap();
    assert!(!d.path().join("cvs/b.cv.json").exists());
    let h = lib.history("b", 5).unwrap();
    assert_eq!(h[0].message, "Deleted b");
    // The rename carried the creation row along, and the old name has no history left.
    assert_eq!(h.last().unwrap().message, "Created a from my-cv");
    assert!(lib.history("a", 5).unwrap().is_empty());
    assert!(matches!(
        lib.read_cv_at("b", &h[0].commit),
        Err(CoreError::NotFound(_))
    ));
    // Recreating the slug keeps the old versions under it.
    lib.create_tailored("b", "my-cv", Author::You).unwrap();
    assert_eq!(lib.history("b", 10).unwrap().len(), h.len() + 1);
}

#[test]
fn set_theme() {
    let (_d, lib) = setup();
    let t = _d.path().join("themes/t1");
    std::fs::create_dir_all(&t).unwrap();
    std::fs::write(t.join("template.html.j2"), "x").unwrap();
    std::fs::write(t.join("style.css"), "").unwrap();
    std::fs::write(
        t.join("theme.json"),
        r#"{"name":"T","description":"d","page":{"size":"A4"}}"#,
    )
    .unwrap();
    lib.set_cv_theme("my-cv", "t1", Author::You).unwrap();
    assert_eq!(lib.read_cv("my-cv").unwrap().cv["meta"]["theme"], "t1");
}

#[test]
fn write_to_broken_file_refused() {
    let (d, lib) = setup();
    std::fs::write(d.path().join("cvs/my-cv.cv.json"), "{").unwrap();
    let p = patch(json!([{"op":"replace","path":"/basics/name","value":"G"}]));
    assert!(matches!(
        lib.patch_cv("my-cv", &p, Author::Agent, None),
        Err(CoreError::Json(_))
    ));
    let cv = json!({"basics":{"name":"x"}});
    assert!(matches!(
        lib.save_draft("my-cv", &cv, &json!({"basics":{"name":"y"}})),
        Err(CoreError::Json(_))
    ));
    assert!(matches!(
        lib.set_cv_theme("my-cv", "builtin:classic", Author::You),
        Err(CoreError::Json(_))
    ));
    assert_eq!(
        std::fs::read_to_string(d.path().join("cvs/my-cv.cv.json")).unwrap(),
        "{"
    );
}

#[test]
fn restore_adds_new_commit() {
    let (_d, lib) = setup();
    let old = lib.history("my-cv", 1).unwrap()[0].commit.clone();
    let orig = lib.read_cv("my-cv").unwrap().cv;
    let p = patch(json!([{"op":"replace","path":"/basics/name","value":"Grace"}]));
    lib.patch_cv("my-cv", &p, Author::Agent, None).unwrap();
    let n = lib.history("my-cv", 50).unwrap().len();
    lib.restore("my-cv", &old, Author::You).unwrap();
    assert_eq!(lib.read_cv("my-cv").unwrap().cv, orig);
    let h = lib.history("my-cv", 50).unwrap();
    assert_eq!(h.len(), n + 1);
    assert_eq!(h[0].message, format!("Restored version v{old}"));
}

#[test]
fn draft_tail_removal_detects_concurrent_edit() {
    let (_d, lib) = setup();
    let base = lib.read_cv("my-cv").unwrap().cv;
    let p = patch(json!([{"op":"replace","path":"/work/1/position","value":"AgentEdit"}]));
    lib.patch_cv("my-cv", &p, Author::Agent, None).unwrap();
    let mut draft = base.clone();
    draft["work"] = json!([]);
    assert!(matches!(
        lib.save_draft("my-cv", &base, &draft),
        Err(CoreError::PatchFailed(_))
    ));
    assert_eq!(
        lib.read_cv("my-cv").unwrap().cv["work"][1]["position"],
        "AgentEdit"
    );
}

#[test]
fn draft_add_after_array_shift_detects_conflict() {
    let (d, lib) = setup();
    let base = lib.read_cv("my-cv").unwrap().cv;
    assert!(base["work"][1].get("url").is_none());
    let mut draft = base.clone();
    draft["work"][1]["url"] = json!("https://user.example");
    let p = patch(
        json!([{"op":"add","path":"/work/0","value":{"name":"NewCo","position":"CTO","startDate":"2025-01"}}]),
    );
    lib.patch_cv("my-cv", &p, Author::Agent, None).unwrap();
    let before = std::fs::read(d.path().join("cvs/my-cv.cv.json")).unwrap();
    assert!(matches!(
        lib.save_draft("my-cv", &base, &draft),
        Err(CoreError::PatchFailed(_))
    ));
    assert_eq!(
        std::fs::read(d.path().join("cvs/my-cv.cv.json")).unwrap(),
        before
    );
}

#[test]
fn draft_root_add_conflict_detected() {
    let (d, lib) = setup();
    let base = lib.read_cv("my-cv").unwrap().cv;
    assert!(base.get("awards").is_none());
    let mut draft = base.clone();
    draft["awards"] = json!([{"title":"Mine"}]);
    let p = patch(json!([{"op":"add","path":"/awards","value":[{"title":"Agent"}]}]));
    lib.patch_cv("my-cv", &p, Author::Agent, None).unwrap();
    let before = std::fs::read(d.path().join("cvs/my-cv.cv.json")).unwrap();
    assert!(matches!(
        lib.save_draft("my-cv", &base, &draft),
        Err(CoreError::PatchFailed(_))
    ));
    assert_eq!(
        std::fs::read(d.path().join("cvs/my-cv.cv.json")).unwrap(),
        before
    );
}

#[test]
fn draft_root_add_without_conflict_saves() {
    let (_d, lib) = setup();
    let base = lib.read_cv("my-cv").unwrap().cv;
    let mut draft = base.clone();
    draft["awards"] = json!([{"title":"Mine"}]);
    draft["work"][1]["url"] = json!("https://user.example");
    lib.save_draft("my-cv", &base, &draft).unwrap();
    let cv = lib.read_cv("my-cv").unwrap().cv;
    assert_eq!(cv["awards"][0]["title"], "Mine");
    assert_eq!(cv["work"][1]["url"], "https://user.example");
}
