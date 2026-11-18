use jsoncv_core::{library::Library, Author, CoreError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn patch(v: Value) -> json_patch::Patch {
    serde_json::from_value(v).unwrap()
}

fn setup() -> (TempDir, Library) {
    let d = TempDir::new().unwrap();
    let lib = Library::open(d.path()).unwrap();
    (d, lib)
}

fn head_author(d: &TempDir) -> String {
    let r = git2::Repository::open(d.path()).unwrap();
    let c = r.head().unwrap().peel_to_commit().unwrap();
    let n = c.author().name().unwrap().to_string();
    n
}

#[test]
fn patch_writes_and_commits() {
    let (d, lib) = setup();
    let p = patch(json!([{"op":"replace","path":"/basics/name","value":"Grace"}]));
    let s = lib.patch_cv("main", &p, Author::Agent, None).unwrap();
    let doc = lib.read_cv("main").unwrap();
    assert_eq!(doc.cv["basics"]["name"], "Grace");
    assert_eq!(doc.base, s.base);
    assert_eq!(head_author(&d), "Agent");
    assert_eq!(lib.history("main", 1).unwrap()[0].message, "Edited basics");
}

#[test]
fn invalid_patch_result_not_written() {
    let (d, lib) = setup();
    let before = std::fs::read(d.path().join("main.cv.json")).unwrap();
    let p = patch(json!([{"op":"replace","path":"/work/0/startDate","value":"March"}]));
    assert!(matches!(lib.patch_cv("main", &p, Author::Agent, None), Err(CoreError::Invalid(_))));
    assert_eq!(std::fs::read(d.path().join("main.cv.json")).unwrap(), before);
}

#[test]
fn replace_rejects_stale_base() {
    let (_d, lib) = setup();
    let doc = lib.read_cv("main").unwrap();
    let p = patch(json!([{"op":"replace","path":"/basics/name","value":"Grace"}]));
    lib.patch_cv("main", &p, Author::Agent, None).unwrap();
    let r = lib.replace_cv("main", doc.cv, &doc.base, Author::You, None);
    assert!(matches!(r, Err(CoreError::Stale)));
}

#[test]
fn draft_edits_on_different_fields_both_survive() {
    let (_d, lib) = setup();
    let base = lib.read_cv("main").unwrap().cv;
    let p = patch(json!([{"op":"replace","path":"/basics/label","value":"Boss"}]));
    lib.patch_cv("main", &p, Author::Agent, None).unwrap();
    let mut draft = base.clone();
    draft["basics"]["name"] = json!("Grace");
    lib.save_draft("main", &base, &draft).unwrap();
    let cv = lib.read_cv("main").unwrap().cv;
    assert_eq!(cv["basics"]["name"], "Grace");
    assert_eq!(cv["basics"]["label"], "Boss");
}

#[test]
fn draft_detects_shifted_array() {
    let (_d, lib) = setup();
    let base = lib.read_cv("main").unwrap().cv;
    let mut item = base["work"][0].clone();
    item["name"] = json!("Inserted");
    let p = patch(json!([{"op":"add","path":"/work/0","value":item}]));
    lib.patch_cv("main", &p, Author::Agent, None).unwrap();
    let mut draft = base.clone();
    draft["work"][1]["position"] = json!("Changed");
    assert!(matches!(lib.save_draft("main", &base, &draft), Err(CoreError::PatchFailed(_))));
    assert_eq!(lib.read_cv("main").unwrap().cv["work"][0]["name"], "Inserted");
    assert_eq!(lib.read_cv("main").unwrap().cv["work"].as_array().unwrap().len(), 3);
}

#[test]
fn tailored_copy_records_origin() {
    let (d, lib) = setup();
    let info = lib.create_tailored("stripe", "main", Author::You).unwrap();
    assert_eq!(info.name, "tailored/stripe");
    let doc = lib.read_cv("tailored/stripe").unwrap();
    assert_eq!(doc.cv["meta"]["tailoredFrom"]["cv"], "main");
    assert!(d.path().join("tailored/stripe.cv.json").exists());
    assert!(matches!(lib.create_tailored("stripe", "main", Author::You), Err(CoreError::BadName(_))));
    assert!(lib.create_tailored("Bad Slug", "main", Author::You).is_err());
}

#[test]
fn rename_and_delete_tailored() {
    let (d, lib) = setup();
    lib.create_tailored("a", "main", Author::You).unwrap();
    let info = lib.rename_cv("tailored/a", "b", Author::You).unwrap();
    assert_eq!(info.name, "tailored/b");
    assert!(!d.path().join("tailored/a.cv.json").exists());
    assert_eq!(lib.history("tailored/b", 5).unwrap()[0].message, "Renamed tailored/a to tailored/b");
    lib.delete_cv("tailored/b", Author::You).unwrap();
    assert!(!d.path().join("tailored/b.cv.json").exists());
    let r = git2::Repository::open(d.path()).unwrap();
    assert_eq!(r.head().unwrap().peel_to_commit().unwrap().message().unwrap(), "Deleted tailored/b");
    let st: Vec<_> = r.statuses(None).unwrap().iter().filter(|e| !e.status().is_ignored()).map(|e| e.path().unwrap().to_string()).collect();
    assert!(st.is_empty(), "{st:?}");
}

#[test]
fn cannot_delete_or_rename_main() {
    let (_d, lib) = setup();
    assert!(matches!(lib.delete_cv("main", Author::You), Err(CoreError::ReadOnly(_))));
    assert!(matches!(lib.rename_cv("main", "x", Author::You), Err(CoreError::ReadOnly(_))));
}

#[test]
fn set_theme() {
    let (_d, lib) = setup();
    let t = _d.path().join("themes/t1");
    std::fs::create_dir_all(&t).unwrap();
    std::fs::write(t.join("template.html.j2"), "x").unwrap();
    std::fs::write(t.join("style.css"), "").unwrap();
    std::fs::write(t.join("theme.json"), r#"{"name":"T","description":"d","page":{"size":"A4"}}"#).unwrap();
    lib.set_cv_theme("main", "t1", Author::You).unwrap();
    assert_eq!(lib.read_cv("main").unwrap().cv["meta"]["theme"], "t1");
}

#[test]
fn write_to_broken_file_refused() {
    let (d, lib) = setup();
    std::fs::write(d.path().join("main.cv.json"), "{").unwrap();
    let p = patch(json!([{"op":"replace","path":"/basics/name","value":"G"}]));
    assert!(matches!(lib.patch_cv("main", &p, Author::Agent, None), Err(CoreError::Json(_))));
    let cv = json!({"basics":{"name":"x"}});
    assert!(matches!(lib.save_draft("main", &cv, &json!({"basics":{"name":"y"}})), Err(CoreError::Json(_))));
    assert!(matches!(lib.set_cv_theme("main", "builtin:x", Author::You), Err(CoreError::Json(_))));
    assert_eq!(std::fs::read_to_string(d.path().join("main.cv.json")).unwrap(), "{");
}

#[test]
fn restore_adds_new_commit() {
    let (_d, lib) = setup();
    let old = lib.history("main", 1).unwrap()[0].commit.clone();
    let orig = lib.read_cv("main").unwrap().cv;
    let p = patch(json!([{"op":"replace","path":"/basics/name","value":"Grace"}]));
    lib.patch_cv("main", &p, Author::Agent, None).unwrap();
    let n = lib.history("main", 50).unwrap().len();
    lib.restore("main", &old, Author::You).unwrap();
    assert_eq!(lib.read_cv("main").unwrap().cv, orig);
    let h = lib.history("main", 50).unwrap();
    assert_eq!(h.len(), n + 1);
    assert_eq!(h[0].message, format!("Restored version {}", &old[..7]));
}

#[test]
fn draft_tail_removal_detects_concurrent_edit() {
    let (_d, lib) = setup();
    let base = lib.read_cv("main").unwrap().cv;
    let p = patch(json!([{"op":"replace","path":"/work/1/position","value":"AgentEdit"}]));
    lib.patch_cv("main", &p, Author::Agent, None).unwrap();
    let mut draft = base.clone();
    draft["work"] = json!([]);
    assert!(matches!(lib.save_draft("main", &base, &draft), Err(CoreError::PatchFailed(_))));
    assert_eq!(lib.read_cv("main").unwrap().cv["work"][1]["position"], "AgentEdit");
}

#[test]
fn gitignore_has_tmp_pattern() {
    let (d, _lib) = setup();
    let gi = std::fs::read_to_string(d.path().join(".gitignore")).unwrap();
    assert!(gi.lines().any(|l| l == ".*.tmp-*"));
}

#[test]
fn draft_add_after_array_shift_detects_conflict() {
    let (d, lib) = setup();
    let base = lib.read_cv("main").unwrap().cv;
    assert!(base["work"][1].get("url").is_none());
    let mut draft = base.clone();
    draft["work"][1]["url"] = json!("https://user.example");
    let p = patch(json!([{"op":"add","path":"/work/0","value":{"name":"NewCo","position":"CTO","startDate":"2025-01"}}]));
    lib.patch_cv("main", &p, Author::Agent, None).unwrap();
    let before = std::fs::read(d.path().join("main.cv.json")).unwrap();
    assert!(matches!(lib.save_draft("main", &base, &draft), Err(CoreError::PatchFailed(_))));
    assert_eq!(std::fs::read(d.path().join("main.cv.json")).unwrap(), before);
}

#[test]
fn draft_root_add_conflict_detected() {
    let (d, lib) = setup();
    let base = lib.read_cv("main").unwrap().cv;
    assert!(base.get("awards").is_none());
    let mut draft = base.clone();
    draft["awards"] = json!([{"title":"Mine"}]);
    let p = patch(json!([{"op":"add","path":"/awards","value":[{"title":"Agent"}]}]));
    lib.patch_cv("main", &p, Author::Agent, None).unwrap();
    let before = std::fs::read(d.path().join("main.cv.json")).unwrap();
    assert!(matches!(lib.save_draft("main", &base, &draft), Err(CoreError::PatchFailed(_))));
    assert_eq!(std::fs::read(d.path().join("main.cv.json")).unwrap(), before);
}

#[test]
fn draft_root_add_without_conflict_saves() {
    let (_d, lib) = setup();
    let base = lib.read_cv("main").unwrap().cv;
    let mut draft = base.clone();
    draft["awards"] = json!([{"title":"Mine"}]);
    draft["work"][1]["url"] = json!("https://user.example");
    lib.save_draft("main", &base, &draft).unwrap();
    let cv = lib.read_cv("main").unwrap().cv;
    assert_eq!(cv["awards"][0]["title"], "Mine");
    assert_eq!(cv["work"][1]["url"], "https://user.example");
}
