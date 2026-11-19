use jsoncv_core::library::Library;
use jsoncv_core::{SAMPLE_CV, THEME_GUIDE};
use serde_json::json;

const THEMES: [&str; 2] = ["builtin:classic", "builtin:two-column"];

fn lib() -> (tempfile::TempDir, Library) {
    let d = tempfile::tempdir().unwrap();
    let l = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    (d, l)
}

#[test]
fn builtin_themes_render_sample() {
    let (_d, lib) = lib();
    let cv: serde_json::Value = serde_json::from_str(SAMPLE_CV).unwrap();
    for t in THEMES {
        let out = lib.render(&cv, t).unwrap_or_else(|e| panic!("{t}: {e}"));
        for needle in [
            "Mara Lindqvist",
            "Senior Firmware Engineer",
            "Software Engineer",
            "Work",
            "Education",
            "Skills",
            "Projects",
            "Mar 2021",
            "Present",
        ] {
            assert!(out.contains(needle), "{t} missing {needle}");
        }
        assert!(
            !out.contains("src=\"http") && !out.contains("@import") && !out.contains("url(http"),
            "{t}"
        );
    }
}

#[test]
fn builtin_themes_render_minimal_cv() {
    let (_d, lib) = lib();
    for cv in [
        json!({"basics":{"name":"A"}}),
        json!({"basics":{"name":"A","profiles":[]},"work":[],"skills":[],"education":[]}),
    ] {
        for t in THEMES {
            let out = lib.render(&cv, t).unwrap();
            assert!(out.contains(">A<"), "{t}");
            for h in [
                "Work",
                "Education",
                "Skills",
                "Projects",
                "Languages",
                "Interests",
                "Profiles",
            ] {
                assert!(!out.contains(&format!(">{h}<")), "{t} shows empty {h}");
            }
        }
    }
}

#[test]
fn builtin_themes_render_present_job() {
    let (_d, lib) = lib();
    let cv = json!({"basics":{"name":"A"},"work":[{"name":"Co","position":"Dev","startDate":"2020-01"}]});
    for t in THEMES {
        assert!(
            lib.render(&cv, t).unwrap().contains("Jan 2020 – Present"),
            "{t}"
        );
    }
}

#[test]
fn builtin_themes_render_all_sections() {
    let (_d, lib) = lib();
    let cv = json!({"basics":{"name":"A","profiles":[{"network":"GH","url":"https://gh.example/a"}]},
        "work":[{"name":"W"}],"volunteer":[{"organization":"V"}],"education":[{"institution":"E"}],
        "awards":[{"title":"Aw"}],"certificates":[{"name":"Ce"}],"publications":[{"name":"Pu"}],
        "skills":[{"name":"S"}],"languages":[{"language":"L"}],"interests":[{"name":"I"}],
        "references":[{"name":"R"}],"projects":[{"name":"P"}],"sideProjects":[{"name":"SP"}]});
    for t in THEMES {
        let out = lib.render(&cv, t).unwrap();
        for h in [
            "Work",
            "Education",
            "Skills",
            "Projects",
            "Volunteer",
            "Awards",
            "Certificates",
            "Publications",
            "Languages",
            "Interests",
            "References",
            "Side projects",
        ] {
            assert!(out.contains(&format!(">{h}<")), "{t} missing {h}");
        }
        // Classic shows profiles in the header, not as a section.
        assert!(t == "builtin:classic" || out.contains(">Profiles<"), "{t}");
    }
}

#[test]
fn guide_quotes_classic_template() {
    let (_d, lib) = lib();
    let t = lib.get_theme("builtin:classic").unwrap();
    assert!(THEME_GUIDE.contains(t.template.trim()));
    assert!(THEME_GUIDE.contains(t.css.trim()));
}

#[test]
fn builtins_are_listed_first() {
    let (d, lib) = lib();
    let t = d.path().join("themes/mine");
    std::fs::create_dir_all(&t).unwrap();
    std::fs::write(t.join("template.html.j2"), "x").unwrap();
    std::fs::write(t.join("style.css"), "").unwrap();
    std::fs::write(
        t.join("theme.json"),
        r#"{"name":"M","description":"d","page":{"size":"A4"}}"#,
    )
    .unwrap();
    let ids: Vec<String> = lib
        .list_themes()
        .unwrap()
        .into_iter()
        .map(|t| t.id)
        .collect();
    assert_eq!(ids, ["builtin:classic", "builtin:two-column", "mine"]);
}

#[test]
fn builtin_themes_escape_html_and_block_js_urls() {
    let (_d, lib) = lib();
    let cv = json!({"basics":{"name":"<script>x</script>","url":"javascript:alert(1)","profiles":[{"network":"n","url":"JavaScript:alert(1)"}]},
        "work":[{"name":"C","position":"<script>y</script>","url":"javascript:alert(1)"}],
        "certificates":[{"name":"c","url":"javascript:alert(1)"}]});
    for t in THEMES {
        let out = lib.render(&cv, t).unwrap().to_lowercase();
        assert!(
            !out.contains("<script>") && !out.contains("href=\"javascript:"),
            "{t}"
        );
        assert!(out.contains("&lt;script&gt;"), "{t}");
    }
}

#[test]
fn builtin_themes_render_extra_schema_fields() {
    let (_d, lib) = lib();
    let cv = json!({"basics":{"name":"A","location":{"address":"1 Main St","postalCode":"12345","city":"Town"}},
        "projects":[{"name":"P","type":"talk","entity":"ACME","roles":["Lead","Speaker"]}],
        "education":[{"institution":"E","url":"https://uni.example/"}],
        "languages":[{"language":"L","summary":"ZZLangSummary"}]});
    for t in THEMES {
        let out = lib.render(&cv, t).unwrap();
        for n in [
            "1 Main St",
            "12345",
            "talk",
            "ACME",
            "Lead, Speaker",
            "uni.example",
            "ZZLangSummary",
        ] {
            assert!(out.contains(n), "{t} missing {n}");
        }
    }
}
