use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{schema, Author, CoreError};

const PINS: &str = ".jsoncv-pins.json";
const ORDER: &str = ".jsoncv-order.json";
const SAMPLE: &str = include_str!("../resources/sample.cv.json");

#[derive(Debug, Clone)]
pub struct Library {
    root: PathBuf,
    db: PathBuf,
    /// Contents of `.jsoncv-id`; keys this library's rows in the history database.
    id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CvInfo {
    pub name: String,
    pub theme: Option<String>,
    /// The subfolder of `cvs/` the file is in; None at the top level.
    pub folder: Option<String>,
    /// Position in `.jsoncv-pins.json`; None when not pinned.
    pub pin: Option<usize>,
    /// `meta.pageMode == "continuous"`: one long page instead of paper-sized pages.
    pub continuous: bool,
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
    /// Open (creating if needed) the CV folder `root`, with its history kept in the database `db`.
    pub fn open(root: &Path, db: &Path) -> Result<Library, CoreError> {
        std::fs::create_dir_all(root)?;
        for d in ["cvs", "themes", "assets", "exports"] {
            std::fs::create_dir_all(root.join(d))?;
        }
        let fresh = !root.join(".jsoncv-id").exists();
        let id = library_id(root)?;
        crate::history::init(db)?;
        let lib = Library {
            root: root.to_path_buf(),
            db: db.to_path_buf(),
            id,
        };
        crate::migrate::migrate(&lib)?;
        // Only a brand-new library gets the sample; an emptied one stays empty.
        if fresh && lib.cv_entries()?.is_empty() {
            std::fs::write(lib.cvs_dir().join("my-cv.cv.json"), SAMPLE)?;
        }
        lib.locked(|tx| {
            if lib.cv_head_any(tx)?.is_none() {
                lib.sweep(tx, Author::You, "Create library")?;
            }
            Ok(())
        })?;
        Ok(lib)
    }

    /// Startup open of the configured library. A missing non-default folder (an unplugged drive,
    /// a moved folder) is an error rather than silently recreated empty.
    pub fn open_configured(
        configured: &Path,
        default: &Path,
        db: &Path,
    ) -> Result<Library, CoreError> {
        if configured != default && !configured.exists() {
            return Err(CoreError::NotFound(format!(
                "library folder {}",
                configured.display()
            )));
        }
        Library::open(configured, db)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn db(&self) -> &Path {
        &self.db
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    /// Any version of this library, used to spot a library the database has never seen.
    fn cv_head_any(&self, c: &rusqlite::Connection) -> Result<Option<i64>, CoreError> {
        Ok(c.query_row(
            "SELECT MAX(id) FROM versions WHERE library = ?1",
            [self.id()],
            |r| r.get(0),
        )?)
    }

    pub(crate) fn cvs_dir(&self) -> PathBuf {
        self.root.join("cvs")
    }

    /// Every `*.cv.json` in `cvs/` and its first-level subfolders, as (slug, folder, path),
    /// including slugs that fail the name rule.
    pub(crate) fn cv_entries(&self) -> Result<Vec<(String, Option<String>, PathBuf)>, CoreError> {
        let mut out = vec![];
        let mut scan = |dir: &Path, folder: Option<String>| -> Result<(), CoreError> {
            let rd = match std::fs::read_dir(dir) {
                Ok(rd) => rd,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(e) => return Err(e.into()),
            };
            for e in rd {
                let e = e?;
                let f = e.file_name().to_string_lossy().into_owned();
                if let Some(slug) = f.strip_suffix(".cv.json") {
                    if e.file_type()?.is_file() {
                        out.push((slug.to_string(), folder.clone(), e.path()));
                    }
                }
            }
            Ok(())
        };
        scan(&self.cvs_dir(), None)?;
        for f in self.list_folders()? {
            scan(&self.cvs_dir().join(&f), Some(f))?;
        }
        out.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        Ok(out)
    }

    /// Subfolders of `cvs/`, sorted, empty ones included.
    pub fn list_folders(&self) -> Result<Vec<String>, CoreError> {
        let mut out = vec![];
        let rd = match std::fs::read_dir(self.cvs_dir()) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(e.into()),
        };
        for e in rd {
            let e = e?;
            let f = e.file_name().to_string_lossy().into_owned();
            if !f.starts_with('.') && e.file_type()?.is_dir() {
                out.push(f);
            }
        }
        out.sort();
        Ok(out)
    }

    /// Distinct CV slugs, sorted, including names that fail `cv_path`.
    pub(crate) fn cv_names(&self) -> Result<Vec<String>, CoreError> {
        let mut names: Vec<String> = self.cv_entries()?.into_iter().map(|e| e.0).collect();
        names.dedup();
        Ok(names)
    }

    /// The file of CV `name`, wherever it sits; `cvs/<name>.cv.json` when there is none yet.
    pub(crate) fn cv_path(&self, name: &str) -> Result<PathBuf, CoreError> {
        if !valid_slug(name) {
            return Err(CoreError::BadName(name.to_string()));
        }
        let hits: Vec<_> = self
            .cv_entries()?
            .into_iter()
            .filter(|e| e.0 == name)
            .collect();
        match hits.as_slice() {
            [] => Ok(self.cvs_dir().join(format!("{name}.cv.json"))),
            [one] => Ok(one.2.clone()),
            [a, b, ..] => {
                let at = |f: &Option<String>| format!("{}/", f.as_deref().unwrap_or("cvs"));
                Err(CoreError::BadName(format!(
                    "{name}.cv.json is in both {} and {}; rename or remove one",
                    at(&a.1),
                    at(&b.1)
                )))
            }
        }
    }

    /// Where a new CV `slug` goes; fails if that slug is already used anywhere.
    pub(crate) fn new_cv_path(
        &self,
        slug: &str,
        folder: Option<&str>,
    ) -> Result<PathBuf, CoreError> {
        if !valid_slug(slug) {
            return Err(CoreError::BadName(slug.to_string()));
        }
        let dir = self.folder_dir(folder)?;
        if self.cv_entries()?.iter().any(|e| e.0 == slug) {
            return Err(CoreError::BadName(format!("{slug} already exists")));
        }
        Ok(dir.join(format!("{slug}.cv.json")))
    }

    fn folder_of(&self, path: &Path) -> Option<String> {
        let dir = path.parent()?;
        (dir != self.cvs_dir()).then(|| dir.file_name().unwrap().to_string_lossy().into_owned())
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
        let pins = self.pins();
        let mut out = vec![];
        for name in self.cv_names()? {
            let bad = |error: &str| CvInfo {
                name: name.clone(),
                theme: None,
                folder: None,
                pin: None,
                continuous: false,
                modified_ms: 0,
                error: Some(error.into()),
            };
            let path = match self.cv_path(&name) {
                Ok(p) => p,
                Err(CoreError::BadName(m)) if valid_slug(&name) => {
                    out.push(bad(&m));
                    continue;
                }
                Err(_) => {
                    out.push(bad(
                        "Invalid file name: use lowercase letters, digits and dashes",
                    ));
                    continue;
                }
            };
            let Ok(meta) = std::fs::metadata(&path) else {
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
                folder: self.folder_of(&path),
                pin: pins.iter().position(|p| *p == name),
                continuous: false,
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
                    info.continuous = meta["pageMode"] == "continuous";
                }
            }
            out.push(info);
        }
        Ok(out)
    }

    /// Validate, write atomically, record a version. Caller holds the lock and has swept.
    fn write_locked(
        &self,
        tx: &rusqlite::Transaction,
        path: &Path,
        name: &str,
        cv: &Value,
        author: Author,
        message: &str,
    ) -> Result<Saved, CoreError> {
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
            std::fs::rename(&tmp, path)
        })();
        if let Err(e) = res {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.into());
        }
        let commit = self.record(tx, name, author, message, Some(&bytes))?;
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
        self.locked(|tx| {
            self.sweep_external(tx)?;
            self.write_locked(tx, &self.cv_path(name)?, name, cv, author, message)
        })
    }

    pub fn patch_cv(
        &self,
        name: &str,
        patch: &json_patch::Patch,
        author: Author,
        message: Option<&str>,
    ) -> Result<Saved, CoreError> {
        self.locked(|tx| {
            self.sweep_external(tx)?;
            let mut cv = self.read_cv(name)?.cv;
            json_patch::patch(&mut cv, patch).map_err(|e| CoreError::PatchFailed(e.to_string()))?;
            let msg = message
                .map(String::from)
                .unwrap_or_else(|| default_message(patch));
            self.write_locked(tx, &self.cv_path(name)?, name, &cv, author, &msg)
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
        self.locked(|tx| {
            self.sweep_external(tx)?;
            if self.read_cv(name)?.base != base {
                return Err(CoreError::Stale);
            }
            self.write_locked(
                tx,
                &self.cv_path(name)?,
                name,
                &cv,
                author,
                message.unwrap_or("Replaced CV"),
            )
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
        self.locked(|tx| {
            self.sweep_external(tx)?;
            let mut cv = self.read_cv(name)?.cv;
            if let Some(p) = root_adds.iter().find(|p| cv.pointer(p).is_some()) {
                return Err(CoreError::PatchFailed(format!(
                    "{p} was added by someone else"
                )));
            }
            json_patch::patch(&mut cv, &patch)
                .map_err(|e| CoreError::PatchFailed(e.to_string()))?;
            self.write_locked(
                tx,
                &self.cv_path(name)?,
                name,
                &cv,
                Author::You,
                &default_message(&patch),
            )
        })
    }

    pub fn restore(&self, name: &str, commit: &str, author: Author) -> Result<Saved, CoreError> {
        let cv = self.read_cv_at(name, commit)?;
        let id = commit.trim_start_matches('v');
        self.write_validated(name, &cv, author, &format!("Restored version v{id}"))
    }

    /// Copies `from` as `slug`, into the same folder. The copy is independent of `from`.
    pub fn duplicate_cv(
        &self,
        slug: &str,
        from: &str,
        author: Author,
    ) -> Result<CvInfo, CoreError> {
        self.locked(|tx| {
            self.sweep_external(tx)?;
            let cv = self.read_cv(from)?.cv;
            let folder = self.folder_of(&self.cv_path(from)?);
            let target = self.new_cv_path(slug, folder.as_deref())?;
            std::fs::create_dir_all(target.parent().unwrap())?;
            self.write_locked(
                tx,
                &target,
                slug,
                &cv,
                author,
                &format!("Duplicated {from} as {slug}"),
            )
        })?;
        self.info(slug)
    }

    /// A new, empty CV (`{}`) named `slug`.
    pub fn create_cv(
        &self,
        slug: &str,
        folder: Option<&str>,
        author: Author,
    ) -> Result<CvInfo, CoreError> {
        self.locked(|tx| {
            self.sweep_external(tx)?;
            let target = self.new_cv_path(slug, folder)?;
            std::fs::create_dir_all(target.parent().unwrap())?;
            self.write_locked(
                tx,
                &target,
                slug,
                &serde_json::json!({}),
                author,
                &format!("Created {slug}"),
            )
        })?;
        self.info(slug)
    }

    /// Moves CV `name` into `folder` (None: the top of `cvs/`). Its name and history are unchanged.
    pub fn move_cv(&self, name: &str, folder: Option<&str>) -> Result<CvInfo, CoreError> {
        let dir = self.folder_dir(folder)?;
        self.locked(|_| {
            let old = self.cv_path(name)?;
            if !old.exists() {
                return Err(CoreError::NotFound(name.into()));
            }
            let new = dir.join(format!("{name}.cv.json"));
            if new == old {
                return Ok(());
            }
            if new.exists() {
                return Err(CoreError::BadName(format!("{name} is already there")));
            }
            std::fs::create_dir_all(&dir)?;
            std::fs::rename(&old, &new)?;
            Ok(())
        })?;
        self.info(name)
    }

    /// The folder `folder` (None: the top of `cvs/`). New folders must follow the slug rule; one
    /// that already exists (perhaps made in Finder) is used whatever its name.
    fn folder_dir(&self, folder: Option<&str>) -> Result<PathBuf, CoreError> {
        match folder {
            None => Ok(self.cvs_dir()),
            Some(f) if valid_slug(f) || self.list_folders()?.iter().any(|x| x == f) => {
                Ok(self.cvs_dir().join(f))
            }
            Some(f) => Err(CoreError::BadName(f.into())),
        }
    }

    pub fn create_folder(&self, name: &str) -> Result<(), CoreError> {
        let dir = self.folder_dir(Some(name))?;
        self.locked(|_| {
            if dir.exists() {
                return Err(CoreError::BadName(format!("Folder {name} already exists")));
            }
            Ok(std::fs::create_dir(&dir)?)
        })
    }

    /// Renames a folder; the CVs inside keep their names and history.
    pub fn rename_folder(&self, name: &str, new_name: &str) -> Result<(), CoreError> {
        let old = self.folder_dir(Some(name))?;
        let new = self.folder_dir(Some(new_name))?;
        self.locked(|_| {
            if !old.is_dir() {
                return Err(CoreError::NotFound(format!("folder {name}")));
            }
            if new.exists() {
                return Err(CoreError::BadName(format!(
                    "Folder {new_name} already exists"
                )));
            }
            std::fs::rename(&old, &new)?;
            self.rename_in_list(ORDER, &format!("{name}/"), Some(&format!("{new_name}/")))
        })
    }

    /// Deletes an empty folder (a Finder `.DS_Store` does not count).
    pub fn delete_folder(&self, name: &str) -> Result<(), CoreError> {
        let dir = self.folder_dir(Some(name))?;
        self.locked(|_| {
            if !dir.is_dir() {
                return Err(CoreError::NotFound(format!("folder {name}")));
            }
            let others = std::fs::read_dir(&dir)?
                .filter_map(Result::ok)
                .any(|e| e.file_name() != ".DS_Store");
            if others {
                return Err(CoreError::BadName(format!("Folder {name} is not empty")));
            }
            let _ = std::fs::remove_file(dir.join(".DS_Store"));
            std::fs::remove_dir(&dir)?;
            self.rename_in_list(ORDER, &format!("{name}/"), None)
        })
    }

    /// A JSON list of strings kept in the library root. A missing or unreadable file means none.
    fn read_list(&self, file: &str) -> Vec<String> {
        std::fs::read(self.root.join(file))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn write_list(&self, file: &str, items: &[String]) -> Result<(), CoreError> {
        let f = self.root.join(file);
        let tmp = f.with_file_name(format!("{file}.tmp-{}", std::process::id()));
        std::fs::write(&tmp, serde_json::to_string(items)? + "\n")?;
        Ok(std::fs::rename(&tmp, &f)?)
    }

    /// Replaces `old` with `new` in a list file (None removes it); a no-op when absent.
    fn rename_in_list(&self, file: &str, old: &str, new: Option<&str>) -> Result<(), CoreError> {
        let items = self.read_list(file);
        if !items.iter().any(|i| i == old) {
            return Ok(());
        }
        let next: Vec<String> = items
            .into_iter()
            .filter_map(|i| {
                if i == old {
                    new.map(String::from)
                } else {
                    Some(i)
                }
            })
            .collect();
        self.write_list(file, &next)
    }

    /// Pinned slugs in pin order.
    fn pins(&self) -> Vec<String> {
        self.read_list(PINS)
    }

    fn write_pins(&self, pins: &[String]) -> Result<(), CoreError> {
        self.write_list(PINS, pins)
    }

    /// The sidebar's custom order: CV slugs and folder names with a trailing `/`, in display order.
    pub fn order(&self) -> Vec<String> {
        self.read_list(ORDER)
    }

    pub fn set_order(&self, keys: &[String]) -> Result<(), CoreError> {
        self.locked(|_| self.write_list(ORDER, keys))
    }

    /// Rewrites the pin order; names that are not pinned, and repeats, are dropped.
    pub fn set_pin_order(&self, names: &[String]) -> Result<(), CoreError> {
        self.locked(|_| {
            let pins = self.pins();
            let mut next: Vec<String> = vec![];
            for n in names {
                if pins.contains(n) && !next.contains(n) {
                    next.push(n.clone());
                }
            }
            // Pins the list left out keep their place at the end, so none is lost.
            next.extend(pins.into_iter().filter(|p| !names.contains(p)));
            self.write_pins(&next)
        })
    }

    /// Pins (at the end of the list) or unpins CV `name`.
    pub fn set_pinned(&self, name: &str, pinned: bool) -> Result<(), CoreError> {
        self.locked(|_| {
            if !self.cv_path(name)?.exists() {
                return Err(CoreError::NotFound(name.into()));
            }
            let mut pins = self.pins();
            pins.retain(|p| p != name);
            if pinned {
                pins.push(name.to_string());
            }
            self.write_pins(&pins)
        })
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
        let new_name = new_slug.to_string();
        self.locked(|tx| {
            self.sweep_external(tx)?;
            if !old.exists() {
                return Err(CoreError::NotFound(name.into()));
            }
            let new = self.new_cv_path(new_slug, self.folder_of(&old).as_deref())?;
            std::fs::rename(&old, &new)?;
            self.rename_in_list(PINS, name, Some(&new_name))?;
            self.rename_in_list(ORDER, name, Some(&new_name))?;
            // History follows the CV to its new name.
            tx.execute(
                "UPDATE versions SET cv = ?1 WHERE library = ?2 AND cv = ?3",
                rusqlite::params![new_name, self.id(), name],
            )?;
            self.insert(
                tx,
                &new_name,
                author,
                &format!("Renamed {name} to {new_name}"),
                Some(&std::fs::read(&new)?),
            )?;
            Ok(())
        })?;
        self.info(&new_name)
    }

    pub fn delete_cv(&self, name: &str, author: Author) -> Result<(), CoreError> {
        let path = self.cv_path(name)?;
        self.locked(|tx| {
            self.sweep_external(tx)?;
            if path.exists() && self.cv_entries()?.len() <= 1 {
                return Err(CoreError::ReadOnly("You can't delete the last CV".into()));
            }
            std::fs::remove_file(&path).map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => CoreError::NotFound(name.into()),
                _ => e.into(),
            })?;
            self.record(tx, name, author, &format!("Deleted {name}"), None)?;
            self.rename_in_list(PINS, name, None)?;
            self.rename_in_list(ORDER, name, None)?;
            Ok(())
        })
    }

    pub fn set_cv_theme(
        &self,
        name: &str,
        theme_id: &str,
        author: Author,
    ) -> Result<Saved, CoreError> {
        self.get_theme(theme_id)?;
        self.set_cv_meta(
            name,
            "theme",
            Some(theme_id.into()),
            author,
            &format!("Theme set to {theme_id}"),
        )
    }

    /// Sets (or with `None` removes) `meta.<key>`, a per-CV tool setting such as `pageMode`.
    pub fn set_cv_meta(
        &self,
        name: &str,
        key: &str,
        value: Option<Value>,
        author: Author,
        message: &str,
    ) -> Result<Saved, CoreError> {
        self.locked(|tx| {
            self.sweep_external(tx)?;
            let mut cv = self.read_cv(name)?.cv;
            let obj = cv
                .as_object_mut()
                .ok_or_else(|| CoreError::PatchFailed("CV is not an object".into()))?;
            let meta = obj
                .entry("meta")
                .or_insert_with(|| serde_json::json!({}))
                .as_object_mut()
                .ok_or_else(|| CoreError::PatchFailed("meta is not an object".into()))?;
            match value {
                Some(v) => meta.insert(key.into(), v),
                None => meta.remove(key),
            };
            self.write_locked(tx, &self.cv_path(name)?, name, &cv, author, message)
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

/// Read `.jsoncv-id` in `root`, creating it with a random id if missing.
fn library_id(root: &Path) -> Result<String, CoreError> {
    let f = root.join(".jsoncv-id");
    match std::fs::read_to_string(&f) {
        Ok(s) if !s.trim().is_empty() => return Ok(s.trim().to_string()),
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let id = sha_hex(format!("{}{nanos}{}", root.display(), std::process::id()).as_bytes())[..32]
        .to_string();
    std::fs::write(&f, format!("{id}\n"))?;
    Ok(id)
}
