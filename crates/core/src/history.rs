use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;

use crate::{library::Library, schema, Author, CoreError};

/// Exclusive lock on `.git/jsoncv.lock`; retries every 50 ms for up to 2 s.
/// Public (not `pub(crate)`) so integration tests can reach it.
pub fn with_lock<T>(root: &Path, f: impl FnOnce() -> Result<T, CoreError>) -> Result<T, CoreError> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(root.join(".git/jsoncv.lock"))?;
    let mut lock = fd_lock::RwLock::new(file);
    let start = Instant::now();
    loop {
        match lock.try_write() {
            Ok(_guard) => return f(),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if start.elapsed() >= Duration::from_secs(2) {
                    return Err(CoreError::Locked);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(e.into()),
        }
    }
}

/// Stage `paths` (removal if gone) and commit. Returns "" when nothing changed.
/// Public so integration tests can reach it.
pub fn commit(root: &Path, paths: &[&Path], author: Author, message: &str) -> Result<String, CoreError> {
    let repo = git2::Repository::open(root)?;
    let mut idx = repo.index()?;
    for p in paths {
        let rel = p.strip_prefix(root).unwrap_or(p);
        if root.join(rel).exists() {
            idx.add_path(rel)?;
        } else if idx.get_path(rel, 0).is_some() {
            idx.remove_path(rel)?;
        }
    }
    idx.write()?;
    let tree_id = idx.write_tree()?;
    let parent = repo.head().ok().map(|h| h.peel_to_commit()).transpose()?;
    if parent.as_ref().is_some_and(|p| p.tree_id() == tree_id) {
        return Ok(String::new());
    }
    let sig = git2::Signature::now(author.name(), "jsoncv@localhost")?;
    let tree = repo.find_tree(tree_id)?;
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    Ok(repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct Version {
    pub commit: String,
    pub author: String,
    pub message: String,
    pub time_ms: i64,
}

fn managed(p: &str) -> bool {
    let cv = |s: &str| s.ends_with(".cv.json") && !s.contains('/');
    p == ".gitignore"
        || p.starts_with("themes/")
        || p.starts_with("assets/")
        || cv(p)
        || p.strip_prefix("tailored/").is_some_and(cv)
}

fn canonical(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<_> = m.keys().collect();
            keys.sort();
            Value::Object(keys.into_iter().map(|k| (k.clone(), canonical(&m[k]))).collect())
        }
        Value::Array(a) => Value::Array(a.iter().map(canonical).collect()),
        _ => v.clone(),
    }
}

fn pretty(v: &Value) -> Result<String, CoreError> {
    Ok(serde_json::to_string_pretty(&canonical(v))? + "\n")
}

impl Library {
    /// Commit external changes to managed paths as `External`. Call inside `with_lock`.
    pub fn sweep_external(&self) -> Result<Option<String>, CoreError> {
        let repo = git2::Repository::open(self.root())?;
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true).recurse_untracked_dirs(true);
        let mut paths = vec![];
        for e in repo.statuses(Some(&mut opts))?.iter() {
            let Ok(p) = e.path() else { continue };
            if !managed(p) {
                continue;
            }
            let full = self.root().join(p);
            if p.ends_with(".cv.json") && full.exists() {
                let ok = std::fs::read(&full)
                    .ok()
                    .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                    .is_some_and(|v| schema::validate(&v).is_ok());
                if !ok {
                    continue;
                }
            }
            paths.push(std::path::PathBuf::from(p));
        }
        if paths.is_empty() {
            return Ok(None);
        }
        let refs: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();
        let sha = commit(self.root(), &refs, Author::External, "External changes")?;
        Ok((!sha.is_empty()).then_some(sha))
    }

    pub fn sweep_external_locked(&self) -> Result<Option<String>, CoreError> {
        with_lock(self.root(), || self.sweep_external())
    }

    pub fn history(&self, name: &str, limit: usize) -> Result<Vec<Version>, CoreError> {
        let rel = format!("{name}.cv.json");
        self.cv_path(name)?;
        let repo = git2::Repository::open(self.root())?;
        let Ok(head) = repo.head() else { return Ok(vec![]) };
        let blob = |c: &git2::Commit| c.tree().ok().and_then(|t| t.get_path(Path::new(&rel)).ok()).map(|e| e.id());
        let mut walk = repo.revwalk()?;
        walk.push(head.peel_to_commit()?.id())?;
        let mut out = vec![];
        for id in walk {
            if out.len() >= limit {
                break;
            }
            let c = repo.find_commit(id?)?;
            let prev = c.parents().next().and_then(|p| blob(&p));
            if blob(&c) != prev {
                out.push(Version {
                    commit: c.id().to_string(),
                    author: c.author().name().unwrap_or("").to_string(),
                    message: c.message().unwrap_or("").trim_end().to_string(),
                    time_ms: c.time().seconds() * 1000,
                });
            }
        }
        Ok(out)
    }

    pub fn read_cv_at(&self, name: &str, commit: &str) -> Result<Value, CoreError> {
        self.cv_path(name)?;
        let repo = git2::Repository::open(self.root())?;
        let tree = repo.revparse_single(commit)?.peel_to_commit()?.tree()?;
        let entry = tree
            .get_path(Path::new(&format!("{name}.cv.json")))
            .map_err(|_| CoreError::NotFound(format!("{name} at {commit}")))?;
        let blob = repo.find_blob(entry.id())?;
        let v = serde_json::from_slice(blob.content())?;
        Ok(v)
    }

    pub fn diff(&self, name: &str, commit: &str) -> Result<String, CoreError> {
        let old = pretty(&self.read_cv_at(name, commit)?)?;
        let new = pretty(&self.read_cv(name)?.cv)?;
        let short = &commit[..commit.len().min(7)];
        let d = similar::TextDiff::from_lines(&old, &new).unified_diff().header(short, "current").to_string();
        Ok(d)
    }
}
