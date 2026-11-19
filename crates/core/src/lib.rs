pub mod assets;
pub mod config;
pub mod coverage;
pub mod error;
pub mod export;
pub mod history;
pub mod library;
mod migrate;
pub mod render;
pub mod schema;

pub use assets::MAX_ASSET_BYTES;
pub use error::{Author, CoreError};

/// A fictional example CV, used by the app for "new CV" and previews.
pub const SAMPLE_CV: &str = include_str!("../resources/sample.cv.json");
/// Authoring guide for custom themes.
pub const THEME_GUIDE: &str = include_str!("../resources/theme-guide.md");
