use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{schema, CoreError};

const SAMPLE: &str = include_str!("../resources/sample.cv.json");

#[derive(Debug, Clone)]
pub struct Library {
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct CvInfo {
    pub name: String,
    pub theme: Option<String>,
    pub tailored_from: Option<String>,
    pub modified_ms: i64,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CvDoc {
    pub name: String,
    pub cv: Value,
    pub base: String,
}

pub(crate) fn sha_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn valid_slug(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=64).contains(&b.len())
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

impl Library {
    pub fn open(root: &Path) -> Result<Library, CoreError> {
        std::fs::create_dir_all(root)?;
        for d in ["tailored", "themes", "assets", "exports"] {
            std::fs::create_dir_all(root.join(d))?;
        }
        let main = root.join("main.cv.json");
        if !main.exists() {
            std::fs::write(&main, SAMPLE)?;
        }
        let gi = root.join(".gitignore");
        let cur = match std::fs::read(&gi) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(e) => return Err(e.into()),
        };
        if !cur.split(|b| *b == b'\n').any(|l| l == b"exports/") {
            let sep = if cur.is_empty() || cur.ends_with(b"\n") { "" } else { "\n" };
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&gi)?;
            std::io::Write::write_all(&mut f, format!("{sep}exports/\n").as_bytes())?;
        }
        if !root.join(".git").exists() {
            git2::Repository::init(root)?;
        }
        Ok(Library { root: root.to_path_buf() })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn cv_path(&self, name: &str) -> Result<PathBuf, CoreError> {
        let ok = name == "main" || name.strip_prefix("tailored/").is_some_and(valid_slug);
        if !ok {
            return Err(CoreError::BadName(name.to_string()));
        }
        Ok(self.root.join(format!("{name}.cv.json")))
    }

    pub fn read_cv(&self, name: &str) -> Result<CvDoc, CoreError> {
        let path = self.cv_path(name)?;
        let bytes = std::fs::read(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => CoreError::NotFound(name.to_string()),
            _ => e.into(),
        })?;
        Ok(CvDoc { name: name.to_string(), cv: serde_json::from_slice(&bytes)?, base: sha_hex(&bytes) })
    }

    pub fn list_cvs(&self) -> Result<Vec<CvInfo>, CoreError> {
        let mut names = vec!["main".to_string()];
        let mut tailored = vec![];
        for e in std::fs::read_dir(self.root.join("tailored"))? {
            let f = e?.file_name().to_string_lossy().into_owned();
            if let Some(slug) = f.strip_suffix(".cv.json") {
                tailored.push(format!("tailored/{slug}"));
            }
        }
        tailored.sort();
        names.extend(tailored);
        let mut out = vec![];
        for name in names {
            let bad = |error: &str| CvInfo {
                name: name.clone(), theme: None, tailored_from: None, modified_ms: 0, error: Some(error.into()),
            };
            let Ok(path) = self.cv_path(&name) else {
                out.push(bad("Invalid file name: use lowercase letters, digits and dashes"));
                continue;
            };
            let Ok(meta) = std::fs::metadata(&path) else {
                out.push(bad("main.cv.json is missing"));
                continue;
            };
            let modified_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_millis() as i64);
            let mut info = CvInfo { name: name.clone(), theme: None, tailored_from: None, modified_ms, error: None };
            match self.read_cv(&name) {
                Err(e) => info.error = Some(e.to_string()),
                Ok(doc) => {
                    if let Err(errs) = schema::validate(&doc.cv) {
                        info.error = Some(
                            errs.iter().map(|e| format!("{}: {}", e.path, e.message)).collect::<Vec<_>>().join("; "),
                        );
                    }
                    let meta = &doc.cv["meta"];
                    info.theme = meta["theme"].as_str().map(String::from);
                    info.tailored_from = meta["tailoredFrom"]["cv"].as_str().map(String::from);
                }
            }
            out.push(info);
        }
        Ok(out)
    }
}
