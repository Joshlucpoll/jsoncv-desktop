<img src="assets/icon.svg" width="64" height="64" alt="">

# jsoncv-desktop

## What it is

jsoncv-desktop is a macOS app (Tauri 2) for keeping your CV as plain JSON. A theme turns the JSON into HTML or PDF. Every change to a CV is saved as a version, so you can undo anything. Keep as many CVs as you like (for example, one per job), organised in folders. Duplicate one to start another. An AI agent can edit everything over MCP.

## Features

- Edit a CV in a form (or as JSON, under Options), with live preview and schema validation.
- Click anything in the preview to jump to its field; the field you edit is marked in the preview.
- Page setup per CV (Options): paper-sized pages, or one continuous page in the preview and the PDF.
- Swappable themes (Jinja template + CSS), with a field-coverage warning.
- Undo and redo (⌘Z / ⇧⌘Z) for every saved change, yours or an agent's.
- Folders and pinned CVs. A duplicate is an independent copy.
- Export to HTML and PDF (PDF needs an installed Chromium browser).
- Built-in MCP server for AI agents.

## Build

Prerequisites: Rust (rustup), Node 22, Xcode Command Line Tools, and `cargo install tauri-cli`.

```sh
npm --prefix ui ci
cargo tauri dev      # run the app
cargo tauri build    # bundle: target/release/bundle/{macos,dmg}
```

## Library layout

The default library is `~/Documents/CVs`. Change it in Settings.

```
<library>/
  cvs/
    <slug>.cv.json
    <folder>/<slug>.cv.json # one level of folders
  assets/                   # CV files: photo, logos
  themes/<theme-id>/
    template.html.j2
    style.css
    theme.json              # { name, description, page: { size: "A4" | "Letter" } }
    assets/                 # theme files: fonts, icons (optional)
  exports/
  .jsoncv-id                # links the folder to its history
  .jsoncv-pins.json         # pinned CVs, in order
```

A CV's name is its file name, so a CV keeps its history when you move it to another folder (in the app or in Finder). Names must be unique across folders. Libraries from earlier versions (`main.cv.json`, `tailored/`) are moved to `cvs/` the first time they open.

History is kept outside the library, in one SQLite file:
`~/Library/Application Support/dev.jsoncv.jsoncv-desktop/history.db`.

## Connect an AI agent (MCP)

Settings → AI agents adds the server in one click to Claude Code, Claude Desktop or Codex. For any other agent (Cursor, VS Code, Antigravity and so on), "Copy setup prompt" gives it the details to set itself up.

The app binary is the MCP server. Run it with the `mcp` subcommand (stdio). It opens no window. Inside the bundle, the binary is:

```
/Applications/jsoncv.app/Contents/MacOS/jsoncv-desktop
```

Claude Code:

```sh
claude mcp add jsoncv -- "/Applications/jsoncv.app/Contents/MacOS/jsoncv-desktop" mcp
```

Claude Desktop (`claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "jsoncv": {
      "command": "/Applications/jsoncv.app/Contents/MacOS/jsoncv-desktop",
      "args": ["mcp"]
    }
  }
}
```

The server reads the library path from the app's Settings. To override it, add `--dir <path>` after `mcp`. The app's Settings view shows the exact snippet for your install path.

## Data safety

- The library can live in iCloud Drive or Dropbox. Each Mac keeps its own history file.
- Back up the history file with Time Machine or by copying it.
- Only CVs are versioned. Themes and assets are not.

## Themes

Themes live in `<library>/themes/<theme-id>/`. Built-in themes (`builtin:classic`, `builtin:two-column`) are read-only. Use "Duplicate…" under Settings → Themes to copy one into your library. The theme authoring guide is an MCP resource, so an agent can write themes. In templates, pass URLs through the `safe_url` filter.

## Releases

GitHub Actions builds on tags matching `v*` (macOS, Windows, Linux) and drafts a release. Builds are unsigned: on macOS, right-click the app and choose Open the first time.

## License note

jsoncv-desktop validates CVs against the [jsoncv](https://github.com/reorx/jsoncv) schema. That repository has no license file. Most of the schema comes from JSON Resume (MIT). No jsoncv code or theme is copied here. Bundling the schema in a public release needs the jsoncv author's permission.
