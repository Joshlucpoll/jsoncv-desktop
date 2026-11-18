use std::path::PathBuf;

use minijinja::{Environment, Value};
use serde::{Deserialize, Serialize};

use crate::assets::{inline_cv_assets, inline_theme_assets};
use crate::library::Library;
use crate::CoreError;

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

// Task 7 fills this.
fn builtin_themes() -> &'static [BuiltinTheme] {
    &[]
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
        let mut ids: Vec<String> = std::fs::read_dir(self.root().join("themes"))?
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|id| valid_theme_id(id))
            .collect();
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
            .map_err(|e| {
                CoreError::Render(RenderError {
                    message: match e.detail() {
                        Some(d) => format!("{}: {d}", e.kind()),
                        None => e.kind().to_string(),
                    },
                    line: e.line(),
                })
            })?;
        let style = format!("<style>{css}</style>");
        Ok(match body.find("</head>") {
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
        let id = theme.or_else(|| cv["meta"]["theme"].as_str()).unwrap_or("builtin:classic");
        match self.render(&cv, id) {
            Err(CoreError::NotFound(_)) if id != "builtin:classic" => self.render(&cv, "builtin:classic"),
            r => r,
        }
    }
}

pub(crate) fn environment() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::Html);
    env.add_filter("format_date", |v: Value, fmt: String| format_date(&v, &fmt));
    env.add_function("format_date", |v: Value, fmt: String| format_date(&v, &fmt));
    env.add_filter("markdown", markdown);
    env.add_filter("strip_scheme", strip_scheme);
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
    use pulldown_cmark::{html::push_html, Event, Parser};
    let text = text.unwrap_or_default();
    let events = Parser::new(&text).map(|e| match e {
        Event::Html(s) | Event::InlineHtml(s) => Event::Text(s),
        e => e,
    });
    let mut out = String::new();
    push_html(&mut out, events);
    Value::from_safe_string(out)
}

fn strip_scheme(url: Option<String>) -> String {
    let u = url.unwrap_or_default();
    let u = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://")).unwrap_or(&u);
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
    fn strip_scheme_cases() {
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
