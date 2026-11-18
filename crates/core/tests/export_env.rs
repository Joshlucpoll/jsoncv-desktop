use jsoncv_core::export::{find_browser, html_to_pdf};
use jsoncv_core::render::PageSize;
use jsoncv_core::CoreError;

// Own file (own process) so the env var cannot race other tests.
#[tokio::test]
async fn missing_browser_errors() {
    std::env::set_var("JSONCV_BROWSER", "/nonexistent");
    assert!(find_browser().is_none());
    let d = tempfile::tempdir().unwrap();
    match html_to_pdf("<p>x</p>", &d.path().join("a.pdf"), PageSize::A4).await {
        Err(CoreError::Browser(m)) => assert!(m.contains("No Chrome"), "{m}"),
        o => panic!("{o:?}"),
    }
}
