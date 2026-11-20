//! Editors that can open a theme folder, found per OS.
//! macOS finds apps by bundle id (many users never install an editor's shell command);
//! Windows and Linux look for the editor's command on `PATH`.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

pub struct Editor {
    pub id: &'static str,
    pub label: &'static str,
    /// macOS bundle id.
    pub mac: Option<&'static str>,
    /// Command on `PATH` (Windows, Linux).
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub cli: Option<&'static str>,
    /// Arguments placed before the folder.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub args: &'static [&'static str],
}

const fn ed(
    id: &'static str,
    label: &'static str,
    mac: Option<&'static str>,
    cli: Option<&'static str>,
) -> Editor {
    Editor {
        id,
        label,
        mac,
        cli,
        args: &[],
    }
}

pub const EDITORS: &[Editor] = &[
    ed(
        "vscode",
        "VS Code",
        Some("com.microsoft.VSCode"),
        Some("code"),
    ),
    ed(
        "cursor",
        "Cursor",
        Some("com.todesktop.230313mzl4w4u92"),
        Some("cursor"),
    ),
    ed(
        "windsurf",
        "Windsurf",
        Some("com.exafunction.windsurf"),
        Some("windsurf"),
    ),
    ed("zed", "Zed", Some("dev.zed.Zed"), Some("zed")),
    ed(
        "sublime",
        "Sublime Text",
        Some("com.sublimetext.4"),
        Some("subl"),
    ),
    ed("nova", "Nova", Some("com.panic.Nova"), None),
    ed("bbedit", "BBEdit", Some("com.barebones.bbedit"), None),
    ed("terminal", "Terminal", Some("com.apple.Terminal"), None),
    ed("iterm", "iTerm", Some("com.googlecode.iterm2"), None),
    Editor {
        id: "wt",
        label: "Windows Terminal",
        mac: None,
        cli: Some("wt"),
        args: &["-d"],
    },
];

/// The first `dir/{cmd}{ext}` on `path_var` that is a file.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn find_in(path_var: &OsStr, exts: &[&str], cmd: &str) -> Option<PathBuf> {
    std::env::split_paths(path_var)
        .flat_map(|dir| exts.iter().map(move |ext| dir.join(format!("{cmd}{ext}"))))
        .find(|p| p.is_file())
}

/// Resolves `cmd` on this process's `PATH`, honouring `PATHEXT` on Windows (`code` is `code.cmd`).
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn which(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let pathext = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
            .to_lowercase()
    } else {
        String::new()
    };
    let exts: Vec<&str> = std::iter::once("")
        .chain(pathext.split(';').filter(|e| !e.is_empty()))
        .collect();
    find_in(&path, &exts, cmd)
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn launch_status(s: ExitStatus, label: &str) -> io::Result<()> {
    if s.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("Could not open {label}")))
    }
}

/// Editors installed on this machine, in list order.
pub fn installed() -> Vec<&'static Editor> {
    EDITORS.iter().filter(|e| is_installed(e)).collect()
}

#[cfg(target_os = "macos")]
fn is_installed(e: &Editor) -> bool {
    // shortcut: needs Spotlight; if users report missing editors, fall back to /Applications/*.app Info.plist
    let Some(id) = e.mac else { return false };
    Command::new("mdfind")
        .arg(format!("kMDItemCFBundleIdentifier == '{id}'"))
        .output()
        .is_ok_and(|o| !o.stdout.trim_ascii().is_empty())
}

#[cfg(not(target_os = "macos"))]
fn is_installed(e: &Editor) -> bool {
    e.cli.and_then(which).is_some()
}

fn unsupported(id: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("{id} is not available on this computer"),
    )
}

/// Opens `dir` in editor `editor_id` (an id from `EDITORS`, never a command from the UI).
pub fn open(editor_id: &str, dir: &Path) -> io::Result<()> {
    let e = EDITORS
        .iter()
        .find(|e| e.id == editor_id)
        .ok_or_else(|| unsupported(editor_id))?;
    #[cfg(target_os = "macos")]
    {
        let id = e.mac.ok_or_else(|| unsupported(e.label))?;
        let s = Command::new("open").arg("-b").arg(id).arg(dir).status()?;
        launch_status(s, e.label)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let cli = e.cli.and_then(which).ok_or_else(|| unsupported(e.label))?;
        // Don't wait: some editors stay attached to the process that started them.
        Command::new(cli).args(e.args).arg(dir).spawn().map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_ids_unique_and_launchable() {
        let mut ids: Vec<_> = EDITORS.iter().map(|e| e.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), EDITORS.len());
        assert!(EDITORS.iter().all(|e| e.mac.is_some() || e.cli.is_some()));
    }

    #[test]
    fn find_in_searches_path_and_extensions() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("code.cmd"), "").unwrap();
        let path = std::env::join_paths([d.path()]).unwrap();
        assert_eq!(
            find_in(&path, &["", ".cmd"], "code"),
            Some(d.path().join("code.cmd"))
        );
        assert_eq!(find_in(&path, &[""], "code"), None);
        assert_eq!(find_in(&path, &["", ".cmd"], "zed"), None);
    }

    #[cfg(unix)]
    #[test]
    fn launch_status_reports_failure() {
        use std::process::Command;
        assert!(launch_status(Command::new("false").status().unwrap(), "X").is_err());
        assert!(launch_status(Command::new("true").status().unwrap(), "X").is_ok());
    }
}
