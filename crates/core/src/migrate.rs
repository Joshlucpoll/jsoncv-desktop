//! Moves libraries from the old layout (`main.cv.json` at the root, copies in
//! `tailored/<slug>.cv.json`) to `cvs/`. Runs on every open: a second Mac sharing the folder, or an
//! agent still running the old code, can bring back old files or old history names at any time.
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::{library::Library, CoreError};

/// `slug`, or the first of `slug-2`, `slug-3`, … not in `taken`.
fn free(slug: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(slug) {
        return slug.to_string();
    }
    (2..)
        .map(|n| format!("{slug}-{n}"))
        .find(|s| !taken.contains(s))
        .unwrap()
}

pub(crate) fn migrate(lib: &Library) -> Result<(), CoreError> {
    let root = lib.root();
    let tailored = root.join("tailored");
    // (old name, slug, file)
    let mut old: Vec<(String, String, PathBuf)> = vec![];
    if root.join("main.cv.json").is_file() {
        old.push(("main".into(), "main".into(), root.join("main.cv.json")));
    }
    if let Ok(rd) = std::fs::read_dir(&tailored) {
        let mut t: Vec<_> = rd
            .filter_map(Result::ok)
            .filter_map(|e| {
                let f = e.file_name().to_string_lossy().into_owned();
                let slug = f.strip_suffix(".cv.json")?.to_string();
                Some((format!("tailored/{slug}"), slug, e.path()))
            })
            .collect();
        t.sort();
        old.extend(t);
    }
    lib.locked(|tx| {
        let mut taken: HashSet<String> = lib.cv_entries()?.into_iter().map(|e| e.0).collect();
        let mut moved: HashMap<String, String> = HashMap::new();
        for (name, slug, path) in &old {
            let new = free(slug, &taken);
            std::fs::rename(path, lib.cvs_dir().join(format!("{new}.cv.json")))?;
            taken.insert(new.clone());
            moved.insert(name.clone(), new);
        }
        // shortcut: a re-created root main keeps sharing main's rows (old code named it main), fine while old processes are rare
        let all: Vec<String> = tx
            .prepare("SELECT DISTINCT cv FROM versions WHERE library = ?1")?
            .query_map([lib.id()], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        let mut named: HashSet<String> = all.iter().cloned().collect();
        for n in all.iter().filter(|n| n.starts_with("tailored/")) {
            let target = moved
                .get(n)
                .cloned()
                .unwrap_or_else(|| free(&n["tailored/".len()..], &named));
            tx.execute(
                "UPDATE versions SET cv = ?1 WHERE library = ?2 AND cv = ?3",
                rusqlite::params![target, lib.id(), n],
            )?;
            named.insert(target);
        }
        Ok(())
    })?;
    if tailored.is_dir() {
        let _ = std::fs::remove_file(tailored.join(".DS_Store"));
        let _ = std::fs::remove_dir(&tailored); // only succeeds when empty
    }
    Ok(())
}
