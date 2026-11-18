//! Field-coverage check: render a template against a probe CV whose every value is wrapped in a
//! tracking object, then report the schema fields the template never read.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use minijinja::value::{Enumerator, Object, ObjectRepr, Value};
use serde::Serialize;
use serde_json::Value as Json;

use crate::render::{environment, render_error, RenderError};
use crate::schema::SCHEMA;
use crate::CoreError;

type Rec = Arc<Mutex<BTreeSet<String>>>;

#[derive(Debug, Clone, Serialize)]
pub struct ThemeReport {
    pub render_error: Option<RenderError>,
    pub missing: Vec<String>,
}

fn probe_node(schema: &Json, root: &Json) -> Json {
    if let Some(r) = schema["$ref"].as_str() {
        if r.ends_with("/iso8601") {
            return "2020-01-01".into();
        }
        return match r.strip_prefix('#').and_then(|p| root.pointer(p)) {
            Some(t) => probe_node(t, root),
            None => "x".into(),
        };
    }
    match schema["type"].as_str() {
        Some("boolean") => true.into(),
        Some("number" | "integer") => 1.into(),
        Some("array") => Json::Array(vec![probe_node(&schema["items"], root)]),
        Some("object") => Json::Object(
            schema["properties"].as_object().into_iter().flatten().map(|(k, v)| (k.clone(), probe_node(v, root))).collect(),
        ),
        _ => match schema["format"].as_str() {
            Some("email") => "a@b.co".into(),
            Some("uri") => "https://x.co".into(),
            Some("date") => "2020-01-01".into(),
            _ => "x".into(),
        },
    }
}

/// A CV with one value for every property in the schema, except `$schema` and `meta`.
pub(crate) fn probe_cv(schema: &Json) -> Json {
    let Json::Object(mut o) = probe_node(schema, schema) else { return Json::Null };
    o.shift_remove("$schema");
    o.shift_remove("meta");
    Json::Object(o)
}

#[derive(Debug)]
struct Tracked {
    node: Json,
    path: String,
    rec: Rec,
}

fn wrap(node: &Json, path: String, rec: &Rec) -> Value {
    match node {
        Json::Object(_) | Json::Array(_) => Value::from_object(Tracked { node: node.clone(), path, rec: rec.clone() }),
        leaf => Value::from(minijinja::value::Serde(leaf)),
    }
}

impl Tracked {
    fn record(&self, p: &str) {
        self.rec.lock().unwrap().insert(p.to_string());
    }
}

impl Object for Tracked {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        if self.node.is_array() { ObjectRepr::Seq } else { ObjectRepr::Map }
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        match &self.node {
            Json::Object(o) => {
                let k = key.as_str()?;
                let child = o.get(k)?;
                let path = if self.path.is_empty() { k.to_string() } else { format!("{}.{k}", self.path) };
                self.record(&path);
                Some(wrap(child, path, &self.rec))
            }
            Json::Array(a) => {
                let item = a.get(key.as_usize()?)?;
                let path = format!("{}[]", self.path);
                self.record(&path);
                Some(wrap(item, path, &self.rec))
            }
            _ => None,
        }
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        match &self.node {
            Json::Object(o) => Enumerator::Iter(Box::new(o.keys().map(|k| Value::from(k.as_str())).collect::<Vec<_>>().into_iter())),
            Json::Array(a) => Enumerator::Seq(a.len()),
            _ => Enumerator::Empty,
        }
    }
}

fn leaves(node: &Json, path: &str, out: &mut Vec<String>) {
    match node {
        Json::Object(o) => {
            for (k, v) in o {
                leaves(v, &if path.is_empty() { k.clone() } else { format!("{path}.{k}") }, out);
            }
        }
        Json::Array(a) if a.iter().any(|v| v.is_object() || v.is_array()) => {
            a.iter().for_each(|v| leaves(v, &format!("{path}[]"), out))
        }
        _ => out.push(path.to_string()),
    }
}

/// Schema leaf fields the template never reads. A fully unread top-level section collapses to
/// `"<section> (whole section)"`.
pub fn missing_fields(template: &str) -> Result<Vec<String>, CoreError> {
    let schema: Json = serde_json::from_str(SCHEMA)?;
    let probe = probe_cv(&schema);
    let rec: Rec = Default::default();
    let cv = wrap(&probe, String::new(), &rec);
    environment().render_str(template, minijinja::context! { cv => cv }).map_err(render_error)?;
    let read = rec.lock().unwrap();
    let mut all = vec![];
    leaves(&probe, "", &mut all);
    let section = |p: &str| p.split(['.', '[']).next().unwrap_or(p).to_string();
    let mut out = BTreeSet::new();
    for l in &all {
        let s = section(l);
        if !all.iter().any(|o| section(o) == s && read.contains(o)) {
            out.insert(format!("{s} (whole section)"));
        } else if !read.contains(l) {
            out.insert(l.clone());
        }
    }
    Ok(out.into_iter().collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn probe_is_valid() {
        let schema: serde_json::Value = serde_json::from_str(super::SCHEMA).unwrap();
        let probe = super::probe_cv(&schema);
        assert!(crate::schema::validate(&probe).is_ok(), "{:?}", crate::schema::validate(&probe));
        assert!(probe.get("meta").is_none() && probe.get("$schema").is_none());
    }
}
