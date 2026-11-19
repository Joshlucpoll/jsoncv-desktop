use std::{
    io,
    path::{Path, PathBuf},
};

use directories::{BaseDirs, ProjectDirs};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub library: PathBuf,
    /// History database file; `None` means [`default_history`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<PathBuf>,
}

impl Config {
    pub fn history_path(&self) -> PathBuf {
        self.history.clone().unwrap_or_else(default_history)
    }
}

fn dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("dev", "jsoncv", "jsoncv-desktop")
}

fn config_file() -> Option<PathBuf> {
    dirs().map(|d| d.config_dir().join("config.json"))
}

/// `history.db` in the app's data folder (`~/Library/Application Support/dev.jsoncv.jsoncv-desktop`).
pub fn default_history() -> PathBuf {
    dirs()
        .map(|d| d.data_dir().join("history.db"))
        .unwrap_or_else(|| PathBuf::from("history.db"))
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
            history: None,
        })
}

pub fn save(cfg: &Config) -> io::Result<()> {
    let f = config_file().ok_or_else(|| io::Error::other("no config dir"))?;
    std::fs::create_dir_all(f.parent().unwrap())?;
    let tmp = f.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(cfg)?)?;
    std::fs::rename(tmp, f)
}

/// True when `p` sits in a cloud-sync folder, where a database written from two Macs can corrupt.
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
