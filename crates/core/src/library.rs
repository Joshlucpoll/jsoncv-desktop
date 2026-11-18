use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::history::{commit, with_lock};
use crate::{schema, Author, CoreError};

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

#[derive(Debug, Clone, Serialize)]
pub struct Saved {
    pub commit: String,
    pub base: String,
}

#[derive(Debug, Clone)]
pub struct CvDoc {
    pub name: String,
    pub cv: Value,
    pub base: String,
}

pub(crate) fn sha_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn valid_slug(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=64).contains(&b.len())
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
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
        let has = |pat: &str| {
            cur.split(|b| *b == b'\n')
                .any(|l| l.strip_suffix(b"\r").unwrap_or(l) == pat.as_bytes())
        };
        let mut add = String::new();
        if !cur.is_empty() && !cur.ends_with(b"\n") {
            add.push('\n');
        }
        for pat in ["exports/", ".*.tmp-*"].into_iter().filter(|p| !has(p)) {
            add += &format!("{pat}\n");
        }
        if add.trim() != "" {
            // Append only; existing bytes are never rewritten.
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&gi)?;
            std::io::Write::write_all(&mut f, add.as_bytes())?;
        }
        let fresh = !root.join(".git").exists();
        if fresh {
            git2::Repository::init(root)?;
        }
        let lib = Library {
            root: root.to_path_buf(),
        };
        if fresh {
            // shortcut: stages only main + .gitignore; other pre-existing files are left to the sweep.
            let mut paths = vec![Path::new(".gitignore")];
            if crate::history::cv_valid(&main) {
                paths.push(Path::new("main.cv.json"));
            }
            crate::history::with_lock(root, || {
                crate::history::commit(root, &paths, Author::You, "Create library")
            })?;
        }
        Ok(lib)
    }

    /// Startup open of the configured library. A missing non-default folder (an unplugged drive,
    /// a moved folder) is an error rather than silently recreated empty.
    pub fn open_configured(configured: &Path, default: &Path) -> Result<Library, CoreError> {
        if configured != default && !configured.exists() {
            return Err(CoreError::NotFound(format!(
                "library folder {}",
                configured.display()
            )));
        }
        Library::open(configured)
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
        Ok(CvDoc {
            name: name.to_string(),
            cv: serde_json::from_slice(&bytes)?,
            base: sha_hex(&bytes),
        })
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
                name: name.clone(),
                theme: None,
                tailored_from: None,
                modified_ms: 0,
                error: Some(error.into()),
            };
            let Ok(path) = self.cv_path(&name) else {
                out.push(bad(
                    "Invalid file name: use lowercase letters, digits and dashes",
                ));
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
            let mut info = CvInfo {
                name: name.clone(),
                theme: None,
                tailored_from: None,
                modified_ms,
                error: None,
            };
            match self.read_cv(&name) {
                Err(e) => info.error = Some(e.to_string()),
                Ok(doc) => {
                    if let Err(errs) = schema::validate(&doc.cv) {
                        info.error = Some(
                            errs.iter()
                                .map(|e| format!("{}: {}", e.path, e.message))
                                .collect::<Vec<_>>()
                                .join("; "),
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

    /// Validate, write atomically, commit. Caller holds the lock and has swept.
    fn write_locked(
        &self,
        name: &str,
        cv: &Value,
        author: Author,
        message: &str,
    ) -> Result<Saved, CoreError> {
        let path = self.cv_path(name)?;
        schema::validate(cv).map_err(CoreError::Invalid)?;
        let bytes = (serde_json::to_string_pretty(cv)? + "\n").into_bytes();
        let tmp = path.with_file_name(format!(
            ".{}.tmp-{}",
            path.file_name().unwrap().to_string_lossy(),
            std::process::id()
        ));
        let res = (|| -> std::io::Result<()> {
            let mut f = std::fs::File::create(&tmp)?;
            std::io::Write::write_all(&mut f, &bytes)?;
            f.sync_all()?;
            std::fs::rename(&tmp, &path)
        })();
        if let Err(e) = res {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
        let rel = format!("{name}.cv.json");
        let commit = commit(&self.root, &[Path::new(&rel)], author, message)?;
        Ok(Saved {
            commit,
            base: sha_hex(&bytes),
        })
    }

    fn write_validated(
        &self,
        name: &str,
        cv: &Value,
        author: Author,
        message: &str,
    ) -> Result<Saved, CoreError> {
        with_lock(&self.root, || {
            self.sweep_external()?;
            self.write_locked(name, cv, author, message)
        })
    }

    pub fn patch_cv(
        &self,
        name: &str,
        patch: &json_patch::Patch,
        author: Author,
        message: Option<&str>,
    ) -> Result<Saved, CoreError> {
        with_lock(&self.root, || {
            self.sweep_external()?;
            let mut cv = self.read_cv(name)?.cv;
            json_patch::patch(&mut cv, patch).map_err(|e| CoreError::PatchFailed(e.to_string()))?;
            let msg = message
                .map(String::from)
                .unwrap_or_else(|| default_message(patch));
            self.write_locked(name, &cv, author, &msg)
        })
    }

    pub fn replace_cv(
        &self,
        name: &str,
        cv: Value,
        base: &str,
        author: Author,
        message: Option<&str>,
    ) -> Result<Saved, CoreError> {
        with_lock(&self.root, || {
            self.sweep_external()?;
            if self.read_cv(name)?.base != base {
                return Err(CoreError::Stale);
            }
            self.write_locked(name, &cv, author, message.unwrap_or("Replaced CV"))
        })
    }

    pub fn save_draft(
        &self,
        name: &str,
        base_cv: &Value,
        draft: &Value,
    ) -> Result<Saved, CoreError> {
        let ops = serde_json::to_value(json_patch::diff(base_cv, draft))?;
        let ops = ops.as_array().cloned().unwrap_or_default();
        // Each op gets a test just before it, valued from base_cv with earlier ops applied
        // (json-patch shifts array indices, so a single up-front guard would miss elements).
        // replace/remove test the target; a nested add tests its parent. A root-level add has no
        // parent to test, so the key must still be absent in the live doc (checked under the lock).
        let mut work = base_cv.clone();
        let mut all: Vec<Value> = vec![];
        let mut root_adds: Vec<String> = vec![];
        for o in ops {
            let path = o["path"].as_str().unwrap_or("").to_string();
            let guard = match o["op"].as_str() {
                Some("replace" | "remove") => Some(path.as_str()),
                Some("add") => match path.rsplit_once('/') {
                    Some(("", _)) => {
                        root_adds.push(path.clone());
                        None
                    }
                    Some((parent, _)) => Some(parent),
                    None => None,
                },
                _ => None,
            };
            if let Some(g) = guard {
                let v = work
                    .pointer(g)
                    .ok_or_else(|| CoreError::PatchFailed(format!("no value at {g}")))?;
                all.push(serde_json::json!({"op":"test","path":g,"value":v}));
            }
            let one: json_patch::Patch = serde_json::from_value(Value::Array(vec![o.clone()]))?;
            json_patch::patch(&mut work, &one)
                .map_err(|e| CoreError::PatchFailed(e.to_string()))?;
            all.push(o);
        }
        let patch: json_patch::Patch = serde_json::from_value(Value::Array(all))?;
        with_lock(&self.root, || {
            self.sweep_external()?;
            let mut cv = self.read_cv(name)?.cv;
            if let Some(p) = root_adds.iter().find(|p| cv.pointer(p).is_some()) {
                return Err(CoreError::PatchFailed(format!(
                    "{p} was added by someone else"
                )));
            }
            json_patch::patch(&mut cv, &patch)
                .map_err(|e| CoreError::PatchFailed(e.to_string()))?;
            self.write_locked(name, &cv, Author::You, &default_message(&patch))
        })
    }

    pub fn restore(&self, name: &str, commit: &str, author: Author) -> Result<Saved, CoreError> {
        let cv = self.read_cv_at(name, commit)?;
        let id = git2::Repository::open(&self.root)?
            .revparse_single(commit)?
            .peel_to_commit()?
            .id()
            .to_string();
        self.write_validated(name, &cv, author, &format!("Restored version {}", &id[..7]))
    }

    pub fn create_tailored(
        &self,
        slug: &str,
        from: &str,
        author: Author,
    ) -> Result<CvInfo, CoreError> {
        let name = format!("tailored/{slug}");
        let target = self.cv_path(&name)?;
        with_lock(&self.root, || {
            self.sweep_external()?;
            if target.exists() {
                return Err(CoreError::BadName(format!("{name} already exists")));
            }
            let mut cv = self.read_cv(from)?.cv;
            let head = git2::Repository::open(&self.root)?
                .head()?
                .peel_to_commit()?
                .id()
                .to_string();
            cv.as_object_mut()
                .ok_or_else(|| CoreError::PatchFailed("CV is not an object".into()))?
                .entry("meta")
                .or_insert_with(|| serde_json::json!({}))
                .as_object_mut()
                .ok_or_else(|| CoreError::PatchFailed("meta is not an object".into()))?
                .insert(
                    "tailoredFrom".into(),
                    serde_json::json!({"cv": from, "commit": head}),
                );
            self.write_locked(&name, &cv, author, &format!("Created {name} from {from}"))
        })?;
        self.info(&name)
    }

    fn info(&self, name: &str) -> Result<CvInfo, CoreError> {
        self.list_cvs()?
            .into_iter()
            .find(|i| i.name == name)
            .ok_or_else(|| CoreError::NotFound(name.into()))
    }

    pub fn rename_cv(
        &self,
        name: &str,
        new_slug: &str,
        author: Author,
    ) -> Result<CvInfo, CoreError> {
        let old = self.cv_path(name)?;
        if name == "main" {
            return Err(CoreError::ReadOnly("main cannot be renamed".into()));
        }
        let new_name = format!("tailored/{new_slug}");
        let new = self.cv_path(&new_name)?;
        with_lock(&self.root, || {
            self.sweep_external()?;
            if !old.exists() {
                return Err(CoreError::NotFound(name.into()));
            }
            if new.exists() {
                return Err(CoreError::BadName(format!("{new_name} already exists")));
            }
            std::fs::rename(&old, &new)?;
            let (a, b) = (format!("{name}.cv.json"), format!("{new_name}.cv.json"));
            commit(
                &self.root,
                &[Path::new(&a), Path::new(&b)],
                author,
                &format!("Renamed {name} to {new_name}"),
            )?;
            Ok(())
        })?;
        self.info(&new_name)
    }

    pub fn delete_cv(&self, name: &str, author: Author) -> Result<(), CoreError> {
        let path = self.cv_path(name)?;
        if name == "main" {
            return Err(CoreError::ReadOnly("main cannot be deleted".into()));
        }
        with_lock(&self.root, || {
            self.sweep_external()?;
            std::fs::remove_file(&path).map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => CoreError::NotFound(name.into()),
                _ => e.into(),
            })?;
            let rel = format!("{name}.cv.json");
            commit(
                &self.root,
                &[Path::new(&rel)],
                author,
                &format!("Deleted {name}"),
            )?;
            Ok(())
        })
    }

    pub fn set_cv_theme(
        &self,
        name: &str,
        theme_id: &str,
        author: Author,
    ) -> Result<Saved, CoreError> {
        with_lock(&self.root, || {
            self.sweep_external()?;
            let mut cv = self.read_cv(name)?.cv;
            self.get_theme(theme_id)?;
            let obj = cv
                .as_object_mut()
                .ok_or_else(|| CoreError::PatchFailed("CV is not an object".into()))?;
            obj.entry("meta")
                .or_insert_with(|| serde_json::json!({}))
                .as_object_mut()
                .ok_or_else(|| CoreError::PatchFailed("meta is not an object".into()))?
                .insert("theme".into(), theme_id.into());
            self.write_locked(name, &cv, author, &format!("Theme set to {theme_id}"))
        })
    }
}

fn default_message(patch: &json_patch::Patch) -> String {
    let mut keys: Vec<String> = vec![];
    for op in serde_json::to_value(patch)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
    {
        let path = op["path"].as_str().unwrap_or("");
        let k = path
            .trim_start_matches('/')
            .split('/')
            .next()
            .filter(|k| !k.is_empty())
            .unwrap_or("cv");
        let k = k.replace("~1", "/").replace("~0", "~");
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    format!("Edited {}", keys.join(", "))
}
