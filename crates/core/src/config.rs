use std::{
    io,
    path::{Path, PathBuf},
};

use directories::{BaseDirs, ProjectDirs};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub library: PathBuf,
}

fn config_file() -> Option<PathBuf> {
    ProjectDirs::from("dev", "jsoncv", "jsoncv-desktop").map(|d| d.config_dir().join("config.json"))
}

pub fn default_library() -> PathBuf {
    BaseDirs::new()
        .map(|d| d.home_dir().join("Documents/CVs"))
        .unwrap_or_else(|| PathBuf::from("CVs"))
}

pub fn load() -> Config {
    config_file()
        .and_then(|f| std::fs::read(f).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| Config {
            library: default_library(),
        })
}

pub fn save(cfg: &Config) -> io::Result<()> {
    let f = config_file().ok_or_else(|| io::Error::other("no config dir"))?;
    std::fs::create_dir_all(f.parent().unwrap())?;
    let tmp = f.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(cfg)?)?;
    std::fs::rename(tmp, f)
}

/// True when `p` sits in a cloud-sync folder, where Git repos can corrupt.
pub fn is_in_sync_folder(p: &Path) -> bool {
    let Some(home) = BaseDirs::new().map(|d| d.home_dir().to_path_buf()) else {
        return false;
    };
    [
        "Library/Mobile Documents",
        "Library/CloudStorage",
        "Dropbox",
        "Google Drive",
        "OneDrive",
    ]
    .iter()
    .any(|d| p.starts_with(home.join(d)))
}
