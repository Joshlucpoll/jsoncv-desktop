use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use minijinja::{Environment, Value};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::assets::{inline_cv_assets, inline_theme_assets};
use crate::coverage::{missing_fields, ThemeReport};
use crate::history::{commit, with_lock};
use crate::library::Library;
use crate::{Author, CoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageSize {
    A4,
    Letter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageMeta {
    pub size: PageSize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeMeta {
    pub name: String,
    pub description: String,
    pub page: PageMeta,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThemeInfo {
    pub id: String,
    pub builtin: bool,
    pub meta: ThemeMeta,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThemeFiles {
    pub info: ThemeInfo,
    pub template: String,
    pub css: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RenderError {
    pub message: String,
    pub line: Option<usize>,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            Some(l) => write!(f, "{} (line {l})", self.message),
            None => f.write_str(&self.message),
        }
    }
}

pub(crate) struct BuiltinTheme {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub size: PageSize,
    pub template: &'static str,
    pub css: &'static str,
}

fn builtin_themes() -> &'static [BuiltinTheme] {
    const THEMES: &[BuiltinTheme] = &[
        BuiltinTheme {
            id: "builtin:classic",
            name: "Classic",
            description: "One column, serif headings, tight print-friendly spacing.",
            size: PageSize::A4,
            template: include_str!("../themes/classic/template.html.j2"),
            css: include_str!("../themes/classic/style.css"),
        },
        BuiltinTheme {
            id: "builtin:two-column",
            name: "Two column",
            description: "Tinted sidebar for contact and skills, main column for experience.",
            size: PageSize::A4,
            template: include_str!("../themes/two-column/template.html.j2"),
            css: include_str!("../themes/two-column/style.css"),
        },
    ];
    THEMES
}

impl BuiltinTheme {
    fn files(&self) -> ThemeFiles {
        ThemeFiles {
            info: ThemeInfo {
                id: self.id.into(),
                builtin: true,
                meta: ThemeMeta {
                    name: self.name.into(),
                    description: self.description.into(),
                    page: PageMeta { size: self.size },
                },
            },
            template: self.template.into(),
            css: self.css.into(),
        }
    }
}

fn valid_theme_id(s: &str) -> bool {
    let b = s.as_bytes();
    (1..=64).contains(&b.len())
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

fn nf(name: &str) -> impl Fn(std::io::Error) -> CoreError + '_ {
    move |e| match e.kind() {
        std::io::ErrorKind::NotFound => CoreError::NotFound(name.to_string()),
        _ => e.into(),
    }
}

impl Library {
    fn theme_dir(&self, id: &str) -> Result<PathBuf, CoreError> {
        if !valid_theme_id(id) {
            return Err(CoreError::BadName(id.to_string()));
        }
        Ok(self.root().join("themes").join(id))
    }

    pub fn list_themes(&self) -> Result<Vec<ThemeInfo>, CoreError> {
        let mut out: Vec<ThemeInfo> = builtin_themes().iter().map(|b| b.files().info).collect();
        let mut ids: Vec<String> = match std::fs::read_dir(self.root().join("themes")) {
            Ok(rd) => rd.filter_map(|e| e.ok()?.file_name().into_string().ok()).filter(|id| valid_theme_id(id)).collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
            Err(e) => return Err(e.into()),
        };
        ids.sort();
        for id in ids {
            if let Ok(t) = self.get_theme(&id) {
                out.push(t.info);
            }
        }
        Ok(out)
    }

    pub fn get_theme(&self, id: &str) -> Result<ThemeFiles, CoreError> {
        if id.starts_with("builtin:") {
            return builtin_themes()
                .iter()
                .find(|b| b.id == id)
                .map(BuiltinTheme::files)
                .ok_or_else(|| CoreError::NotFound(id.to_string()));
        }
        let dir = self.theme_dir(id)?;
        let meta: ThemeMeta = serde_json::from_slice(&std::fs::read(dir.join("theme.json")).map_err(nf(id))?)?;
        Ok(ThemeFiles {
            info: ThemeInfo { id: id.into(), builtin: false, meta },
            template: std::fs::read_to_string(dir.join("template.html.j2")).map_err(nf(id))?,
            css: std::fs::read_to_string(dir.join("style.css")).map_err(nf(id))?,
        })
    }

    pub fn theme_report(&self, id: &str) -> Result<ThemeReport, CoreError> {
        report(&self.get_theme(id)?.template)
    }

    pub fn save_theme(
        &self,
        id: &str,
        template: &str,
        css: &str,
        meta: &ThemeMeta,
        author: Author,
    ) -> Result<ThemeReport, CoreError> {
        if id.starts_with("builtin:") {
            return Err(CoreError::ReadOnly(format!("{id} is a built-in theme; duplicate it to edit")));
        }
        let dir = self.theme_dir(id)?;
        let meta_json = serde_json::to_string_pretty(meta)? + "\n";
        with_lock(self.root(), || {
            self.sweep_external()?;
            std::fs::create_dir_all(&dir)?;
            let files = [("template.html.j2", template), ("style.css", css), ("theme.json", meta_json.as_str())];
            for (f, body) in files {
                write_atomic(&dir.join(f), body.as_bytes())?;
            }
            let rels: Vec<String> = files.iter().map(|(f, _)| format!("themes/{id}/{f}")).collect();
            let paths: Vec<&Path> = rels.iter().map(Path::new).collect();
            commit(self.root(), &paths, author, &format!("Saved theme {id}"))?;
            Ok(())
        })?;
        report(template)
    }

    pub fn duplicate_theme(&self, id: &str, new_id: &str, author: Author) -> Result<ThemeInfo, CoreError> {
        let src = self.get_theme(id)?;
        if new_id.starts_with("builtin:") {
            return Err(CoreError::BadName(new_id.to_string()));
        }
        let dir = self.theme_dir(new_id)?;
        with_lock(self.root(), || {
            self.sweep_external()?;
            if dir.exists() {
                return Err(CoreError::BadName(format!("Theme {new_id} already exists")));
            }
            let res = (|| {
            std::fs::create_dir_all(&dir)?;
            let meta = ThemeMeta { name: format!("{} copy", src.info.meta.name), ..src.info.meta.clone() };
            write_atomic(&dir.join("template.html.j2"), src.template.as_bytes())?;
            write_atomic(&dir.join("style.css"), src.css.as_bytes())?;
            write_atomic(&dir.join("theme.json"), (serde_json::to_string_pretty(&meta)? + "\n").as_bytes())?;
            let mut rels = vec!["template.html.j2".to_string(), "style.css".into(), "theme.json".into()];
            if !src.info.builtin {
                copy_tree(&self.theme_dir(id)?.join("assets"), &dir.join("assets"), "assets", &mut rels)?;
            }
            let rels: Vec<String> = rels.iter().map(|r| format!("themes/{new_id}/{r}")).collect();
            let paths: Vec<&Path> = rels.iter().map(Path::new).collect();
            commit(self.root(), &paths, author, &format!("Duplicated theme {id} as {new_id}"))?;
            Ok(())
            })();
            if res.is_err() {
                let _ = std::fs::remove_dir_all(&dir); // don't leave a half-copied theme behind
            }
            res
        })?;
        Ok(self.get_theme(new_id)?.info)
    }

    pub fn render(&self, cv: &serde_json::Value, theme_id: &str) -> Result<String, CoreError> {
        let theme = self.get_theme(theme_id)?;
        let (mut template, mut css) = (theme.template, theme.css);
        if !theme.info.builtin {
            let dir = self.theme_dir(theme_id)?;
            template = inline_theme_assets(&template, &dir);
            css = inline_theme_assets(&css, &dir);
        }
        let mut cv = cv.clone();
        inline_cv_assets(&mut cv, self.root());
        let body = environment()
            .render_str(&template, minijinja::context! { cv => Value::from(minijinja::value::Serde(&cv)) })
            .map_err(render_error)?;
        static STYLE_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</style").unwrap());
        static HEAD_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</head>").unwrap());
        let style = format!("<style>{}</style>", STYLE_END.replace_all(&css, r"<\/style"));
        Ok(match HEAD_END.find(&body).map(|m| m.start()) {
            Some(i) => format!("{}{style}{}", &body[..i], &body[i..]),
            None => format!(
                "<!doctype html><html><head><meta charset=\"utf-8\">{style}</head><body>{body}</body></html>"
            ),
        })
    }

    pub fn render_cv(&self, name: &str, theme: Option<&str>, commit: Option<&str>) -> Result<String, CoreError> {
        let cv = match commit {
            Some(sha) => self.read_cv_at(name, sha)?,
            None => self.read_cv(name)?.cv,
        };
        if let Some(id) = theme {
            return self.render(&cv, id);
        }
        let id = cv["meta"]["theme"].as_str().unwrap_or("builtin:classic");
        match self.render(&cv, id) {
            // Only a stale or hand-edited meta.theme falls back; an explicit choice reports its error.
            Err(CoreError::NotFound(_) | CoreError::BadName(_)) if id != "builtin:classic" => {
                self.render(&cv, "builtin:classic")
            }
            r => r,
        }
    }
}

fn report(template: &str) -> Result<ThemeReport, CoreError> {
    match missing_fields(template) {
        Ok(missing) => Ok(ThemeReport { render_error: None, missing }),
        Err(CoreError::Render(e)) => Ok(ThemeReport { render_error: Some(e), missing: vec![] }),
        Err(e) => Err(e),
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_file_name(format!(".{}.tmp-{}", path.file_name().unwrap().to_string_lossy(), std::process::id()));
    let res = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        std::io::Write::write_all(&mut f, bytes)?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    res
}

/// Copies `src` into `dst` if it exists, pushing each file's path (relative to the theme dir) onto `rels`.
fn copy_tree(src: &Path, dst: &Path, rel: &str, rels: &mut Vec<String>) -> std::io::Result<()> {
    let rd = match std::fs::read_dir(src) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    std::fs::create_dir_all(dst)?;
    for e in rd {
        let e = e?;
        let name = e.file_name();
        let r = format!("{rel}/{}", name.to_string_lossy());
        if e.file_type()?.is_dir() {
            copy_tree(&e.path(), &dst.join(&name), &r, rels)?;
        } else {
            std::fs::copy(e.path(), dst.join(&name))?;
            rels.push(r);
        }
    }
    Ok(())
}

pub(crate) fn render_error(e: minijinja::Error) -> CoreError {
    CoreError::Render(RenderError {
        message: match e.detail() {
            Some(d) => format!("{}: {d}", e.kind()),
            None => e.kind().to_string(),
        },
        line: e.line(),
    })
}

pub(crate) fn environment() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::Html);
    env.add_filter("format_date", |v: Value, fmt: String| format_date(&v, &fmt));
    env.add_function("format_date", |v: Value, fmt: String| format_date(&v, &fmt));
    env.add_filter("markdown", markdown);
    env.add_filter("strip_scheme", strip_scheme);
    env.add_filter("safe_url", |u: Option<String>| safe_url(u.unwrap_or_default().into()).to_string());
    env
}

pub(crate) fn format_date(v: &Value, fmt: &str) -> String {
    if v.is_undefined() || v.is_none() {
        return String::new();
    }
    let raw = v.to_string();
    let parts: Vec<&str> = raw.split('-').collect();
    let num = |i: usize, len: usize, max: u32| -> Option<Option<u32>> {
        match parts.get(i) {
            None => Some(None),
            Some(p) if p.len() == len && p.bytes().all(|b| b.is_ascii_digit()) => {
                let n: u32 = p.parse().ok()?;
                (if i == 0 { true } else { (1..=max).contains(&n) }).then_some(Some(n))
            }
            _ => None,
        }
    };
    let (Some(Some(y)), Some(m), Some(d), true) = (num(0, 4, 0), num(1, 2, 12), num(2, 2, 31), parts.len() <= 3) else {
        return raw;
    };
    const MONTHS: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let mut out = String::new();
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' || chars.peek().is_none() {
            out.push(c);
            continue;
        }
        let t = chars.next().unwrap();
        let piece = match t {
            'Y' => Some(format!("{y:04}")),
            'm' => m.map(|m| format!("{m:02}")),
            'd' => d.map(|d| format!("{d:02}")),
            'b' => m.map(|m| MONTHS[m as usize - 1][..3].to_string()),
            'B' => m.map(|m| MONTHS[m as usize - 1].to_string()),
            other => {
                out.push('%');
                out.push(other);
                continue;
            }
        };
        match piece {
            Some(p) => out.push_str(&p),
            // Missing part: drop the token and the separator text that follows it.
            None => while chars.next_if(|c| *c != '%').is_some() {},
        }
    }
    out
}

fn markdown(text: Option<String>) -> Value {
    use pulldown_cmark::{html::push_html, Event, Parser, Tag};
    let text = text.unwrap_or_default();
    let events = Parser::new(&text).map(|e| match e {
        Event::Html(s) | Event::InlineHtml(s) => Event::Text(s),
        Event::Start(Tag::Link { link_type, dest_url, title, id }) => {
            Event::Start(Tag::Link { link_type, dest_url: safe_url(dest_url), title, id })
        }
        Event::Start(Tag::Image { link_type, dest_url, title, id }) => {
            Event::Start(Tag::Image { link_type, dest_url: safe_url(dest_url), title, id })
        }
        e => e,
    });
    let mut out = String::new();
    push_html(&mut out, events);
    Value::from_safe_string(out)
}

/// Allow only http(s), mailto, tel or scheme-less (relative / `#`) destinations.
fn safe_url(u: pulldown_cmark::CowStr<'_>) -> pulldown_cmark::CowStr<'_> {
    // Browsers ignore whitespace/control chars inside a scheme, so drop them before checking.
    let t: String = u.chars().filter(|c| !c.is_whitespace() && !c.is_control()).collect::<String>().to_lowercase();
    let scheme = t.split(['/', '?', '#']).next().unwrap_or("").split_once(':').map(|(s, _)| s);
    match scheme {
        None | Some("http" | "https" | "mailto" | "tel") => u,
        Some(_) => "#".into(),
    }
}

fn strip_scheme(url: Option<String>) -> String {
    let u = url.unwrap_or_default();
    let n = ["https://", "http://"]
        .iter()
        .find(|p| u.get(..p.len()).is_some_and(|h| h.eq_ignore_ascii_case(p)))
        .map_or(0, |p| p.len());
    let u = &u[n..];
    u.strip_suffix('/').unwrap_or(u).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fd(v: &str, f: &str) -> String {
        format_date(&Value::from(v), f)
    }

    #[test]
    fn format_date_cases() {
        assert_eq!(fd("2020-03", "%b %Y"), "Mar 2020");
        assert_eq!(fd("2020", "%b %Y"), "2020");
        assert_eq!(fd("2020-03-05", "%d %b %Y"), "05 Mar 2020");
        assert_eq!(fd("2020-03", "%B %Y"), "March 2020");
        assert_eq!(fd("garbage", "%Y"), "garbage");
        assert_eq!(fd("2020-13", "%Y"), "2020-13");
        assert_eq!(format_date(&Value::UNDEFINED, "%Y"), "");
        assert_eq!(format_date(&Value::from(()), "%Y"), "");
    }

    #[test]
    fn markdown_strips_javascript_links() {
        let md = |t: &str| environment().render_str("{{ t | markdown }}", minijinja::context! { t => t }).unwrap();
        for bad in ["[x](javascript:alert(1))", "[x](JaVaScRiPt:alert(1))", "[x](<  javascript:alert(1)>)", "![x](data:text/html,a)"] {
            let o = md(bad);
            assert!(!o.to_lowercase().contains("javascript:") && !o.contains("data:"), "{bad} -> {o}");
        }
        assert!(md("[x](https://a.com)").contains(r#"href="https://a.com""#));
        assert!(md("[x](#top)").contains(r##"href="#top""##));
        assert!(md("[x](mailto:a@b.c)").contains("mailto:a@b.c"));
    }

    #[test]
    fn safe_url_filter() {
        let f = |u: &str| environment().render_str("{{ u | safe_url | safe }}", minijinja::context! { u => u }).unwrap();
        assert_eq!(f("https://a.com/x"), "https://a.com/x");
        assert_eq!(f("mailto:a@b.c"), "mailto:a@b.c");
        assert_eq!(f("/rel"), "/rel");
        assert_eq!(f("JavaScript:alert(1)"), "#");
        assert_eq!(f(" java\nscript:alert(1)"), "#");
    }

    #[test]
    fn strip_scheme_cases() {
        assert_eq!(strip_scheme(Some("HTTPS://A.com/".into())), "A.com");
        assert_eq!(strip_scheme(Some("https://a.com/".into())), "a.com");
        assert_eq!(strip_scheme(Some("http://a.com/x".into())), "a.com/x");
        assert_eq!(strip_scheme(None), "");
    }

    #[test]
    fn markdown_renders_bold_and_escapes_html() {
        let out = environment().render_str("{{ t | markdown }}", minijinja::context! { t => "**a** <b>" }).unwrap();
        assert!(out.contains("<strong>a</strong>"), "{out}");
        assert!(out.contains("&lt;b&gt;"), "{out}");
    }
}
