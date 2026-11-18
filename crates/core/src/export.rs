use std::path::{Path, PathBuf};
use std::time::Duration;

use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
use chromiumoxide::cdp::browser_protocol::page::{PrintToPdfParams, Viewport};
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::{Browser, BrowserConfig};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::library::Library;
use crate::render::PageSize;
use crate::CoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Pdf,
    Html,
    Json,
}

fn first_existing(paths: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    paths.into_iter().find(|p| p.is_file())
}

/// Finds Chrome, Edge or Chromium. `JSONCV_BROWSER` wins and is never second-guessed.
pub fn find_browser() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("JSONCV_BROWSER") {
        return first_existing([PathBuf::from(p)]);
    }
    if cfg!(target_os = "macos") {
        let app = |n: &str, bin: &str| PathBuf::from(format!("/Applications/{n}.app/Contents/MacOS/{bin}"));
        first_existing([
            app("Google Chrome", "Google Chrome"),
            app("Microsoft Edge", "Microsoft Edge"),
            app("Chromium", "Chromium"),
            app("Brave Browser", "Brave Browser"),
        ])
    } else if cfg!(windows) {
        let rels = [r"Google\Chrome\Application\chrome.exe", r"Microsoft\Edge\Application\msedge.exe"];
        first_existing(["ProgramFiles", "ProgramFiles(x86)", "LocalAppData"].iter().filter_map(|v| std::env::var_os(v)).flat_map(|base| {
            rels.iter().map(move |r| PathBuf::from(&base).join(r))
        }))
    } else {
        ["google-chrome", "chromium", "chromium-browser", "microsoft-edge"].iter().find_map(|b| {
            let out = std::process::Command::new("which").arg(b).output().ok()?;
            let p = PathBuf::from(String::from_utf8(out.stdout).ok()?.trim());
            (out.status.success() && p.is_file()).then_some(p)
        })
    }
}

fn browser_err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Browser(e.to_string())
}

/// Launches a throwaway headless browser, runs `f` on a page holding `html`, then always cleans up.
async fn with_page<T, F, Fut>(html: &str, f: F) -> Result<T, CoreError>
where
    F: FnOnce(chromiumoxide::Page) -> Fut,
    Fut: std::future::Future<Output = Result<T, CoreError>>,
{
    let exe = find_browser().ok_or_else(|| CoreError::Browser("No Chrome, Edge or Chromium found".into()))?;
    // Fresh profile so a running Chrome's profile is never touched.
    let profile = tempfile::tempdir()?;
    let config = BrowserConfig::builder()
        .chrome_executable(exe)
        .user_data_dir(profile.path())
        .new_headless_mode()
        .args(["--disable-gpu", "--no-first-run", "--no-default-browser-check"])
        .build()
        .map_err(CoreError::Browser)?;
    let (mut browser, mut handler) = Browser::launch(config).await.map_err(browser_err)?;
    let task = tokio::spawn(async move { while handler.next().await.is_some() {} });

    let run = async {
        let page = browser.new_page("about:blank").await.map_err(browser_err)?;
        page.set_content(html).await.map_err(browser_err)?;
        f(page).await
    };
    let res = match tokio::time::timeout(Duration::from_secs(30), run).await {
        Ok(r) => r,
        Err(_) => Err(CoreError::Browser("Timed out waiting for the browser".into())),
    };

    // A wedged Chrome never answers close, so bound it and then kill.
    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        let _ = browser.close().await;
        let _ = browser.wait().await;
    })
    .await;
    if closed.is_err() {
        let _ = browser.kill().await;
    }
    task.abort();
    let _ = task.await;
    res
}

pub async fn html_to_pdf(html: &str, out: &Path) -> Result<(), CoreError> {
    let params = PrintToPdfParams::builder().prefer_css_page_size(true).print_background(true).build();
    let bytes = with_page(html, |page| async move { page.pdf(params).await.map_err(browser_err) }).await?;
    tokio::fs::write(out, bytes).await?;
    Ok(())
}

pub async fn html_to_png(html: &str, size: PageSize) -> Result<Vec<u8>, CoreError> {
    let (w, h) = match size {
        PageSize::A4 => (794, 1123),
        PageSize::Letter => (816, 1056),
    };
    with_page(html, |page| async move {
        page.execute(SetDeviceMetricsOverrideParams::new(w, h, 2.0, false)).await.map_err(browser_err)?;
        // Clip to the first page; the default full-viewport capture would otherwise be the same, but this is explicit.
        let clip = Viewport { x: 0.0, y: 0.0, width: w as f64, height: h as f64, scale: 1.0 };
        page.screenshot(ScreenshotParams::builder().clip(clip).build()).await.map_err(browser_err)
    })
    .await
}

impl Library {
    pub async fn export(&self, name: &str, format: ExportFormat, theme: Option<&str>) -> Result<PathBuf, CoreError> {
        let cv_path = self.cv_path(name)?;
        let dir = self.root().join("exports");
        std::fs::create_dir_all(&dir)?;
        let flat = name.replace('/', "-");
        if format == ExportFormat::Json {
            let out = dir.join(format!("{flat}.json"));
            let bytes = std::fs::read(&cv_path).map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => CoreError::NotFound(name.to_string()),
                _ => e.into(),
            })?;
            std::fs::write(&out, bytes)?;
            return Ok(out);
        }
        let id = self.effective_theme(name, theme)?;
        let html = self.render_cv(name, Some(&id), None)?;
        let ext = if format == ExportFormat::Pdf { "pdf" } else { "html" };
        let out = dir.join(format!("{flat}-{}.{ext}", id.strip_prefix("builtin:").unwrap_or(&id)));
        if format == ExportFormat::Pdf {
            html_to_pdf(&html, &out).await?;
        } else {
            std::fs::write(&out, html)?;
        }
        Ok(out)
    }
}
