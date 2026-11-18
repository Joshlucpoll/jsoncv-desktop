use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use jsoncv_core::{config, library::Library, CoreError};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::http::{header, Response};
use tauri::{AppHandle, Emitter, Manager, Url};

use crate::commands;

pub const PREVIEW_SCHEME: &str = "jsoncv-preview";
const PREVIEW_CSP: &str =
    "default-src 'none'; style-src 'unsafe-inline'; img-src data: https: http:; font-src data:";
const DEBOUNCE: Duration = Duration::from_millis(300);
const SWEEP_DELAY: Duration = Duration::from_secs(2);

/// The configured library that could not be opened at startup, and why.
#[derive(Clone, Serialize)]
pub struct Fallback {
    pub path: String,
    pub message: String,
}

pub struct AppState {
    pub lib: Mutex<Library>,
    pub fallback: Mutex<Option<Fallback>>,
    pub watcher: Mutex<Option<RecommendedWatcher>>,
}

/// Locks ignoring poison: a panicked command must not brick every later one.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppState {
    /// A clone of the current library, so no lock is held across slow work.
    pub fn lib(&self) -> Library {
        lock(&self.lib).clone()
    }
}

/// Opens `configured`; if that fails, opens `default` and records why. Fails only if `default` fails too.
pub fn open_with_fallback(configured: &Path, default: &Path) -> Result<(Library, Option<Fallback>), CoreError> {
    match Library::open_configured(configured, default) {
        Ok(lib) => Ok((lib, None)),
        Err(e) if configured != default => {
            let lib = Library::open(default)?;
            Ok((lib, Some(Fallback { path: configured.to_string_lossy().into_owned(), message: e.to_string() })))
        }
        Err(e) => Err(e),
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let (lib, fallback) = open_with_fallback(&config::load().library, &config::default_library())?;
            let watcher = start_watcher(app.handle().clone(), lib.clone());
            app.manage(AppState { lib: Mutex::new(lib), fallback: Mutex::new(fallback), watcher: Mutex::new(watcher) });
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol(PREVIEW_SCHEME, |ctx, req, responder| {
            let lib = ctx.app_handle().state::<AppState>().lib();
            let uri = req.uri().to_string();
            tauri::async_runtime::spawn_blocking(move || {
                let html = render_preview(&lib, &uri);
                let res = Response::builder()
                    .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                    .header(header::CONTENT_SECURITY_POLICY, PREVIEW_CSP)
                    .body(html.into_bytes())
                    .expect("static headers are valid");
                responder.respond(res);
            });
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_cvs,
            commands::get_schema,
            commands::read_cv,
            commands::save_draft,
            commands::create_tailored,
            commands::rename_cv,
            commands::delete_cv,
            commands::history,
            commands::diff,
            commands::restore,
            commands::list_themes,
            commands::theme_report,
            commands::duplicate_theme,
            commands::set_cv_theme,
            commands::export,
            commands::print_cv,
            commands::add_asset_from_path,
            commands::get_config,
            commands::set_library,
            commands::reveal,
            commands::binary_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// The URL that loads `name` through the preview scheme (platform-specific host form).
pub fn preview_url(name: &str) -> Url {
    let base = if cfg!(any(windows, target_os = "android")) {
        format!("http://{PREVIEW_SCHEME}.localhost/render")
    } else {
        format!("{PREVIEW_SCHEME}://localhost/render")
    };
    let mut url = Url::parse(&base).expect("static url");
    url.query_pairs_mut().append_pair("cv", name);
    url
}

fn render_preview(lib: &Library, uri: &str) -> String {
    let (mut cv, mut theme, mut commit) = (None, None, None);
    if let Ok(url) = Url::parse(uri) {
        for (k, v) in url.query_pairs() {
            match &*k {
                "cv" => cv = Some(v.into_owned()),
                "theme" => theme = Some(v.into_owned()),
                "commit" => commit = Some(v.into_owned()),
                _ => {}
            }
        }
    }
    let Some(cv) = cv else { return error_page("Missing cv parameter", None) };
    match lib.render_cv(&cv, theme.as_deref().filter(|s| !s.is_empty()), commit.as_deref().filter(|s| !s.is_empty())) {
        Ok(html) => html,
        Err(CoreError::Render(e)) => error_page(&e.message, e.line),
        Err(e) => error_page(&e.to_string(), None),
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn error_page(message: &str, line: Option<usize>) -> String {
    let line = line.map(|l| format!("<p>Template line {l}</p>")).unwrap_or_default();
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>body{{font:14px system-ui,sans-serif;padding:2em;color:#a00}}pre{{white-space:pre-wrap}}</style></head><body><h1>Cannot render this CV</h1><pre>{}</pre>{line}</body></html>",
        escape(message)
    )
}

#[derive(Clone, Serialize)]
struct LibraryChanged {
    paths: Vec<String>,
}

/// The library-relative path (`/`-separated) of an event path, or None if the app should ignore it.
fn relevant(roots: &[PathBuf], p: &Path) -> Option<String> {
    let rel = roots.iter().find_map(|r| p.strip_prefix(r).ok())?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let first = parts.first()?;
    let file = parts.last()?;
    if first == ".git" || first == "exports" || (file.starts_with('.') && file.contains(".tmp-")) {
        return None;
    }
    Some(parts.join("/"))
}

/// Watches the library; emits `library-changed` after 300 ms of quiet and sweeps external edits after 2 s.
/// The thread ends when the returned watcher is dropped (its channel disconnects).
pub fn start_watcher(app: AppHandle, lib: Library) -> Option<RecommendedWatcher> {
    let (tx, rx) = mpsc::channel();
    let watcher = notify::recommended_watcher(tx).and_then(|mut w| {
        w.watch(lib.root(), RecursiveMode::Recursive)?;
        Ok(w)
    });
    let watcher = match watcher {
        Ok(w) => w,
        Err(e) => {
            eprintln!("jsoncv: cannot watch {}: {e}", lib.root().display());
            return None;
        }
    };
    // FSEvents reports canonical paths (/private/var/…), so match both forms of the root.
    let mut roots = vec![lib.root().to_path_buf()];
    roots.extend(lib.root().canonicalize().ok());
    std::thread::spawn(move || {
        let mut pending = BTreeSet::new();
        let mut last = Instant::now();
        let mut sweep_due = false;
        loop {
            let wait = if !pending.is_empty() {
                Some(DEBOUNCE)
            } else if sweep_due {
                Some(SWEEP_DELAY)
            } else {
                None
            };
            let msg = match wait {
                Some(d) => rx.recv_timeout(d.saturating_sub(last.elapsed())),
                None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match msg {
                Ok(Ok(ev)) => {
                    for rel in ev.paths.iter().filter_map(|p| relevant(&roots, p)) {
                        pending.insert(rel);
                        last = Instant::now();
                        sweep_due = true;
                    }
                }
                Ok(Err(e)) => eprintln!("jsoncv: watcher error: {e}"),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if !pending.is_empty() && last.elapsed() >= DEBOUNCE {
                let paths = std::mem::take(&mut pending).into_iter().collect();
                if let Err(e) = app.emit("library-changed", LibraryChanged { paths }) {
                    eprintln!("jsoncv: emit library-changed: {e}");
                }
            }
            if sweep_due && pending.is_empty() && last.elapsed() >= SWEEP_DELAY {
                match lib.sweep_external_locked() {
                    // Another writer holds the lock: try again after another quiet period.
                    Err(CoreError::Locked) => last = Instant::now(),
                    Err(e) => {
                        sweep_due = false;
                        eprintln!("jsoncv: sweep failed: {e}");
                    }
                    Ok(_) => sweep_due = false,
                }
            }
        }
    });
    Some(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relevant_filters_paths() {
        let roots = [PathBuf::from("/lib")];
        let r = |p: &str| relevant(&roots, Path::new(p));
        assert_eq!(r("/lib/main.cv.json").as_deref(), Some("main.cv.json"));
        assert_eq!(r("/lib/tailored/a.cv.json").as_deref(), Some("tailored/a.cv.json"));
        assert_eq!(r("/lib/.git/index"), None);
        assert_eq!(r("/lib/exports/main.pdf"), None);
        assert_eq!(r("/lib/.main.cv.json.tmp-123"), None);
        assert_eq!(r("/lib"), None);
        assert_eq!(r("/other/main.cv.json"), None);
    }

    #[test]
    fn preview_renders_and_reports_errors() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(dir.path()).unwrap();
        let ok = render_preview(&lib, preview_url("main").as_str());
        assert!(!ok.contains("Cannot render"), "{ok}");
        let bad = render_preview(&lib, "http://jsoncv-preview.localhost/render?cv=tailored%2Fnope");
        assert!(bad.contains("Cannot render") && bad.contains("tailored/nope"), "{bad}");
        assert!(render_preview(&lib, "jsoncv-preview://localhost/render").contains("Missing cv"));
    }

    #[test]
    fn falls_back_to_default_library() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("file");
        std::fs::write(&blocker, "x").unwrap();
        let bad = blocker.join("lib"); // a path below a regular file cannot be created
        let good = dir.path().join("default");
        let (lib, fb) = open_with_fallback(&good, &good).unwrap();
        assert!(fb.is_none() && lib.root() == good);
        let (lib, fb) = open_with_fallback(&bad, &good).unwrap();
        let fb = fb.expect("fallback recorded");
        assert_eq!(lib.root(), good);
        assert_eq!(fb.path, bad.to_string_lossy());
        assert!(!fb.message.is_empty());
        assert!(open_with_fallback(&bad, &bad).is_err());
        let gone = dir.path().join("gone");
        let (_, fb) = open_with_fallback(&gone, &good).unwrap();
        assert!(fb.is_some() && !gone.exists());
    }

    #[test]
    fn error_page_escapes() {
        let page = error_page("<script>x</script>", Some(3));
        assert!(page.contains("&lt;script&gt;") && page.contains("line 3"));
    }
}
