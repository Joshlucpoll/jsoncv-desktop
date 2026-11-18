use crate::schema::SchemaError;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("CV failed schema validation ({} errors)", .0.len())]
    Invalid(Vec<SchemaError>),
    #[error("file changed on disk since it was read")]
    Stale,
    #[error("patch failed: {0}")]
    PatchFailed(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("bad name: {0}")]
    BadName(String),
    #[error("bad path: {0}")]
    BadPath(String),
    #[error("library is locked by another process")]
    Locked,
    #[error("read-only: {0}")]
    ReadOnly(String),
    #[error("browser error: {0}")]
    Browser(String),
    #[error("render error: {0}")]
    Render(crate::render::RenderError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Git(#[from] git2::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Author {
    You,
    Agent,
    External,
}

impl Author {
    pub fn name(&self) -> &'static str {
        match self {
            Author::You => "You",
            Author::Agent => "Agent",
            Author::External => "External",
        }
    }
}
