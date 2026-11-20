use std::path::{Path, PathBuf};

use jsoncv_core::coverage::ThemeReport;
use jsoncv_core::export::ExportFormat;
use jsoncv_core::history::Version;
use jsoncv_core::library::{CvInfo, Library, Saved};
use jsoncv_core::render::ThemeInfo;
use jsoncv_core::schema::SchemaError;
use jsoncv_core::{config, Author, CoreError, MAX_ASSET_BYTES};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::app::{lock, preview_url, start_watcher, AppState, Fallback};

#[derive(Debug, Serialize)]
pub struct ApiError {
    kind: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<Vec<SchemaError>>,
}

impl From<CoreError> for ApiError {
    fn from(e: CoreError) -> Self {
        let kind = match &e {
            CoreError::Invalid(_) => "Invalid",
            CoreError::Stale => "Stale",
            CoreError::PatchFailed(_) => "PatchFailed",
            CoreError::NotFound(_) => "NotFound",
            CoreError::BadName(_) => "BadName",
            CoreError::BadPath(_) => "BadPath",
            CoreError::Locked => "Locked",
            CoreError::ReadOnly(_) => "ReadOnly",
            CoreError::Browser(_) => "Browser",
            CoreError::Render(_) => "Render",
            CoreError::Io(_) => "Io",
            CoreError::Json(_) => "Json",
            CoreError::Db(_) => "Db",
        };
        let message = e.to_string();
        let errors = match e {
            CoreError::Invalid(v) => Some(v),
            _ => None,
        };
        ApiError {
            kind: kind.into(),
            message,
            errors,
        }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        CoreError::Io(e).into()
    }
}

// Window/plugin failures have no CoreError variant; they surface as Io.
impl From<tauri::Error> for ApiError {
    fn from(e: tauri::Error) -> Self {
        std::io::Error::other(e.to_string()).into()
    }
}

type Res<T> = Result<T, ApiError>;

/// Runs blocking file/database work off the main thread. Sync Tauri commands run inline on the
/// main thread, and core's `with_lock` can sleep-retry for up to 2 s.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))?
}

/// `blocking` with a clone of the current library.
async fn with_lib<T: Send + 'static>(
    state: &State<'_, AppState>,
    f: impl FnOnce(Library) -> Result<T, CoreError> + Send + 'static,
) -> Res<T> {
    let lib = state.lib();
    blocking(move || Ok(f(lib)?)).await
}

#[tauri::command]
pub async fn list_cvs(state: State<'_, AppState>) -> Res<Vec<CvInfo>> {
    with_lib(&state, |l| l.list_cvs()).await
}

#[tauri::command]
pub fn get_schema() -> Result<Value, ApiError> {
    serde_json::from_str(jsoncv_core::schema::SCHEMA).map_err(|e| CoreError::from(e).into())
}

#[tauri::command]
pub async fn read_cv(state: State<'_, AppState>, name: String) -> Res<Value> {
    let doc = with_lib(&state, move |l| l.read_cv(&name)).await?;
    Ok(json!({ "name": doc.name, "cv": doc.cv, "base": doc.base }))
}

#[tauri::command]
pub async fn save_draft(
    state: State<'_, AppState>,
    name: String,
    base_cv: Value,
    draft: Value,
) -> Res<Saved> {
    with_lib(&state, move |l| l.save_draft(&name, &base_cv, &draft)).await
}

#[tauri::command]
pub async fn duplicate_cv(state: State<'_, AppState>, slug: String, from: String) -> Res<CvInfo> {
    with_lib(&state, move |l| l.duplicate_cv(&slug, &from, Author::You)).await
}

#[tauri::command]
pub async fn create_cv(
    state: State<'_, AppState>,
    slug: String,
    folder: Option<String>,
) -> Res<CvInfo> {
    with_lib(&state, move |l| {
        l.create_cv(&slug, folder.as_deref(), Author::You)
    })
    .await
}

#[tauri::command]
pub async fn move_cv(
    state: State<'_, AppState>,
    name: String,
    folder: Option<String>,
) -> Res<CvInfo> {
    with_lib(&state, move |l| l.move_cv(&name, folder.as_deref())).await
}

#[tauri::command]
pub async fn list_folders(state: State<'_, AppState>) -> Res<Vec<String>> {
    with_lib(&state, |l| l.list_folders()).await
}

#[tauri::command]
pub async fn create_folder(state: State<'_, AppState>, name: String) -> Res<()> {
    with_lib(&state, move |l| l.create_folder(&name)).await
}

#[tauri::command]
pub async fn rename_folder(state: State<'_, AppState>, name: String, new_name: String) -> Res<()> {
    with_lib(&state, move |l| l.rename_folder(&name, &new_name)).await
}

#[tauri::command]
pub async fn delete_folder(state: State<'_, AppState>, name: String) -> Res<()> {
    with_lib(&state, move |l| l.delete_folder(&name)).await
}

#[tauri::command]
pub async fn set_pinned(state: State<'_, AppState>, name: String, pinned: bool) -> Res<()> {
    with_lib(&state, move |l| l.set_pinned(&name, pinned)).await
}

#[tauri::command]
pub async fn get_order(state: State<'_, AppState>) -> Res<Vec<String>> {
    with_lib(&state, |l| Ok(l.order())).await
}

#[tauri::command]
pub async fn set_order(state: State<'_, AppState>, keys: Vec<String>) -> Res<()> {
    with_lib(&state, move |l| l.set_order(&keys)).await
}

#[tauri::command]
pub async fn set_pin_order(state: State<'_, AppState>, names: Vec<String>) -> Res<()> {
    with_lib(&state, move |l| l.set_pin_order(&names)).await
}

#[tauri::command]
pub async fn rename_cv(state: State<'_, AppState>, name: String, new_slug: String) -> Res<CvInfo> {
    with_lib(&state, move |l| l.rename_cv(&name, &new_slug, Author::You)).await
}

#[tauri::command]
pub async fn delete_cv(state: State<'_, AppState>, name: String) -> Res<()> {
    with_lib(&state, move |l| l.delete_cv(&name, Author::You)).await
}

#[tauri::command]
pub async fn history(state: State<'_, AppState>, name: String, limit: usize) -> Res<Vec<Version>> {
    with_lib(&state, move |l| l.history(&name, limit)).await
}

#[tauri::command]
pub async fn diff(state: State<'_, AppState>, name: String, commit: String) -> Res<String> {
    with_lib(&state, move |l| l.diff(&name, &commit)).await
}

#[tauri::command]
pub async fn change_diff(state: State<'_, AppState>, name: String, commit: String) -> Res<String> {
    with_lib(&state, move |l| l.change_diff(&name, &commit)).await
}

#[tauri::command]
pub async fn preview_html(
    state: State<'_, AppState>,
    cv: String,
    theme: Option<String>,
    commit: Option<String>,
) -> Res<String> {
    let lib = state.lib();
    blocking(move || {
        Ok(crate::app::render_preview(
            &lib,
            &cv,
            theme.as_deref(),
            commit.as_deref(),
        ))
    })
    .await
}

#[tauri::command]
pub async fn restore(state: State<'_, AppState>, name: String, commit: String) -> Res<Saved> {
    with_lib(&state, move |l| l.restore(&name, &commit, Author::You)).await
}

#[tauri::command]
pub async fn list_themes(state: State<'_, AppState>) -> Res<Vec<ThemeInfo>> {
    with_lib(&state, |l| l.list_themes()).await
}

#[tauri::command]
pub async fn theme_report(state: State<'_, AppState>, id: String) -> Res<ThemeReport> {
    with_lib(&state, move |l| l.theme_report(&id)).await
}

#[tauri::command]
pub async fn duplicate_theme(
    state: State<'_, AppState>,
    id: String,
    new_id: String,
    name: Option<String>,
) -> Res<ThemeInfo> {
    with_lib(&state, move |l| {
        l.duplicate_theme(&id, &new_id, name.as_deref(), Author::You)
    })
    .await
}

#[tauri::command]
pub async fn delete_theme(state: State<'_, AppState>, id: String) -> Res<()> {
    with_lib(&state, move |l| l.delete_theme(&id, Author::You)).await
}

#[tauri::command]
pub async fn set_cv_theme(
    state: State<'_, AppState>,
    name: String,
    theme_id: String,
) -> Res<Saved> {
    with_lib(&state, move |l| {
        l.set_cv_theme(&name, &theme_id, Author::You)
    })
    .await
}

/// `continuous`: the CV prints as one long page; otherwise as paper-sized pages (the default).
#[tauri::command]
pub async fn set_page_mode(
    state: State<'_, AppState>,
    name: String,
    continuous: bool,
) -> Res<Saved> {
    with_lib(&state, move |l| {
        let (v, msg) = if continuous {
            (Some("continuous".into()), "Page setup: continuous page")
        } else {
            (None, "Page setup: multiple pages")
        };
        l.set_cv_meta(&name, "pageMode", v, Author::You, msg)
    })
    .await
}

/// Adds this app's MCP server to `claude-code`, `claude-desktop` or `codex`.
#[tauri::command]
pub async fn connect_agent(agent: String) -> Res<String> {
    let bin = std::env::current_exe()?.to_string_lossy().into_owned();
    blocking(move || Ok(crate::agents::connect(&agent, &bin)?)).await
}

#[tauri::command]
pub async fn export(
    state: State<'_, AppState>,
    name: String,
    format: ExportFormat,
    path: String,
) -> Res<String> {
    let lib = state.lib();
    Ok(lib
        .export_to(&name, format, None, Some(std::path::Path::new(&path)))
        .await?
        .to_string_lossy()
        .into_owned())
}

/// Prints the CV from a webview on the preview URL once it loads. Async: webview creation in a
/// sync command deadlocks on Windows.
#[tauri::command]
pub async fn print_cv(app: AppHandle, state: State<'_, AppState>, name: String) -> Res<()> {
    // Render first so a broken CV or theme reaches the caller instead of printing the error page.
    let n = name.clone();
    with_lib(&state, move |l| l.render_cv(&n, None, None)).await?;
    let label: String = format!("print-{name}")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let url = preview_url(&name);
    if let Some(w) = app.get_webview(&label) {
        // Reloading fires the webview's page-load handler, which prints again.
        w.navigate(url)?;
        return Ok(());
    }
    let on_load = |w: tauri::Webview, p: tauri::webview::PageLoadPayload<'_>| {
        if p.event() == PageLoadEvent::Finished {
            if let Err(e) = w.print() {
                eprintln!("jsoncv: print failed: {e}");
            }
        }
    };
    // WebView2 draws its print preview inside the webview, so Windows needs a visible window.
    if cfg!(windows) {
        WebviewWindowBuilder::new(&app, label, WebviewUrl::CustomProtocol(url))
            .title(format!("Print – {name}"))
            .on_page_load(move |w, p| on_load(w.as_ref().clone(), p))
            .build()?;
    } else {
        // A hidden child of the main window, so the print sheet opens on the main window.
        let main = app
            .get_window("main")
            .ok_or_else(|| CoreError::NotFound("main window".into()))?;
        let page = tauri::webview::WebviewBuilder::new(label, WebviewUrl::CustomProtocol(url))
            .on_page_load(on_load);
        // Off-screen at A4 size at 96 dpi: a zero-size webview may print blank pages.
        main.add_child(
            page,
            tauri::LogicalPosition::new(-10_000.0, 0.0),
            tauri::LogicalSize::new(794.0, 1123.0),
        )?;
    }
    Ok(())
}

#[tauri::command]
pub async fn add_asset_from_path(state: State<'_, AppState>, path: PathBuf) -> Res<String> {
    let lib = state.lib();
    blocking(move || {
        if std::fs::metadata(&path)?.len() > MAX_ASSET_BYTES as u64 {
            return Err(CoreError::BadPath("Asset too large (max 20 MB)".into()).into());
        }
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = std::fs::read(&path)?;
        Ok(lib.add_asset(&file_name, &bytes, Author::You)?)
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryConfig {
    library: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback_from: Option<Fallback>,
}

fn library_config(lib: &Library, fallback_from: Option<Fallback>) -> LibraryConfig {
    LibraryConfig {
        fallback_from,
        library: lib.root().to_string_lossy().into_owned(),
    }
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> LibraryConfig {
    library_config(&state.lib(), lock(&state.fallback).clone())
}

#[tauri::command]
pub async fn set_library(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
) -> Res<LibraryConfig> {
    if !path.is_absolute() {
        return Err(CoreError::BadPath(format!(
            "Library path must be absolute: {}",
            path.display()
        ))
        .into());
    }
    let db = state.lib().db().to_path_buf();
    let lib = blocking(move || {
        let lib = Library::open(&path, &db)?;
        config::save(&config::Config { library: path })?;
        Ok(lib)
    })
    .await?;
    let watcher = start_watcher(app, lib.clone());
    let out = library_config(&lib, None);
    *lock(&state.lib) = lib;
    *lock(&state.fallback) = None;
    *lock(&state.watcher) = watcher;
    Ok(out)
}

#[derive(Serialize)]
pub struct LibraryStats {
    /// Bytes of every file under the library folder.
    bytes: u64,
    /// Files in `assets/` (photos and other uploads).
    assets: u64,
    /// Bytes of the undo-history database, which stays on this machine.
    history_bytes: u64,
}

/// Total size and count of the files under `dir`; a missing folder counts as empty.
fn dir_usage(dir: &Path) -> (u64, u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (0, 0);
    };
    entries
        .flatten()
        .fold((0, 0), |(b, n), e| match e.file_type() {
            Ok(t) if t.is_dir() => {
                let (db, dn) = dir_usage(&e.path());
                (b + db, n + dn)
            }
            Ok(t) if t.is_file() => (b + e.metadata().map(|m| m.len()).unwrap_or(0), n + 1),
            _ => (b, n), // symlinks are skipped so a link loop cannot recurse forever
        })
}

#[tauri::command]
pub async fn library_stats(state: State<'_, AppState>) -> Res<LibraryStats> {
    let lib = state.lib();
    blocking(move || {
        let history_bytes = ["", "-wal"]
            .iter()
            .filter_map(|ext| std::fs::metadata(format!("{}{ext}", lib.db().display())).ok())
            .map(|m| m.len())
            .sum();
        Ok(LibraryStats {
            bytes: dir_usage(lib.root()).0,
            assets: dir_usage(&lib.root().join("assets")).1,
            history_bytes,
        })
    })
    .await
}

#[tauri::command]
pub async fn reveal(path: PathBuf) -> Res<()> {
    blocking(move || {
        tauri_plugin_opener::reveal_item_in_dir(Path::new(&path))
            .map_err(|e| std::io::Error::other(e.to_string()).into())
    })
    .await
}

/// The folder of a library theme, built natively so the UI never joins paths itself.
#[tauri::command]
pub async fn theme_dir_path(state: State<'_, AppState>, id: String) -> Res<String> {
    with_lib(&state, move |l| {
        Ok(l.theme_dir(&id)?.to_string_lossy().into_owned())
    })
    .await
}

#[derive(Serialize)]
pub struct EditorInfo {
    id: String,
    label: String,
}

/// Editors from the fixed list that are installed here.
#[tauri::command]
pub async fn list_editors() -> Res<Vec<EditorInfo>> {
    blocking(|| {
        Ok(crate::editors::installed()
            .into_iter()
            .map(|e| EditorInfo {
                id: e.id.into(),
                label: e.label.into(),
            })
            .collect())
    })
    .await
}

/// Opens a library theme's folder in an editor from the fixed list.
#[tauri::command]
pub async fn open_theme_in(state: State<'_, AppState>, editor: String, id: String) -> Res<()> {
    let dir = with_lib(&state, move |l| {
        let dir = l.theme_dir(&id)?;
        if !dir.is_dir() {
            return Err(CoreError::NotFound(format!("Theme {id}")));
        }
        Ok(dir)
    })
    .await?;
    blocking(move || Ok(crate::editors::open(&editor, &dir)?)).await
}

/// The theme authoring guide (the same text agents read over MCP) as HTML.
#[tauri::command]
pub fn theme_guide_html() -> String {
    jsoncv_core::render::markdown_html(jsoncv_core::THEME_GUIDE)
}

fn is_web_url(url: &str) -> bool {
    let u = url.trim_start().to_ascii_lowercase();
    u.starts_with("https://") || u.starts_with("http://")
}

/// Opens a web link (from the theme guide) in the default browser instead of the app window.
#[tauri::command]
pub async fn open_link(url: String) -> Res<()> {
    if !is_web_url(&url) {
        return Err(CoreError::BadPath(url).into());
    }
    blocking(move || {
        tauri_plugin_opener::open_url(&url, None::<&str>)
            .map_err(|e| std::io::Error::other(e.to_string()).into())
    })
    .await
}

/// Whether an agent has edited a CV in this library; the theme guide hides "Set up an agent" then.
#[tauri::command]
pub async fn agent_has_edited(state: State<'_, AppState>) -> Res<bool> {
    with_lib(&state, |l| l.agent_has_edited()).await
}

/// Called by the UI once its unsaved-draft guard allows quitting.
#[tauri::command]
pub fn exit_app(app: AppHandle) {
    app.exit(0)
}

#[tauri::command]
pub fn binary_path() -> Res<String> {
    Ok(std::env::current_exe()?.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_links_open_externally() {
        assert!(is_web_url("https://docs.rs/minijinja"));
        assert!(is_web_url("http://example.com"));
        assert!(!is_web_url("file:///etc/passwd"));
        assert!(!is_web_url("javascript:alert(1)"));
        assert!(!is_web_url("/Applications/Calculator.app"));
    }

    #[test]
    fn dir_usage_sums_nested_files() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("a")).unwrap();
        std::fs::write(d.path().join("x"), b"123").unwrap();
        std::fs::write(d.path().join("a/y"), b"45").unwrap();
        assert_eq!(dir_usage(d.path()), (5, 2));
        assert_eq!(dir_usage(&d.path().join("missing")), (0, 0));
    }
    use jsoncv_core::render::RenderError;

    fn kind(e: CoreError) -> String {
        ApiError::from(e).kind
    }

    #[test]
    fn core_error_kinds_are_variant_names() {
        let s = || "x".to_string();
        assert_eq!(kind(CoreError::Invalid(vec![])), "Invalid");
        assert_eq!(kind(CoreError::Stale), "Stale");
        assert_eq!(kind(CoreError::PatchFailed(s())), "PatchFailed");
        assert_eq!(kind(CoreError::NotFound(s())), "NotFound");
        assert_eq!(kind(CoreError::BadName(s())), "BadName");
        assert_eq!(kind(CoreError::BadPath(s())), "BadPath");
        assert_eq!(kind(CoreError::Locked), "Locked");
        assert_eq!(kind(CoreError::ReadOnly(s())), "ReadOnly");
        assert_eq!(kind(CoreError::Browser(s())), "Browser");
        assert_eq!(
            kind(CoreError::Render(RenderError {
                message: s(),
                line: Some(2)
            })),
            "Render"
        );
        assert_eq!(kind(CoreError::Io(std::io::Error::other("x"))), "Io");
        assert_eq!(
            kind(serde_json::from_str::<Value>("{").unwrap_err().into()),
            "Json"
        );
        assert_eq!(kind(rusqlite::Error::InvalidQuery.into()), "Db");
    }

    #[test]
    fn invalid_carries_schema_errors() {
        let e = ApiError::from(CoreError::Invalid(vec![SchemaError {
            path: "/basics".into(),
            message: "bad".into(),
        }]));
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["errors"][0]["path"], "/basics");
        let v = serde_json::to_value(ApiError::from(CoreError::Stale)).unwrap();
        assert!(v.get("errors").is_none());
    }
}
