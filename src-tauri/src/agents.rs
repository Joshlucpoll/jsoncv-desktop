//! One-click MCP setup for coding agents: the agent's own CLI where it has one, else its JSON config file.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use directories::BaseDirs;
use jsoncv_core::CoreError;
use serde_json::{json, Value};

/// The server name every agent registers.
const NAME: &str = "jsoncv";

/// Registers `bin mcp` as the `jsoncv` MCP server in `agent`. Returns a line for the user.
pub fn connect(agent: &str, bin: &str) -> Result<String, CoreError> {
    let dirs = BaseDirs::new().ok_or_else(|| CoreError::NotFound("home folder".into()))?;
    let (home, cfg) = (dirs.home_dir(), dirs.config_dir()); // cfg: ~/Library/Application Support on macOS
    let entry = json!({ "command": bin, "args": ["mcp"] });
    match agent {
        // Remove first so a moved app updates the path instead of failing with "already exists".
        "claude-code" => cli(
            "claude",
            "claude mcp remove --scope user jsoncv >/dev/null 2>&1; claude mcp add --scope user jsoncv -- \"$1\" mcp",
            bin,
            "Claude Code",
        ),
        "codex" => cli(
            "codex",
            "codex mcp remove jsoncv >/dev/null 2>&1; codex mcp add jsoncv -- \"$1\" mcp",
            bin,
            "Codex",
        ),
        "claude-desktop" => merge(&cfg.join("Claude"), "claude_desktop_config.json", "mcpServers", entry, "Claude Desktop"),
        "cursor" => merge(&home.join(".cursor"), "mcp.json", "mcpServers", entry, "Cursor"),
        "gemini" => merge(&home.join(".gemini"), "settings.json", "mcpServers", entry, "Gemini CLI"),
        "windsurf" => merge(&home.join(".codeium/windsurf"), "mcp_config.json", "mcpServers", entry, "Windsurf"),
        "vscode" => merge(
            &cfg.join("Code/User"),
            "mcp.json",
            "servers",
            json!({ "type": "stdio", "command": bin, "args": ["mcp"] }),
            "VS Code",
        ),
        _ => Err(CoreError::BadName(agent.into())),
    }
}

/// Runs `script` in the user's login shell, so the agent CLI is found on the PATH their terminal uses.
fn cli(exe: &str, script: &str, bin: &str, label: &str) -> Result<String, CoreError> {
    let shell = std::env::var("SHELL")
        .ok()
        .filter(|s| s.ends_with("zsh") || s.ends_with("bash"))
        .unwrap_or_else(|| "/bin/zsh".into());
    let script = format!(
        "command -v {exe} >/dev/null 2>&1 || {{ echo 'not-found' >&2; exit 127; }}; {script}"
    );
    // -i reads .zshrc too, where many installers put their PATH line. "$1" is the app binary.
    let mut child = Command::new(shell)
        .args(["-ilc", &script, "jsoncv", bin])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            return Err(std::io::Error::other(format!("{label} setup timed out")).into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    match status.code() {
        Some(0) => Ok(format!(
            "Added jsoncv to {label}. Start a new {label} session to use it."
        )),
        Some(127) if err.contains("not-found") => Err(CoreError::NotFound(format!(
            "the `{exe}` command. Is {label} installed? Copy the setup prompt instead"
        ))),
        _ => Err(std::io::Error::other(format!(
            "{label} setup failed: {}",
            err.lines()
                .filter(|l| !l.trim().is_empty())
                .last()
                .unwrap_or("unknown error")
        ))
        .into()),
    }
}

/// Sets `<key>.jsoncv = entry` in the JSON file `dir/file`, keeping everything else in it.
fn merge(
    dir: &Path,
    file: &str,
    key: &str,
    entry: Value,
    label: &str,
) -> Result<String, CoreError> {
    if !dir.is_dir() {
        return Err(CoreError::NotFound(format!(
            "{label} settings ({}). Is {label} installed?",
            dir.display()
        )));
    }
    let path = dir.join(file);
    let mut root: Value = match std::fs::read(&path) {
        Ok(b) if b.iter().all(u8::is_ascii_whitespace) => json!({}),
        Ok(b) => serde_json::from_slice(&b).map_err(|_| {
            CoreError::BadPath(format!(
                "{} is not plain JSON (it may hold comments), so it was left unchanged. Add the server by hand",
                path.display()
            ))
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e.into()),
    };
    let servers = root
        .as_object_mut()
        .ok_or_else(|| CoreError::BadPath(format!("{} is not a JSON object", path.display())))?
        .entry(key)
        .or_insert_with(|| json!({}));
    servers
        .as_object_mut()
        .ok_or_else(|| CoreError::BadPath(format!("{key} in {} is not an object", path.display())))?
        .insert(NAME.into(), entry);
    let tmp = dir.join(format!(".{file}.jsoncv-tmp"));
    std::fs::write(&tmp, serde_json::to_string_pretty(&root)? + "\n")?;
    std::fs::rename(&tmp, &path)?;
    Ok(format!(
        "Added jsoncv to {label}. Restart {label} to use it."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_keeps_other_settings_and_refuses_jsonc() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("mcp.json");
        std::fs::write(
            &f,
            r#"{"theme":"dark","mcpServers":{"other":{"command":"x"}}}"#,
        )
        .unwrap();
        merge(
            d.path(),
            "mcp.json",
            "mcpServers",
            json!({"command": "/a b/jsoncv"}),
            "T",
        )
        .unwrap();
        let v: Value = serde_json::from_slice(&std::fs::read(&f).unwrap()).unwrap();
        assert_eq!(v["theme"], "dark");
        assert_eq!(v["mcpServers"]["other"]["command"], "x");
        assert_eq!(v["mcpServers"]["jsoncv"]["command"], "/a b/jsoncv");
        // A missing file is created; a file with comments is left alone.
        merge(d.path(), "new.json", "servers", json!({}), "T").unwrap();
        assert!(d.path().join("new.json").exists());
        std::fs::write(&f, "// c\n{}").unwrap();
        assert!(merge(d.path(), "mcp.json", "mcpServers", json!({}), "T").is_err());
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "// c\n{}");
        // An agent that is not installed (no settings folder) is an error, not a new folder.
        assert!(merge(
            &d.path().join("nope"),
            "mcp.json",
            "mcpServers",
            json!({}),
            "T"
        )
        .is_err());
        assert!(!d.path().join("nope").exists());
    }

    #[test]
    fn cli_runs_in_login_shell_and_reports_missing_command() {
        // "$1" is the binary path, spaces included.
        assert!(cli("test", r#"test "$1" = "/a b/x""#, "/a b/x", "T").is_ok());
        assert!(matches!(
            cli("jsoncv-no-such-cmd", "true", "/x", "T"),
            Err(CoreError::NotFound(_))
        ));
        assert!(cli("false", "false", "/x", "T").is_err());
    }
}
