use jsoncv_core::library::Library;
use jsoncv_core::{Author, CoreError};
use serde_json::json;

fn setup(template: &str) -> (tempfile::TempDir, Library) {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(d.path()).unwrap();
    let t = d.path().join("themes/t1");
    std::fs::create_dir_all(&t).unwrap();
    std::fs::write(t.join("template.html.j2"), template).unwrap();
    std::fs::write(t.join("style.css"), "body{color:red}").unwrap();
    std::fs::write(t.join("theme.json"), r#"{"name":"T","description":"d","page":{"size":"Letter"}}"#).unwrap();
    (d, lib)
}

#[test]
fn render_escapes_html_and_keeps_unicode() {
    let (_d, lib) = setup("<html><head></head><body>{{ cv.basics.name }}</body></html>");
    let cv = json!({"basics":{"name":"<script>x</script> Glücklichkeit 🚀"}});
    let out = lib.render(&cv, "t1").unwrap();
    assert!(out.contains("&lt;script&gt;") && out.contains("Glücklichkeit 🚀") && !out.contains("<script>x"), "{out}");
}

#[test]
fn render_inlines_css() {
    let (_d, lib) = setup("<html><head></head><body></body></html>");
    let out = lib.render(&json!({}), "t1").unwrap();
    assert!(out.contains("<style>body{color:red}</style></head>"), "{out}");
    let (_d, lib) = setup("plain");
    let out = lib.render(&json!({}), "t1").unwrap();
    assert!(out.starts_with("<!doctype html>") && out.contains("<style>body{color:red}</style>") && out.contains("<body>plain</body>"));
}

#[test]
fn render_error_has_line() {
    let (_d, lib) = setup("a\nb\n{{ cv.basics.name | nope }}\n");
    match lib.render(&json!({"basics":{"name":"x"}}), "t1") {
        Err(CoreError::Render(e)) => assert_eq!(e.line, Some(3), "{e}"),
        o => panic!("{o:?}"),
    }
}

#[test]
fn themes_list_get_and_ids() {
    let (d, lib) = setup("x");
    std::fs::create_dir_all(d.path().join("themes/empty")).unwrap();
    let l = lib.list_themes().unwrap();
    assert_eq!(l.len(), 3);
    assert!(l[..2].iter().all(|t| t.builtin));
    assert_eq!((l[2].id.as_str(), l[2].builtin), ("t1", false));
    assert!(matches!(lib.get_theme("../x"), Err(CoreError::BadName(_))));
    assert!(matches!(lib.get_theme("builtin:nope"), Err(CoreError::NotFound(_))));
    assert!(matches!(lib.get_theme("empty"), Err(CoreError::NotFound(_))));
}

#[test]
fn render_cv_uses_meta_theme_and_commit() {
    let (_d, lib) = setup("N={{ cv.basics.name }}");
    lib.set_cv_theme("main", "t1", Author::You).unwrap();
    assert!(lib.render_cv("main", None, None).unwrap().contains("N="));
    let head = lib.history("main", 1).unwrap()[0].commit.clone();
    let old_name = lib.read_cv("main").unwrap().cv["basics"]["name"].as_str().unwrap().to_string();
    let p: json_patch::Patch = serde_json::from_value(json!([{"op":"replace","path":"/basics/name","value":"ZZNewName"}])).unwrap();
    lib.patch_cv("main", &p, Author::You, None).unwrap();
    let old = lib.render_cv("main", Some("t1"), Some(&head)).unwrap();
    assert!(old.contains(&format!("N={old_name}")) && !old.contains("ZZNewName"), "{old}");
    assert!(lib.render_cv("main", None, None).unwrap().contains("N=ZZNewName"));
}

#[test]
fn render_cv_explicit_unknown_theme_errors() {
    let (_d, lib) = setup("x");
    assert!(matches!(lib.render_cv("main", Some("nope"), None), Err(CoreError::NotFound(_))));
}

#[test]
fn render_cv_bad_meta_theme_attempts_classic_fallback() {
    let (_d, lib) = setup("x");
    let p: json_patch::Patch = serde_json::from_value(json!([{"op":"add","path":"/meta","value":{"theme":"My Theme"}}])).unwrap();
    lib.patch_cv("main", &p, Author::You, None).unwrap();
    let out = lib.render_cv("main", None, None).unwrap();
    assert!(out.contains("<style>"), "{out}");
}

#[test]
fn render_cv_falls_back_to_classic_when_theme_missing() {
    let (_d, lib) = setup("x");
    let p: json_patch::Patch = serde_json::from_value(json!([{"op":"add","path":"/meta","value":{"theme":"gone"}}])).unwrap();
    lib.patch_cv("main", &p, Author::You, None).unwrap();
    let cv = lib.read_cv("main").unwrap().cv;
    assert_eq!(lib.render_cv("main", None, None).unwrap(), lib.render(&cv, "builtin:classic").unwrap());
}

#[test]
fn render_neutralises_style_end_and_uppercase_head() {
    let (d, lib) = setup("<HTML><HEAD></HEAD><body></body></HTML>");
    std::fs::write(d.path().join("themes/t1/style.css"), "a{}</STYLE><script>1</script>").unwrap();
    let out = lib.render(&json!({}), "t1").unwrap();
    assert!(!out.contains("</STYLE>") && out.contains(r"<\/style"), "{out}");
    assert!(out.matches("</style>").count() == 1 && out.find("</style>").unwrap() < out.find("</HEAD>").unwrap(), "{out}");
}

#[test]
fn set_unknown_theme_rejected() {
    let (_d, lib) = setup("x");
    assert!(matches!(lib.set_cv_theme("main", "nope", Author::You), Err(CoreError::NotFound(_))));
    assert!(matches!(lib.set_cv_theme("main", "builtin:nope", Author::You), Err(CoreError::NotFound(_))));
}
