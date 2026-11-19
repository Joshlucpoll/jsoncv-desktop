<img src="assets/icon.svg" width="64" height="64" alt="">

# jsoncv-desktop

## What it is

jsoncv-desktop is a macOS app (Tauri 2) for keeping your CV as plain JSON. A theme turns the JSON into HTML or PDF. Every change to a CV is saved as a version, so you can undo anything. One main CV gives rise to tailored copies, one per job. An AI agent can edit everything over MCP.

## Features

- Edit a CV as a form or as JSON, with live preview and schema validation.
- Swappable themes (Jinja template + CSS), with a field-coverage warning.
- Undo and redo (⌘Z / ⇧⌘Z) for every saved change, yours or an agent's.
- Tailored CVs as independent copies of the main CV.
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
  main.cv.json
  tailored/<slug>.cv.json
  assets/                   # CV files: photo, logos
  themes/<theme-id>/
    template.html.j2
    style.css
    theme.json              # { name, description, page: { size: "A4" | "Letter" } }
    assets/                 # theme files: fonts, icons (optional)
  exports/
  .jsoncv-id                # links the folder to its history
```

History is kept outside the library, in one SQLite file. The default is
`~/Library/Application Support/dev.jsoncv.jsoncv-desktop/history.db`. Change its folder in Settings.

## Connect an AI agent (MCP)

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
- Keep the history file out of synced folders if two Macs use it. Sync clients can corrupt a database written from two places.
- Back up the history file with Time Machine or by copying it.
- Only CVs are versioned. Themes and assets are not.
- A running MCP server keeps the history path it started with. Restart it after you move the history file.

## Themes

Themes live in `<library>/themes/<theme-id>/`. Built-in themes (`builtin:classic`, `builtin:two-column`) are read-only. Use "Duplicate…" under Settings → Themes to copy one into your library. The theme authoring guide is an MCP resource, so an agent can write themes. In templates, pass URLs through the `safe_url` filter.

## Releases

GitHub Actions builds on tags matching `v*` (macOS, Windows, Linux) and drafts a release. Builds are unsigned: on macOS, right-click the app and choose Open the first time.

## License note

jsoncv-desktop validates CVs against the [jsoncv](https://github.com/reorx/jsoncv) schema. That repository has no license file. Most of the schema comes from JSON Resume (MIT). No jsoncv code or theme is copied here. Bundling the schema in a public release needs the jsoncv author's permission.
