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

/// Opens `configured` with history in `db`; if that fails, opens `default` with `default_db` and
/// records why. Fails only if the defaults fail too.
pub fn open_with_fallback(
    configured: &Path,
    default: &Path,
    db: &Path,
) -> Result<(Library, Option<Fallback>), CoreError> {
    match Library::open_configured(configured, default, db) {
        Ok(lib) => Ok((lib, None)),
        Err(e) if configured != default => {
            let lib = Library::open(default, db)?;
            Ok((
                lib,
                Some(Fallback {
                    path: configured.to_string_lossy().into_owned(),
                    message: e.to_string(),
                }),
            ))
        }
        Err(e) => Err(e),
    }
}

/// Event the UI answers by running its unsaved-draft guard, then calling `exit_app`.
pub const QUIT_REQUESTED: &str = "app-quit-requested";
/// Event carrying a menu command id (`new-cv`, `edit`, `export-pdf`, `print`, `settings`) for the UI.
pub const MENU_COMMAND: &str = "menu-command";
/// Menu items the UI handles; the ids are also the `MENU_COMMAND` payloads.
#[cfg(target_os = "macos")]
const UI_COMMANDS: [(&str, &str, &str); 4] = [
    ("new-cv", "New Tailored CV…", "CmdOrCtrl+N"),
    ("edit", "Edit CV", "CmdOrCtrl+E"),
    ("export-pdf", "Export PDF", "CmdOrCtrl+Shift+E"),
    ("print", "Print…", "CmdOrCtrl+P"),
];

/// The macOS default menu, with Quit (⌘Q) routed through the UI. The stock Quit item calls
/// NSApp `terminate:`, which exits without any Tauri event, so unsaved edits could not be saved.
#[cfg(target_os = "macos")]
fn guarded_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    let menu = Menu::default(app)?;
    let submenus: Vec<_> = menu
        .items()?
        .iter()
        .filter_map(|i| i.as_submenu().cloned())
        .collect();
    // File menu: the UI commands above the stock Close Window.
    if let Some(file) = submenus
        .iter()
        .find(|m| m.text().is_ok_and(|t| t == "File"))
    {
        for (i, (id, label, accel)) in UI_COMMANDS.iter().enumerate() {
            file.insert(&MenuItem::with_id(app, *id, *label, true, Some(*accel))?, i)?;
        }
        file.insert(&PredefinedMenuItem::separator(app)?, UI_COMMANDS.len())?;
    }
    if let Some(app_menu) = submenus.first() {
        // After "About …" and its separator.
        app_menu.insert(
            &MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?,
            2,
        )?;
        if let Some(quit) = app_menu.items()?.last() {
            app_menu.remove(quit)?;
        }
        let label = format!("Quit {}", app.package_info().name);
        app_menu.append(&MenuItem::with_id(
            app,
            "quit",
            label,
            true,
            Some("CmdOrCtrl+Q"),
        )?)?;
    }
    Ok(menu)
}

pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.menu(guarded_menu);
    builder
        .on_menu_event(|app, e| match e.id().as_ref() {
            "quit" => {
                let _ = app.emit(QUIT_REQUESTED, ());
            }
            id @ ("new-cv" | "edit" | "export-pdf" | "print" | "settings") => {
                let _ = app.emit(MENU_COMMAND, id);
            }
            _ => {}
        })
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let cfg = config::load();
            let (lib, fallback) = open_with_fallback(
                &cfg.library,
                &config::default_library(),
                &config::default_history(),
            )?;
            let watcher = start_watcher(app.handle().clone(), lib.clone());
            app.manage(AppState {
                lib: Mutex::new(lib),
                fallback: Mutex::new(fallback),
                watcher: Mutex::new(watcher),
            });
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol(PREVIEW_SCHEME, |ctx, req, responder| {
            let lib = ctx.app_handle().state::<AppState>().lib();
            let uri = req.uri().to_string();
            tauri::async_runtime::spawn_blocking(move || {
                let html = render_uri(&lib, &uri);
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
            commands::change_diff,
            commands::preview_html,
            commands::restore,
            commands::list_themes,
            commands::theme_report,
            commands::duplicate_theme,
            commands::set_cv_theme,
            commands::set_page_mode,
            commands::connect_agent,
            commands::export,
            commands::print_cv,
            commands::add_asset_from_path,
            commands::get_config,
            commands::set_library,
            commands::reveal,
            commands::binary_path,
            commands::theme_dir_path,
            commands::exit_app,
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

fn render_uri(lib: &Library, uri: &str) -> String {
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
    let Some(cv) = cv else {
        return error_page("Missing cv parameter", None);
    };
    render_preview(lib, &cv, theme.as_deref(), commit.as_deref())
}

/// The preview page for a CV: rendered HTML, the last saved version with a notice, or an error page.
pub fn render_preview(
    lib: &Library,
    cv: &str,
    theme: Option<&str>,
    commit: Option<&str>,
) -> String {
    let theme = theme.filter(|s| !s.is_empty());
    let commit = commit.filter(|s| !s.is_empty());
    // Spec §5.3: a file broken outside the app shows its last saved (valid) version plus a notice.
    let broken = commit.is_none()
        && match lib.read_cv(cv) {
            Err(CoreError::Json(_)) => true,
            Ok(doc) => jsoncv_core::schema::validate(&doc.cv).is_err(),
            Err(_) => false,
        };
    let res = if broken {
        let last = lib
            .history(cv, 1)
            .ok()
            .and_then(|h| h.into_iter().next())
            .map_or(String::new(), |v| v.commit);
        lib.render_cv(cv, theme, Some(&last))
            .map(|html| with_notice(&html))
    } else {
        lib.render_cv(cv, theme, commit)
    };
    match res {
        Ok(html) => html,
        Err(CoreError::Render(e)) => error_page(&e.message, e.line),
        Err(e) if broken => error_page(
            &format!("This file has an error and no saved version: {e}"),
            None,
        ),
        Err(e) => error_page(&e.to_string(), None),
    }
}

/// Adds a fixed notice bar to a rendered page (inline style only; the preview CSP allows no scripts).
fn with_notice(html: &str) -> String {
    let bar = "<div style=\"position:fixed;top:0;left:0;right:0;z-index:2147483647;padding:6px 12px;background:#fff3cd;color:#664d03;border-bottom:1px solid #e0c36b;font:13px system-ui,sans-serif\">This file has an error; showing the last saved version.</div>";
    match html.rfind("</body>") {
        Some(i) => format!("{}{bar}{}", &html[..i], &html[i..]),
        None => format!("{html}{bar}"),
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn error_page(message: &str, line: Option<usize>) -> String {
    let line = line
        .map(|l| format!("<p>Template line {l}</p>"))
        .unwrap_or_default();
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
        assert_eq!(
            r("/lib/tailored/a.cv.json").as_deref(),
            Some("tailored/a.cv.json")
        );
        assert_eq!(r("/lib/.git/index"), None);
        assert_eq!(r("/lib/exports/main.pdf"), None);
        assert_eq!(r("/lib/.main.cv.json.tmp-123"), None);
        assert_eq!(r("/lib"), None);
        assert_eq!(r("/other/main.cv.json"), None);
    }

    #[test]
    fn preview_renders_and_reports_errors() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(dir.path(), &dir.path().join(".history.db")).unwrap();
        let ok = render_uri(&lib, preview_url("main").as_str());
        assert!(!ok.contains("Cannot render"), "{ok}");
        let bad = render_uri(
            &lib,
            "http://jsoncv-preview.localhost/render?cv=tailored%2Fnope",
        );
        assert!(
            bad.contains("Cannot render") && bad.contains("tailored/nope"),
            "{bad}"
        );
        assert!(render_uri(&lib, "jsoncv-preview://localhost/render").contains("Missing cv"));
    }

    #[test]
    fn preview_of_broken_file_shows_last_saved_version() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(dir.path(), &dir.path().join(".history.db")).unwrap();
        let url = preview_url("main");
        std::fs::write(dir.path().join("main.cv.json"), "{").unwrap();
        let page = render_uri(&lib, url.as_str());
        assert!(
            page.contains("Mara Lindqvist") && page.contains("showing the last saved version"),
            "{page}"
        );
        std::fs::write(
            dir.path().join("main.cv.json"),
            r#"{"basics":{"name":"Broken"},"work":[{"startDate":"March"}]}"#,
        )
        .unwrap();
        let page = render_uri(&lib, url.as_str());
        assert!(
            page.contains("Mara Lindqvist") && !page.contains("Broken"),
            "{page}"
        );
        assert!(page.contains("showing the last saved version"));
    }

    #[test]
    fn falls_back_to_default_library() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("file");
        std::fs::write(&blocker, "x").unwrap();
        let bad = blocker.join("lib"); // a path below a regular file cannot be created
        let good = dir.path().join("default");
        let db = dir.path().join("history.db");
        let fallback = |lib: &Path, def: &Path| open_with_fallback(lib, def, &db);
        let (lib, fb) = fallback(&good, &good).unwrap();
        assert!(fb.is_none() && lib.root() == good);
        let (lib, fb) = fallback(&bad, &good).unwrap();
        let fb = fb.expect("fallback recorded");
        assert_eq!(lib.root(), good);
        assert_eq!(fb.path, bad.to_string_lossy());
        assert!(!fb.message.is_empty());
        assert!(fallback(&bad, &bad).is_err());
        let gone = dir.path().join("gone");
        let (_, fb) = fallback(&gone, &good).unwrap();
        assert!(fb.is_some() && !gone.exists());
    }

    #[test]
    fn error_page_escapes() {
        let page = error_page("<script>x</script>", Some(3));
        assert!(page.contains("&lt;script&gt;") && page.contains("line 3"));
    }
}
