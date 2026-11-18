//! MCP server over `jsoncv_core`. Stdout carries MCP protocol only: never print to it.
use base64::{engine::general_purpose::STANDARD, Engine};
use jsoncv_core::{
    export::{html_to_png, ExportFormat},
    library::Library,
    render::ThemeMeta,
    schema::SCHEMA, Author, CoreError, SAMPLE_CV, THEME_GUIDE,
};
use rmcp::{
    handler::server::wrapper::Parameters,
    model::{
        CallToolResult, ContentBlock, ErrorData, ListResourcesResult, ReadResourceRequestParams,
        ReadResourceResponse, ReadResourceResult, Resource, ResourceContents, ServerCapabilities,
        ServerConfig,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
    transport::stdio,
    RoleServer, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const INSTRUCTIONS: &str = "Edit jsoncv CVs. Read jsoncv://schema before editing and jsoncv://theme-guide before writing themes. Prefer update_cv (JSON Patch) for small edits. After save_theme, fix every render_error and consider every missing field. Use preview to see your theme.";

const RESOURCES: [(&str, &str, &str, &str); 3] = [
    ("jsoncv://schema", "schema", "application/json", SCHEMA),
    ("jsoncv://theme-guide", "theme-guide", "text/markdown", THEME_GUIDE),
    ("jsoncv://sample-cv", "sample-cv", "application/json", SAMPLE_CV),
];

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NameArgs {
    /// CV name: `main` or `tailored/<slug>`.
    pub name: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateCvArgs {
    /// CV name: `main` or `tailored/<slug>`.
    pub name: String,
    /// RFC 6902 JSON Patch: an array of ops such as `{"op":"replace","path":"/basics/name","value":"A"}`. Paths are JSON pointers into the CV.
    pub patch: Vec<Value>,
    /// Optional history message for this change.
    pub message: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplaceCvArgs {
    /// CV name: `main` or `tailored/<slug>`.
    pub name: String,
    /// The complete new CV as a JSON object. Must satisfy jsoncv://schema.
    pub cv: Value,
    /// The `base` value returned by get_cv. The write fails if the CV changed since.
    pub base: String,
    /// Optional history message for this change.
    pub message: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateTailoredArgs {
    /// Slug for the new CV. It is created as `tailored/<name>`.
    pub name: String,
    /// CV to copy from. Defaults to `main`.
    pub from: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HistoryArgs {
    /// CV name: `main` or `tailored/<slug>`.
    pub name: String,
    /// Maximum number of versions, newest first. Default 20.
    pub limit: Option<usize>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RestoreArgs {
    /// CV name: `main` or `tailored/<slug>`.
    pub name: String,
    /// Commit id from cv_history. Restoring adds a new version; history is kept.
    pub commit: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ThemeIdArgs {
    /// Theme id, for example `builtin:classic` or a custom theme id.
    pub id: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SaveThemeArgs {
    /// Custom theme id. Ids starting with `builtin:` are read-only.
    pub id: String,
    /// Jinja template (see jsoncv://theme-guide).
    pub template: String,
    /// Theme CSS.
    pub css: String,
    /// Theme metadata: `{"name": "...", "description": "...", "page": {"size": "A4" | "Letter"}}`.
    pub meta: Value,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetThemeArgs {
    /// CV name: `main` or `tailored/<slug>`.
    pub name: String,
    /// Theme id to use for this CV.
    pub theme: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PreviewArgs {
    /// CV name: `main` or `tailored/<slug>`.
    pub name: String,
    /// Theme id. Defaults to the CV's own theme.
    pub theme: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportArgs {
    /// CV name: `main` or `tailored/<slug>`.
    pub name: String,
    /// One of `pdf`, `html`, `json`.
    pub format: String,
    /// Theme id. Defaults to the CV's own theme.
    pub theme: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddAssetArgs {
    /// File name for the asset, for example `photo.png`.
    pub name: String,
    /// Absolute path of a file on disk that the server reads. Give this or `base64`, not both.
    pub path: Option<String>,
    /// Base64 file content. Give this or `path`, not both.
    pub base64: Option<String>,
}

fn json_ok<T: Serialize>(v: &T) -> CallToolResult {
    match serde_json::to_string_pretty(v) {
        Ok(s) => CallToolResult::success(vec![ContentBlock::text(s)]),
        Err(e) => text_err(e.to_string()),
    }
}
fn text_err(s: String) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(s)])
}
fn core_err(e: CoreError) -> CallToolResult {
    match e {
        CoreError::Invalid(errs) => text_err(
            serde_json::to_string_pretty(&json!({ "errors": errs })).unwrap_or_default(),
        ),
        e => text_err(e.to_string()),
    }
}
fn done<T: Serialize>(r: Result<T, CoreError>) -> CallToolResult {
    r.map(|v| json_ok(&v)).unwrap_or_else(core_err)
}

#[derive(Clone)]
pub struct JsoncvServer {
    lib: Library,
}

impl JsoncvServer {
    pub fn new(lib: Library) -> Self {
        Self { lib }
    }
}

#[tool_router(vis = "pub")]
impl JsoncvServer {
    /// List all CVs in the library with theme, modified time and any error.
    #[tool(description = "List all CVs in the library (name, theme, tailored_from, modified_ms, error).")]
    pub async fn list_cvs(&self) -> CallToolResult {
        done(self.lib.list_cvs())
    }

    #[tool(description = "Read a CV. Returns {cv, base}. Pass `base` to replace_cv.")]
    pub async fn get_cv(&self, Parameters(a): Parameters<NameArgs>) -> CallToolResult {
        done(self.lib.read_cv(&a.name).map(|d| json!({ "cv": d.cv, "base": d.base })))
    }

    #[tool(description = "Edit a CV with an RFC 6902 JSON Patch. The result is validated against the schema; invalid results are rejected with per-field errors.")]
    pub async fn update_cv(&self, Parameters(a): Parameters<UpdateCvArgs>) -> CallToolResult {
        let patch: json_patch::Patch = match serde_json::from_value(Value::Array(a.patch)) {
            Ok(p) => p,
            Err(e) => return text_err(format!("bad patch: {e}")),
        };
        done(self.lib.patch_cv(&a.name, &patch, Author::Agent, a.message.as_deref()))
    }

    #[tool(description = "Replace a whole CV. `base` must be the value from get_cv, else the write fails as stale.")]
    pub async fn replace_cv(&self, Parameters(a): Parameters<ReplaceCvArgs>) -> CallToolResult {
        done(self.lib.replace_cv(&a.name, a.cv, &a.base, Author::Agent, a.message.as_deref()))
    }

    #[tool(description = "Create a tailored CV as `tailored/<name>`, copied from `from` (default `main`).")]
    pub async fn create_tailored_cv(&self, Parameters(a): Parameters<CreateTailoredArgs>) -> CallToolResult {
        done(self.lib.create_tailored(&a.name, a.from.as_deref().unwrap_or("main"), Author::Agent))
    }

    #[tool(description = "List versions of a CV, newest first (commit, author, message, time_ms).")]
    pub async fn cv_history(&self, Parameters(a): Parameters<HistoryArgs>) -> CallToolResult {
        done(self.lib.history(&a.name, a.limit.unwrap_or(20)))
    }

    #[tool(description = "Restore a CV to an earlier version. This adds a new version; history is kept.")]
    pub async fn restore_cv_version(&self, Parameters(a): Parameters<RestoreArgs>) -> CallToolResult {
        done(self.lib.restore(&a.name, &a.commit, Author::Agent))
    }

    #[tool(description = "List built-in and custom themes.")]
    pub async fn list_themes(&self) -> CallToolResult {
        done(self.lib.list_themes())
    }

    #[tool(description = "Read a theme: {info, template, css}.")]
    pub async fn get_theme(&self, Parameters(a): Parameters<ThemeIdArgs>) -> CallToolResult {
        done(self.lib.get_theme(&a.id))
    }

    #[tool(description = "Create or update a custom theme. Returns {render_error, missing}: fix every render_error and consider every missing schema field. Built-in themes are read-only.")]
    pub async fn save_theme(&self, Parameters(a): Parameters<SaveThemeArgs>) -> CallToolResult {
        let meta: ThemeMeta = match serde_json::from_value(a.meta) {
            Ok(m) => m,
            Err(e) => return text_err(format!("bad meta: {e}")),
        };
        done(self.lib.save_theme(&a.id, &a.template, &a.css, &meta, Author::Agent))
    }

    #[tool(description = "Set the theme a CV uses.")]
    pub async fn set_cv_theme(&self, Parameters(a): Parameters<SetThemeArgs>) -> CallToolResult {
        done(self.lib.set_cv_theme(&a.name, &a.theme, Author::Agent))
    }

    #[tool(description = "Render the first page of a CV as a PNG image. Needs Chrome or Chromium installed.")]
    pub async fn preview(&self, Parameters(a): Parameters<PreviewArgs>) -> CallToolResult {
        let run = async {
            let html = self.lib.render_cv(&a.name, a.theme.as_deref(), None)?;
            let theme = self.lib.effective_theme(&a.name, a.theme.as_deref())?;
            let size = self.lib.get_theme(&theme)?.info.meta.page.size;
            html_to_png(&html, size).await
        };
        match run.await {
            Ok(png) => CallToolResult::success(vec![ContentBlock::image(STANDARD.encode(png), "image/png")]),
            Err(e) => core_err(e),
        }
    }

    #[tool(description = "Export a CV to the library's exports folder. format: pdf, html or json. Returns {path}.")]
    pub async fn export_cv(&self, Parameters(a): Parameters<ExportArgs>) -> CallToolResult {
        let format: ExportFormat = match serde_json::from_value(Value::String(a.format.clone())) {
            Ok(f) => f,
            Err(_) => return text_err(format!("bad format: {} (use pdf, html or json)", a.format)),
        };
        done(
            self.lib
                .export(&a.name, format, a.theme.as_deref())
                .await
                .map(|p| json!({ "path": p.to_string_lossy() })),
        )
    }

    #[tool(description = "Add an image or file to the library's assets folder. Give exactly one of `path` (read from disk) or `base64`. Returns {path} to use in the CV, for example basics.image.")]
    pub async fn add_asset(&self, Parameters(a): Parameters<AddAssetArgs>) -> CallToolResult {
        let bytes = match (&a.path, &a.base64) {
            (Some(p), None) => match std::fs::read(p) {
                Ok(b) => b,
                Err(e) => return text_err(format!("cannot read {p}: {e}")),
            },
            (None, Some(b)) => match STANDARD.decode(b) {
                Ok(b) => b,
                Err(e) => return text_err(format!("bad base64: {e}")),
            },
            _ => return text_err("give exactly one of `path` or `base64`".into()),
        };
        done(self.lib.add_asset(&a.name, &bytes, Author::Agent).map(|p| json!({ "path": p })))
    }
}

#[tool_handler]
impl ServerHandler for JsoncvServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_resources().build())
            .with_instructions(INSTRUCTIONS)
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let mut r = ListResourcesResult::default();
        r.resources = RESOURCES
            .iter()
            .map(|(uri, name, mime, _)| Resource::new(*uri, *name).with_mime_type(*mime))
            .collect();
        Ok(r)
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        let Some((uri, _, mime, body)) = RESOURCES.iter().find(|r| r.0 == request.uri) else {
            return Err(ErrorData::resource_not_found(format!("unknown resource {}", request.uri), None));
        };
        let contents = ResourceContents::TextResourceContents {
            uri: uri.to_string(),
            mime_type: Some(mime.to_string()),
            text: body.to_string(),
            meta: None,
        };
        Ok(ReadResourceResult::new(vec![contents]).into())
    }
}

pub async fn serve_stdio(lib: Library) -> anyhow::Result<()> {
    JsoncvServer::new(lib).serve(stdio()).await?.waiting().await?;
    Ok(())
}
