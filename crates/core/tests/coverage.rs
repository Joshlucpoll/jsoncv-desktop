use jsoncv_core::coverage::missing_fields;
use jsoncv_core::library::Library;
use jsoncv_core::render::{PageMeta, PageSize, ThemeMeta};
use jsoncv_core::{Author, CoreError};

fn lib() -> (tempfile::TempDir, Library) {
    let d = tempfile::tempdir().unwrap();
    let l = Library::open(d.path()).unwrap();
    (d, l)
}

fn meta(name: &str) -> ThemeMeta {
    ThemeMeta { name: name.into(), description: "d".into(), page: PageMeta { size: PageSize::A4 } }
}

#[test]
fn template_skipping_awards_reports_section() {
    let m = missing_fields("{{ cv.basics.name }}").unwrap();
    assert!(m.contains(&"awards (whole section)".to_string()), "{m:?}");
    assert!(m.contains(&"basics.label".to_string()));
    assert!(!m.contains(&"basics.name".to_string()));
    assert!(!m.iter().any(|f| f.starts_with("awards[]")));
}

#[test]
fn loop_fields_tracked() {
    let m = missing_fields("{% for w in cv.work %}{{ w.position }}{% endfor %}").unwrap();
    assert!(!m.contains(&"work[].position".to_string()), "{m:?}");
    assert!(m.contains(&"work[].highlights".to_string()));
}

#[test]
fn filters_still_count_as_read() {
    let m = missing_fields(r#"{{ cv.work[0].startDate | format_date("%Y") }}"#).unwrap();
    assert!(!m.contains(&"work[].startDate".to_string()), "{m:?}");
}

#[test]
fn constructs_work_on_tracked_values() {
    let t = "{% if cv.basics.location is defined %}a{% endif %}{% if cv.work | length %}b{% endif %}\
             {% for h in cv.work[0].highlights %}{{ h | markdown }}{% endfor %}{{ cv.skills[0].keywords | join(', ') }}\
             {% for p in cv.basics.profiles %}{{ p.url | strip_scheme }}{% endfor %}{{ cv.basics.summary | markdown }}";
    let m = missing_fields(t).unwrap();
    for read in ["work[].highlights", "skills[].keywords", "basics.profiles[].url", "basics.summary"] {
        assert!(!m.contains(&read.to_string()), "{read} in {m:?}");
    }
}

#[test]
fn broken_template_is_render_error() {
    assert!(matches!(missing_fields("{{ cv.basics.name "), Err(CoreError::Render(_))));
}

#[test]
fn builtin_reports_are_small() {
    let (_d, lib) = lib();
    for id in ["builtin:classic", "builtin:two-column"] {
        let r = lib.theme_report(id).unwrap();
        assert!(r.render_error.is_none(), "{id}: {:?}", r.render_error);
        println!("{id} missing: {:?}", r.missing);
    }
}

#[test]
fn save_builtin_refused() {
    let (_d, lib) = lib();
    let r = lib.save_theme("builtin:classic", "x", "y", &meta("n"), Author::You);
    assert!(matches!(r, Err(CoreError::ReadOnly(_))));
}

#[test]
fn save_theme_bad_id() {
    let (_d, lib) = lib();
    assert!(matches!(lib.save_theme("Bad Id", "x", "y", &meta("n"), Author::You), Err(CoreError::BadName(_))));
}

#[test]
fn save_theme_with_broken_template_still_saves() {
    let (d, lib) = lib();
    let r = lib.save_theme("mine", "{{ cv.basics.name ", "body{}", &meta("Mine"), Author::Agent).unwrap();
    assert!(r.render_error.is_some() && r.missing.is_empty());
    for f in ["template.html.j2", "style.css", "theme.json"] {
        assert!(d.path().join("themes/mine").join(f).exists(), "{f}");
    }
    assert!(std::fs::read_to_string(d.path().join("themes/mine/theme.json")).unwrap().ends_with("}\n"));
    let msg = git2::Repository::open(d.path()).unwrap().head().unwrap().peel_to_commit().unwrap().message().unwrap().to_string();
    assert_eq!(msg, "Saved theme mine");
    assert!(lib.theme_report("mine").unwrap().render_error.is_some());
}

#[test]
fn save_theme_good_template_reports_missing() {
    let (_d, lib) = lib();
    let r = lib.save_theme("ok", "{{ cv.basics.name }}", "", &meta("Ok"), Author::You).unwrap();
    assert!(r.render_error.is_none() && r.missing.contains(&"awards (whole section)".to_string()));
    assert_eq!(lib.get_theme("ok").unwrap().template, "{{ cv.basics.name }}");
}

#[test]
fn duplicate_theme_copies_builtin_and_assets() {
    let (d, lib) = lib();
    let info = lib.duplicate_theme("builtin:classic", "my-classic", Author::You).unwrap();
    assert_eq!(info.meta.name, "Classic copy");
    assert!(!info.builtin);
    assert!(matches!(lib.duplicate_theme("builtin:classic", "my-classic", Author::You), Err(CoreError::BadName(m)) if m == "Theme my-classic already exists"));
    std::fs::create_dir_all(d.path().join("themes/my-classic/assets")).unwrap();
    std::fs::write(d.path().join("themes/my-classic/assets/f.txt"), "hi").unwrap();
    lib.duplicate_theme("my-classic", "again", Author::You).unwrap();
    assert_eq!(std::fs::read_to_string(d.path().join("themes/again/assets/f.txt")).unwrap(), "hi");
    let repo = git2::Repository::open(d.path()).unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    assert_eq!(head.message().unwrap(), "Duplicated theme my-classic as again");
    assert!(head.tree().unwrap().get_path(std::path::Path::new("themes/again/assets/f.txt")).is_ok());
}
