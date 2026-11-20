use jsoncv_core::export::{find_browser, html_to_pdf, html_to_png, ExportFormat};
use jsoncv_core::library::Library;
use jsoncv_core::render::PageSize;
use jsoncv_core::Author;
use serde_json::json;

fn lib_with_photo() -> (tempfile::TempDir, Library) {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    std::fs::create_dir_all(d.path().join("assets")).unwrap();
    std::fs::write(d.path().join("assets/me.png"), b"\x89PNG\r\n\x1a\nfake").unwrap();
    let cv = json!({"basics":{"name":"Ada Lovelace","image":"assets/me.png"}});
    let base = lib.read_cv("my-cv").unwrap().base;
    lib.replace_cv("my-cv", cv, &base, Author::You, None)
        .unwrap();
    (d, lib)
}

#[tokio::test]
async fn html_export_is_self_contained() {
    let (d, lib) = lib_with_photo();
    let p = lib
        .export("my-cv", ExportFormat::Html, Some("builtin:classic"))
        .await
        .unwrap();
    assert_eq!(p, d.path().join("exports/my-cv-classic.html"));
    let html = std::fs::read_to_string(p).unwrap();
    // minijinja autoescape writes the "/" in the data URI as &#x2f;, which browsers decode.
    assert!(
        html.replace("&#x2f;", "/")
            .contains("data:image/png;base64,")
            && !html.contains("assets/"),
        "{html}"
    );
}

#[tokio::test]
async fn export_uses_effective_theme_and_flattens_name() {
    let (d, lib) = lib_with_photo();
    lib.duplicate_cv("acme", "my-cv", Author::You).unwrap();
    let p = lib.export("acme", ExportFormat::Html, None).await.unwrap();
    assert_eq!(p, d.path().join("exports/acme-classic.html"));
    let g = lib.export("acme", ExportFormat::Json, None).await.unwrap();
    assert_eq!(g, d.path().join("exports/acme.json"));
}

#[tokio::test]
async fn json_export_matches_file() {
    let (d, lib) = lib_with_photo();
    let p = lib.export("my-cv", ExportFormat::Json, None).await.unwrap();
    assert_eq!(
        std::fs::read(p).unwrap(),
        std::fs::read(d.path().join("cvs/my-cv.cv.json")).unwrap()
    );
}

#[tokio::test]
async fn export_to_writes_chosen_path() {
    let (d, lib) = lib_with_photo();
    let dest = d.path().join("elsewhere.html");
    let p = lib
        .export_to("my-cv", ExportFormat::Html, None, Some(&dest))
        .await
        .unwrap();
    assert_eq!(p, dest);
    assert!(dest.is_file());
    assert!(!d.path().join("exports/my-cv-classic.html").exists());
}

#[tokio::test]
async fn pdf_export_writes_pdf() {
    if find_browser().is_none() {
        eprintln!("skip: no browser");
        return;
    }
    let (_d, lib) = lib_with_photo();
    let p = lib.export("my-cv", ExportFormat::Pdf, None).await.unwrap();
    assert!(std::fs::read(&p).unwrap().starts_with(b"%PDF"));
    if let Ok(keep) = std::env::var("JSONCV_KEEP_PDF") {
        std::fs::copy(&p, keep).unwrap();
    }
}

fn dims(png: &[u8]) -> (u32, u32) {
    assert!(png.starts_with(b"\x89PNG"));
    let n = |i: usize| u32::from_be_bytes(png[i..i + 4].try_into().unwrap());
    (n(16), n(20))
}

#[tokio::test]
async fn export_json_rejects_bad_name() {
    let (_d, lib) = lib_with_photo();
    assert!(matches!(
        lib.export("../x", ExportFormat::Json, None).await,
        Err(jsoncv_core::CoreError::BadName(_))
    ));
}

#[tokio::test]
async fn export_json_missing_cv_not_found() {
    let (_d, lib) = lib_with_photo();
    assert!(matches!(
        lib.export("nope", ExportFormat::Json, None).await,
        Err(jsoncv_core::CoreError::NotFound(_))
    ));
}

#[tokio::test]
async fn png_preview_is_png() {
    if find_browser().is_none() {
        eprintln!("skip: no browser");
        return;
    }
    let png = html_to_png("<html><body>hi</body></html>", PageSize::A4)
        .await
        .unwrap();
    assert_eq!(dims(&png), (1588, 2246));
    let png = html_to_png("<html><body>hi</body></html>", PageSize::Letter)
        .await
        .unwrap();
    assert_eq!(dims(&png), (1632, 2112));
}

#[tokio::test]
async fn theme_scripts_do_not_run() {
    if find_browser().is_none() {
        eprintln!("skip: no browser");
        return;
    }
    // With JavaScript on, the busy loop wedges the page and the export times out.
    let html = "<html><body>hi<script>document.body.innerHTML='HACKED';while(true){}</script></body></html>";
    let png = html_to_png(html, PageSize::A4).await.unwrap();
    assert_eq!(dims(&png), (1588, 2246));
}

#[tokio::test]
async fn pdf_paper_falls_back_to_theme_size() {
    if find_browser().is_none() {
        eprintln!("skip: no browser");
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("x.pdf");
    let media = |b: Vec<u8>| {
        let s = String::from_utf8_lossy(&b).into_owned();
        let i = s.find("/MediaBox").expect("MediaBox");
        s[i..i + 40].to_string()
    };
    html_to_pdf("<html><body>hi</body></html>", &out, PageSize::A4, false)
        .await
        .unwrap();
    assert!(
        media(std::fs::read(&out).unwrap()).contains("595."),
        "{}",
        media(std::fs::read(&out).unwrap())
    );
    html_to_pdf(
        "<html><body>hi</body></html>",
        &out,
        PageSize::Letter,
        false,
    )
    .await
    .unwrap();
    assert!(
        media(std::fs::read(&out).unwrap()).contains("612"),
        "{}",
        media(std::fs::read(&out).unwrap())
    );
    let html = "<html><body>hi<script>while(true){}</script></body></html>";
    html_to_pdf(html, &out, PageSize::A4, false).await.unwrap();
    assert!(std::fs::read(&out).unwrap().starts_with(b"%PDF"));
}

#[tokio::test]
async fn continuous_pdf_is_one_tall_page() {
    if find_browser().is_none() {
        eprintln!("skip: no browser");
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("c.pdf");
    // 3000 px of content: three A4 pages when paged.
    let html = r#"<html><head><style>@page { size: A4; margin: 14mm; }</style></head><body style="margin:0"><div style="height:3000px">tall</div></body></html>"#;
    html_to_pdf(html, &out, PageSize::A4, true).await.unwrap();
    let s = String::from_utf8_lossy(&std::fs::read(&out).unwrap()).into_owned();
    let pages = s.matches("/Type /Page").count() - s.matches("/Type /Pages").count();
    assert_eq!(pages, 1, "one page");
    let i = s.find("/MediaBox").expect("MediaBox");
    // 3002 px = 2251.5 pt tall.
    assert!(s[i..i + 60].contains("2251"), "{}", &s[i..i + 60]);
}

#[tokio::test]
async fn continuous_cv_exports_as_one_page() {
    if find_browser().is_none() {
        eprintln!("skip: no browser");
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(d.path(), &d.path().join(".history.db")).unwrap();
    lib.set_cv_meta(
        "my-cv",
        "pageMode",
        Some("continuous".into()),
        Author::You,
        "Continuous page",
    )
    .unwrap();
    assert!(lib.list_cvs().unwrap()[0].continuous);
    for theme in ["builtin:classic", "builtin:two-column"] {
        let p = lib
            .export("my-cv", ExportFormat::Pdf, Some(theme))
            .await
            .unwrap();
        let s = String::from_utf8_lossy(&std::fs::read(p).unwrap()).into_owned();
        let pages = s.matches("/Type /Page").count() - s.matches("/Type /Pages").count();
        assert_eq!(pages, 1, "{theme}");
    }
    lib.set_cv_meta("my-cv", "pageMode", None, Author::You, "Paged")
        .unwrap();
    assert!(!lib.list_cvs().unwrap()[0].continuous);
}
