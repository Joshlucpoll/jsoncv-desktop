use jsoncv_core::export::{find_browser, html_to_png, ExportFormat};
use jsoncv_core::library::Library;
use jsoncv_core::render::PageSize;
use jsoncv_core::Author;
use serde_json::json;

fn lib_with_photo() -> (tempfile::TempDir, Library) {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(d.path()).unwrap();
    std::fs::create_dir_all(d.path().join("assets")).unwrap();
    std::fs::write(d.path().join("assets/me.png"), b"\x89PNG\r\n\x1a\nfake").unwrap();
    let cv = json!({"basics":{"name":"Ada Lovelace","image":"assets/me.png"}});
    let base = lib.read_cv("main").unwrap().base;
    lib.replace_cv("main", cv, &base, Author::You, None).unwrap();
    (d, lib)
}

#[tokio::test]
async fn html_export_is_self_contained() {
    let (d, lib) = lib_with_photo();
    let p = lib.export("main", ExportFormat::Html, Some("builtin:classic")).await.unwrap();
    assert_eq!(p, d.path().join("exports/main-classic.html"));
    let html = std::fs::read_to_string(p).unwrap();
    // minijinja autoescape writes the "/" in the data URI as &#x2f;, which browsers decode.
    assert!(html.replace("&#x2f;", "/").contains("data:image/png;base64,") && !html.contains("assets/"), "{html}");
}

#[tokio::test]
async fn export_uses_effective_theme_and_flattens_name() {
    let (d, lib) = lib_with_photo();
    lib.create_tailored("acme", "main", Author::You).unwrap();
    let p = lib.export("tailored/acme", ExportFormat::Html, None).await.unwrap();
    assert_eq!(p, d.path().join("exports/tailored-acme-classic.html"));
    let g = lib.export("tailored/acme", ExportFormat::Json, None).await.unwrap();
    assert_eq!(g, d.path().join("exports/tailored-acme.json"));
}

#[tokio::test]
async fn json_export_matches_file() {
    let (d, lib) = lib_with_photo();
    let p = lib.export("main", ExportFormat::Json, None).await.unwrap();
    assert_eq!(std::fs::read(p).unwrap(), std::fs::read(d.path().join("main.cv.json")).unwrap());
}

#[tokio::test]
async fn pdf_export_writes_pdf() {
    if find_browser().is_none() {
        eprintln!("skip: no browser");
        return;
    }
    let (_d, lib) = lib_with_photo();
    let p = lib.export("main", ExportFormat::Pdf, None).await.unwrap();
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
    assert!(matches!(lib.export("../x", ExportFormat::Json, None).await, Err(jsoncv_core::CoreError::BadName(_))));
}

#[tokio::test]
async fn export_json_missing_cv_not_found() {
    let (_d, lib) = lib_with_photo();
    assert!(matches!(lib.export("tailored/nope", ExportFormat::Json, None).await, Err(jsoncv_core::CoreError::NotFound(_))));
}

#[tokio::test]
async fn png_preview_is_png() {
    if find_browser().is_none() {
        eprintln!("skip: no browser");
        return;
    }
    let png = html_to_png("<html><body>hi</body></html>", PageSize::A4).await.unwrap();
    assert_eq!(dims(&png), (1588, 2246));
    let png = html_to_png("<html><body>hi</body></html>", PageSize::Letter).await.unwrap();
    assert_eq!(dims(&png), (1632, 2112));
}
