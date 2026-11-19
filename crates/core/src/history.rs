use std::path::Path;
use std::time::Duration;

use rusqlite::{
    params, Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior,
};
use serde::Serialize;
use serde_json::Value;

use crate::library::sha_hex;
use crate::{library::Library, schema, Author, CoreError};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS versions(
    id INTEGER PRIMARY KEY,
    library TEXT NOT NULL,
    cv TEXT NOT NULL,
    author TEXT NOT NULL,
    message TEXT NOT NULL,
    time_ms INTEGER NOT NULL,
    sha TEXT,
    body BLOB
);
CREATE INDEX IF NOT EXISTS versions_cv ON versions(library, cv, id);";

/// How long a writer waits for another process (the app or an MCP agent) before `Locked`.
const BUSY: Duration = Duration::from_secs(2);

fn connect(db: &Path) -> Result<Connection, CoreError> {
    let c = Connection::open(db)?;
    c.busy_timeout(BUSY)?;
    Ok(c)
}

/// Create the database file and table if missing.
pub(crate) fn init(db: &Path) -> Result<(), CoreError> {
    if let Some(dir) = db.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    connect(db)?.execute_batch(SCHEMA)?;
    Ok(())
}

fn busy(e: rusqlite::Error) -> CoreError {
    if e.sqlite_error_code() == Some(ErrorCode::DatabaseBusy) {
        CoreError::Locked
    } else {
        e.into()
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// True when the file parses as JSON and passes the schema.
pub(crate) fn cv_valid(path: &Path) -> bool {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .is_some_and(|v| schema::validate(&v).is_ok())
}

fn parse_id(commit: &str) -> Option<i64> {
    commit.strip_prefix('v').unwrap_or(commit).parse().ok()
}

#[derive(Debug, Clone, Serialize)]
pub struct Version {
    pub commit: String,
    pub author: String,
    pub message: String,
    pub time_ms: i64,
}

fn canonical(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<_> = m.keys().collect();
            keys.sort();
            Value::Object(
                keys.into_iter()
                    .map(|k| (k.clone(), canonical(&m[k])))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.iter().map(canonical).collect()),
        _ => v.clone(),
    }
}

fn unified(old: &str, new: &str, from: &str, to: &str) -> String {
    similar::TextDiff::from_lines(old, new)
        .unified_diff()
        .header(from, to)
        .to_string()
}

fn pretty(v: &Value) -> Result<String, CoreError> {
    Ok(serde_json::to_string_pretty(&canonical(v))? + "\n")
}

impl Library {
    /// Run `f` inside one write transaction. SQLite's lock serialises the app and MCP processes;
    /// a writer that waits longer than 2 s gets `Locked`. Nothing is recorded if `f` fails.
    pub fn locked<T>(
        &self,
        f: impl FnOnce(&Transaction) -> Result<T, CoreError>,
    ) -> Result<T, CoreError> {
        let mut c = connect(self.db())?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(busy)?;
        let out = f(&tx)?;
        tx.commit().map_err(busy)?;
        Ok(out)
    }

    /// Add a version unconditionally. `body` None marks the CV as deleted.
    pub(crate) fn insert(
        &self,
        c: &Connection,
        name: &str,
        author: Author,
        message: &str,
        body: Option<&[u8]>,
    ) -> Result<String, CoreError> {
        c.execute(
            "INSERT INTO versions(library, cv, author, message, time_ms, sha, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                self.id(),
                name,
                author.name(),
                message,
                now_ms(),
                body.map(sha_hex),
                body
            ],
        )?;
        Ok(c.last_insert_rowid().to_string())
    }

    /// Add a version unless the content equals the newest one; returns "" when nothing changed.
    pub(crate) fn record(
        &self,
        c: &Connection,
        name: &str,
        author: Author,
        message: &str,
        body: Option<&[u8]>,
    ) -> Result<String, CoreError> {
        let latest: Option<Option<String>> = c
            .query_row(
                "SELECT sha FROM versions WHERE library = ?1 AND cv = ?2 ORDER BY id DESC LIMIT 1",
                params![self.id(), name],
                |r| r.get(0),
            )
            .optional()?;
        if latest.flatten() == body.map(sha_hex) {
            return Ok(String::new());
        }
        self.insert(c, name, author, message, body)
    }

    /// Record outside changes to CV files. Call inside `locked`.
    pub fn sweep_external(&self, tx: &Transaction) -> Result<Option<String>, CoreError> {
        self.sweep(tx, Author::External, "External changes")
    }

    pub(crate) fn sweep(
        &self,
        tx: &Transaction,
        author: Author,
        message: &str,
    ) -> Result<Option<String>, CoreError> {
        let mut last = None;
        for name in self.cv_names()? {
            let Ok(path) = self.cv_path(&name) else {
                continue;
            };
            // Broken files are skipped until they parse and validate again.
            if !cv_valid(&path) {
                continue;
            }
            let id = self.record(tx, &name, author, message, Some(&std::fs::read(&path)?))?;
            if !id.is_empty() {
                last = Some(id);
            }
        }
        let live: Vec<String> = tx
            .prepare(
                "SELECT v.cv FROM versions v
                 WHERE v.library = ?1 AND v.body IS NOT NULL
                   AND v.id = (SELECT MAX(id) FROM versions WHERE library = v.library AND cv = v.cv)",
            )?
            .query_map([self.id()], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for name in live {
            if self.cv_path(&name).is_ok_and(|p| !p.exists()) {
                last = Some(self.insert(tx, &name, author, message, None)?);
            }
        }
        Ok(last)
    }

    pub fn sweep_external_locked(&self) -> Result<Option<String>, CoreError> {
        self.locked(|tx| self.sweep_external(tx))
    }

    /// Versions of `name`, newest first.
    pub fn history(&self, name: &str, limit: usize) -> Result<Vec<Version>, CoreError> {
        self.cv_path(name)?;
        let c = connect(self.db())?;
        let rows = c
            .prepare(
                "SELECT id, author, message, time_ms FROM versions
                 WHERE library = ?1 AND cv = ?2 ORDER BY id DESC LIMIT ?3",
            )?
            .query_map(
                params![self.id(), name, limit.min(i64::MAX as usize) as i64],
                |r| {
                    Ok(Version {
                        commit: r.get::<_, i64>(0)?.to_string(),
                        author: r.get(1)?,
                        message: r.get(2)?,
                        time_ms: r.get(3)?,
                    })
                },
            )?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    pub fn read_cv_at(&self, name: &str, commit: &str) -> Result<Value, CoreError> {
        self.cv_path(name)?;
        let missing = || CoreError::NotFound(format!("{name} at {commit}"));
        let id = parse_id(commit).ok_or_else(missing)?;
        let body: Option<Vec<u8>> = connect(self.db())?
            .query_row(
                "SELECT body FROM versions WHERE id = ?1 AND library = ?2 AND cv = ?3",
                params![id, self.id(), name],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(serde_json::from_slice(&body.ok_or_else(missing)?)?)
    }

    pub fn diff(&self, name: &str, commit: &str) -> Result<String, CoreError> {
        let old = pretty(&self.read_cv_at(name, commit)?)?;
        let new = pretty(&self.read_cv(name)?.cv)?;
        let id = parse_id(commit).unwrap_or_default();
        Ok(unified(&old, &new, &format!("v{id}"), "current"))
    }

    /// What `commit` changed in this CV: its previous version (empty if none) against `commit`.
    pub fn change_diff(&self, name: &str, commit: &str) -> Result<String, CoreError> {
        let new = pretty(&self.read_cv_at(name, commit)?)?;
        let id = parse_id(commit).unwrap_or_default();
        let prev: Option<(i64, Option<Vec<u8>>)> = connect(self.db())?
            .query_row(
                "SELECT id, body FROM versions WHERE library = ?1 AND cv = ?2 AND id < ?3
                 ORDER BY id DESC LIMIT 1",
                params![self.id(), name, id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (old, from) = match prev {
            Some((p, Some(body))) => (pretty(&serde_json::from_slice(&body)?)?, format!("v{p}")),
            _ => (String::new(), "empty".to_string()),
        };
        Ok(unified(&old, &new, &from, &format!("v{id}")))
    }
}
