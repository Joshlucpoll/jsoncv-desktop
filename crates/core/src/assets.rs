use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use base64::{engine::general_purpose::STANDARD, Engine};
use regex::{Captures, Regex};
use serde_json::Value;

use crate::history::{commit, with_lock};
use crate::library::Library;
use crate::{Author, CoreError};

/// Largest asset `add_asset` accepts.
pub const MAX_ASSET_BYTES: usize = 20 * 1024 * 1024;

pub(crate) fn safe_join(base: &Path, rel: &str) -> Option<PathBuf> {
    let rel = Path::new(rel);
    if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    let p = base.join(rel);
    // Resolve symlinks for existing files so a link cannot point outside base.
    if let (Ok(real), Ok(b)) = (p.canonicalize(), base.canonicalize()) {
        if !real.starts_with(b) {
            return None;
        }
    }
    Some(p)
}

pub(crate) fn data_uri(path: &Path) -> io::Result<String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        _ => "application/octet-stream",
    };
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(std::fs::read(path)?)))
}

/// `s` must start with `assets/`; returns the data URI if it names an existing file under `<dir>/assets`.
fn inline_one(s: &str, dir: &Path) -> Option<String> {
    let rest = s.strip_prefix("assets/")?;
    let p = safe_join(&dir.join("assets"), rest)?;
    if p.is_file() { data_uri(&p).ok() } else { None }
}

pub(crate) fn inline_cv_assets(cv: &mut Value, root: &Path) {
    match cv {
        Value::String(s) => {
            if let Some(u) = inline_one(s, root) {
                *s = u;
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|v| inline_cv_assets(v, root)),
        Value::Object(o) => o.values_mut().for_each(|v| inline_cv_assets(v, root)),
        _ => {}
    }
}

static URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"url\(\s*(?:'(assets/[^']+)'|"(assets/[^"]+)"|(assets/[^)'"\s]+))\s*\)"#).unwrap()
});
static ATTR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\b(src|href)="(assets/[^"]+)""#).unwrap());

pub(crate) fn inline_theme_assets(text: &str, theme_dir: &Path) -> String {
    let out = URL_RE.replace_all(text, |c: &Captures| {
        let p = (1..=3).find_map(|i| c.get(i)).unwrap().as_str();
        match inline_one(p, theme_dir) {
            Some(u) => format!("url(\"{u}\")"),
            None => c[0].to_string(),
        }
    });
    ATTR_RE
        .replace_all(&out, |c: &Captures| match inline_one(&c[2], theme_dir) {
            Some(u) => format!("{}=\"{u}\"", &c[1]),
            None => c[0].to_string(),
        })
        .into_owned()
}

fn sanitise(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let s: String = base.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')).collect();
    s.trim_start_matches('.').to_string()
}

impl Library {
    pub fn add_asset(&self, file_name: &str, bytes: &[u8], author: Author) -> Result<String, CoreError> {
        let name = sanitise(file_name);
        if name.is_empty() {
            return Err(CoreError::BadPath(format!("Invalid asset name: {file_name}")));
        }
        if bytes.len() > MAX_ASSET_BYTES {
            return Err(CoreError::BadPath("Asset too large (max 20 MB)".into()));
        }
        let dir = self.root().join("assets");
        with_lock(self.root(), || {
            self.sweep_external()?;
            let (stem, ext) = match name.rfind('.') {
                Some(i) => (&name[..i], &name[i..]),
                None => (name.as_str(), ""),
            };
            let mut n = 1;
            let (final_name, exists) = loop {
                let cand = if n == 1 { name.clone() } else { format!("{stem}-{n}{ext}") };
                match std::fs::read(dir.join(&cand)) {
                    Ok(old) if old == bytes => break (cand, true),
                    Ok(_) => n += 1,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => break (cand, false),
                    Err(e) => return Err(e.into()),
                }
            };
            if !exists {
                let path = dir.join(&final_name);
                let tmp = dir.join(format!(".{final_name}.tmp-{}", std::process::id()));
                let res = (|| -> io::Result<()> {
                    let mut f = std::fs::File::create(&tmp)?;
                    io::Write::write_all(&mut f, bytes)?;
                    f.sync_all()?;
                    std::fs::rename(&tmp, &path)
                })();
                if let Err(e) = res {
                    let _ = std::fs::remove_file(&tmp);
                    return Err(e.into());
                }
                let rel = format!("assets/{final_name}");
                commit(self.root(), &[Path::new(&rel)], author, &format!("Added asset {final_name}"))?;
            }
            Ok(format!("assets/{final_name}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn safe_join_rejects_escape() {
        let b = Path::new("/tmp/base");
        for r in ["../x", "/etc/passwd", "assets/../../x"] {
            assert!(safe_join(b, r).is_none(), "{r}");
        }
        assert!(safe_join(b, "a/b.png").is_some());
    }

    fn root_with(file: &str, bytes: &[u8]) -> TempDir {
        let d = TempDir::new().unwrap();
        let p = d.path().join(file);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
        d
    }

    #[test]
    fn cv_image_inlined() {
        let d = root_with("assets/p.png", b"\x89PNG");
        let mut cv = json!({"basics": {"image": "assets/p.png"}});
        inline_cv_assets(&mut cv, d.path());
        let s = cv["basics"]["image"].as_str().unwrap();
        assert!(s.starts_with("data:image/png;base64,"), "{s}");
    }

    #[test]
    fn missing_asset_left_alone() {
        let d = TempDir::new().unwrap();
        let mut cv = json!({"a": "assets/nope.png"});
        inline_cv_assets(&mut cv, d.path());
        assert_eq!(cv["a"], "assets/nope.png");
    }

    #[test]
    fn url_kept_and_traversal_not_inlined() {
        let d = root_with("main.cv.json", b"{}");
        let mut cv = json!({"a": "https://x.com/p.png", "b": "assets/../main.cv.json"});
        let before = cv.clone();
        inline_cv_assets(&mut cv, d.path());
        assert_eq!(cv, before);
    }

    #[test]
    fn css_font_inlined() {
        let d = root_with("assets/f.woff2", b"wOF2");
        for css in ["url(assets/f.woff2)", "url('assets/f.woff2')", "url(\"assets/f.woff2\")"] {
            let out = inline_theme_assets(css, d.path());
            assert!(out.contains("data:font/woff2;base64,"), "{css} -> {out}");
        }
        let out = inline_theme_assets("<img src=\"assets/f.woff2\"> url(assets/missing.png)", d.path());
        assert!(out.contains("src=\"data:font/woff2;base64,") && out.contains("url(assets/missing.png)"));
    }
}
