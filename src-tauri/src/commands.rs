use std::path::{Path, PathBuf};

use jsoncv_core::coverage::ThemeReport;
use jsoncv_core::export::ExportFormat;
use jsoncv_core::history::Version;
use jsoncv_core::library::{CvInfo, Library, Saved};
use jsoncv_core::render::ThemeInfo;
use jsoncv_core::schema::SchemaError;
use jsoncv_core::{config, Author, CoreError};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::app::{lock, preview_url, start_watcher, AppState};

const MAX_ASSET: u64 = 20 * 1024 * 1024;

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
            CoreError::Git(_) => "Git",
        };
        let message = e.to_string();
        let errors = match e {
            CoreError::Invalid(v) => Some(v),
            _ => None,
        };
        ApiError { kind: kind.into(), message, errors }
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

#[tauri::command]
pub fn list_cvs(state: State<AppState>) -> Res<Vec<CvInfo>> {
    Ok(state.lib().list_cvs()?)
}

#[tauri::command]
pub fn read_cv(state: State<AppState>, name: String) -> Res<Value> {
    let doc = state.lib().read_cv(&name)?;
    Ok(json!({ "name": doc.name, "cv": doc.cv, "base": doc.base }))
}

#[tauri::command]
pub fn save_draft(state: State<AppState>, name: String, base_cv: Value, draft: Value) -> Res<Saved> {
    Ok(state.lib().save_draft(&name, &base_cv, &draft)?)
}

#[tauri::command]
pub fn create_tailored(state: State<AppState>, slug: String, from: String) -> Res<CvInfo> {
    Ok(state.lib().create_tailored(&slug, &from, Author::You)?)
}

#[tauri::command]
pub fn rename_cv(state: State<AppState>, name: String, new_slug: String) -> Res<CvInfo> {
    Ok(state.lib().rename_cv(&name, &new_slug, Author::You)?)
}

#[tauri::command]
pub fn delete_cv(state: State<AppState>, name: String) -> Res<()> {
    Ok(state.lib().delete_cv(&name, Author::You)?)
}

#[tauri::command]
pub fn history(state: State<AppState>, name: String, limit: usize) -> Res<Vec<Version>> {
    Ok(state.lib().history(&name, limit)?)
}

#[tauri::command]
pub fn diff(state: State<AppState>, name: String, commit: String) -> Res<String> {
    Ok(state.lib().diff(&name, &commit)?)
}

#[tauri::command]
pub fn restore(state: State<AppState>, name: String, commit: String) -> Res<Saved> {
    Ok(state.lib().restore(&name, &commit, Author::You)?)
}

#[tauri::command]
pub fn list_themes(state: State<AppState>) -> Res<Vec<ThemeInfo>> {
    Ok(state.lib().list_themes()?)
}

#[tauri::command]
pub fn theme_report(state: State<AppState>, id: String) -> Res<ThemeReport> {
    Ok(state.lib().theme_report(&id)?)
}

#[tauri::command]
pub fn duplicate_theme(state: State<AppState>, id: String, new_id: String) -> Res<ThemeInfo> {
    Ok(state.lib().duplicate_theme(&id, &new_id, Author::You)?)
}

#[tauri::command]
pub fn set_cv_theme(state: State<AppState>, name: String, theme_id: String) -> Res<Saved> {
    Ok(state.lib().set_cv_theme(&name, &theme_id, Author::You)?)
}

#[tauri::command]
pub async fn export(state: State<'_, AppState>, name: String, format: ExportFormat) -> Res<String> {
    let lib = state.lib();
    Ok(lib.export(&name, format, None).await?.to_string_lossy().into_owned())
}

/// Opens a window on the preview URL and prints it once loaded. Async: window creation in a
/// sync command deadlocks on Windows.
#[tauri::command]
pub async fn print_cv(app: AppHandle, state: State<'_, AppState>, name: String) -> Res<()> {
    state.lib().read_cv(&name)?;
    let label: String =
        format!("print-{name}").chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let url = preview_url(&name);
    if let Some(w) = app.get_webview_window(&label) {
        // Reloading fires the window's page-load handler, which prints again.
        w.navigate(url)?;
        w.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(&app, label, WebviewUrl::CustomProtocol(url))
        .title(format!("Print – {name}"))
        .on_page_load(|w, p| {
            if p.event() == PageLoadEvent::Finished {
                if let Err(e) = w.print() {
                    eprintln!("jsoncv: print failed: {e}");
                }
            }
        })
        .build()?;
    Ok(())
}

#[tauri::command]
pub fn add_asset_from_path(state: State<AppState>, path: PathBuf) -> Res<String> {
    if std::fs::metadata(&path)?.len() > MAX_ASSET {
        return Err(CoreError::BadPath("Asset too large (max 20 MB)".into()).into());
    }
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let bytes = std::fs::read(&path)?;
    Ok(state.lib().add_asset(&file_name, &bytes, Author::You)?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryConfig {
    library: String,
    in_sync_folder: bool,
}

fn library_config(lib: &Library) -> LibraryConfig {
    LibraryConfig {
        library: lib.root().to_string_lossy().into_owned(),
        in_sync_folder: config::is_in_sync_folder(lib.root()),
    }
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> LibraryConfig {
    library_config(&state.lib())
}

#[tauri::command]
pub fn set_library(app: AppHandle, state: State<AppState>, path: PathBuf) -> Res<LibraryConfig> {
    if !path.is_absolute() {
        return Err(CoreError::BadPath(format!("Library path must be absolute: {}", path.display())).into());
    }
    let lib = Library::open(&path)?;
    config::save(&config::Config { library: path })?;
    let watcher = start_watcher(app, lib.clone());
    let out = library_config(&lib);
    *lock(&state.lib) = lib;
    *lock(&state.watcher) = watcher;
    Ok(out)
}

#[tauri::command]
pub fn reveal(path: PathBuf) -> Res<()> {
    tauri_plugin_opener::reveal_item_in_dir(Path::new(&path))
        .map_err(|e| std::io::Error::other(e.to_string()).into())
}

#[tauri::command]
pub fn binary_path() -> Res<String> {
    Ok(std::env::current_exe()?.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(kind(CoreError::Render(RenderError { message: s(), line: Some(2) })), "Render");
        assert_eq!(kind(CoreError::Io(std::io::Error::other("x"))), "Io");
        assert_eq!(kind(serde_json::from_str::<Value>("{").unwrap_err().into()), "Json");
        assert_eq!(kind(git2::Error::from_str("x").into()), "Git");
    }

    #[test]
    fn invalid_carries_schema_errors() {
        let e = ApiError::from(CoreError::Invalid(vec![SchemaError { path: "/basics".into(), message: "bad".into() }]));
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["errors"][0]["path"], "/basics");
        let v = serde_json::to_value(ApiError::from(CoreError::Stale)).unwrap();
        assert!(v.get("errors").is_none());
    }
}
