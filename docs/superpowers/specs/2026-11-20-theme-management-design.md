# Theme management: agents first

Date: 2026-11-20. Size: a small change.

## Intent

The app shows themes and manages them. It does not edit them. An agent (over MCP) or the user's own editor makes the real changes. Today the app hands the user off at "Reveal in Finder" with no explanation. This change makes that limit clear and steers people to agents.

## Goals

1. **New theme…**: give it a name, pick a theme to start from (default: Classic), and the app creates a copy. This replaces the separate Duplicate flow. Each row's "Duplicate…" opens the same dialog with that theme preselected.
2. **Open in editor**: a split button on each custom theme. The main part runs the default action. The arrow lists "Reveal in Finder/Explorer" and every installed editor from a known list.
3. **How do I create a theme?**: a button that opens a pop-up with three routes: ask your agent, edit the files yourself, and the full reference guide.
4. **Delete**: removes a custom theme after a confirm step.

## Non-goals

- No in-app template or CSS editor, and no colour or font controls.
- No theme marketplace. That is a separate project, to design later. The theme folder is already a self-contained package, so nothing here blocks it.
- No rename. To rename a theme, edit `theme.json` or ask an agent.
- No `delete_theme` MCP tool. Themes have no version history, so an agent should not delete them.

## Anti-goals

- The app must not look like a theme editor. If a user expects to restyle a CV inside the app, this change has failed.

## Approaches (editor list)

| Approach | Trade-off |
|---|---|
| **A. Known list, detect installed (chosen)** | Clean names, and every entry opens a folder correctly. A new editor needs a list update. |
| B. Ask the OS for every app that opens folders | No list to keep, but the list is noisy (archivers, viewers, Finder). |
| C. A + "Other app…" picker | Covers editors that are not on the list. One more control few people use. Can be added later. |

Josh chose A. It must work on all platforms, not only macOS.

## Design

### New theme dialog (UI)
- Fields: **Name** (free text) and **Start from** (a select with every theme, default `builtin:classic`).
- The app derives the id from the name: lowercase, non-alphanumerics become `-`, trimmed, max 64 characters, and it must match `THEME_ID`. The id shows under the field ("Folder: themes/navy"). An id that already exists shows an inline error.
- The dialog calls `duplicate_theme(from, id, name)`. Core `duplicate_theme` gets an optional `name`. Without one, it keeps today's "`<name>` copy".
- Entry points: a "New theme…" button in the Settings → Themes header, and each row's "Duplicate…".

### Open in editor (Rust + UI)
- New `src-tauri/src/editors.rs` holds a fixed list. Each entry has an id, a label, a macOS bundle id, a CLI command and an optional Windows-only or macOS-only flag. Entries: VS Code, Cursor, Windsurf, Zed, Sublime Text, Nova, BBEdit, Fleet, Terminal, iTerm, Windows Terminal.
- Detection: macOS looks up the bundle id. This works without the editor's shell command, which many users never install. Windows and Linux look for the CLI command on `PATH`.
- Opening: macOS runs `open -b <bundle id> <dir>`. Windows and Linux run `<cli> <dir>`. Windows Terminal runs `wt -d <dir>`.
- Commands: `list_editors()` returns `[{id, label}]` for installed editors only. `open_theme_in(editor_id, theme_id)` takes an editor id from the fixed list and resolves the folder with `theme_dir`, so the UI cannot run an arbitrary command or open an arbitrary path.
- UI: a split button on each custom theme row. The main part runs the last-used action, and the first default is Reveal. The last-used action is stored in `localStorage` (a per-machine convenience, with try/catch as elsewhere). The arrow menu uses `useMenu`, with Reveal first and then the editors. The menu shows editor icons where `Logo` has them.
- Built-in themes have no folder, so their rows keep only "Duplicate…".

### "How do I create a theme?" pop-up (UI + Rust)
- Entry points: a button in the Settings → Themes header, and a "How to create a theme…" item in the toolbar Theme menu, under "Manage themes…".
- Content, in order:
  1. **Ask your agent** (the recommended route). One line on what an agent can do: create a theme, check field coverage and preview it. A **Copy prompt** button copies a ready-made prompt: "Using the jsoncv MCP server, read jsoncv://theme-guide, then create a theme called <name> that looks like: <describe it>. Fix every render_error, then set it as the theme for my CV <current CV>." If no agent is connected, a link goes to Settings → Agents.
  2. **Edit the files yourself**: New theme… → Open in editor. The preview reloads when you save.
  3. **Reference**: the full theme guide.
- The guide text comes from `THEME_GUIDE` (`crates/core/resources/theme-guide.md`), the same file agents read, so there is one source. A new command `theme_guide_html` renders it with the core markdown renderer (`render.rs` `markdown`, exposed as a small helper).

### Delete (Rust + UI)
- Core `delete_theme(id)` refuses `builtin:` ids. It removes the folder under the library lock and moves it to the OS Trash with the `trash` crate. Themes have no version history, so a plain `remove_dir_all` could lose work.
- UI: a delete icon on custom theme rows. The confirm message names the CVs that use the theme and says they will switch to Classic. They already fall back to Classic when the theme is missing (`render.rs` `resolve_theme`). Use the dialog plugin `confirm`, the same as CV delete (`CvList.tsx`).

### Settings → Themes header copy
Replace the lead text with: "Themes are files: a template and a stylesheet. The easiest way to make one is to ask your agent. You can also open a theme in your editor." Then the two buttons.

## Calls Claude made (check these)
- New theme derives the id from a name instead of asking for an id.
- Delete moves to Trash and adds the `trash` dependency.
- No delete over MCP.
- The guide pop-up is also linked from the toolbar Theme menu.
- The first default action is Reveal, then the last-used action.

## Left to the builder
- How to look up a bundle id on macOS (Launch Services or `mdfind`). Run it once per Settings open, not on every render.
- Windows CLI shims such as `code.cmd`, which `Command::new("code")` does not resolve. Resolve the path through `PATHEXT` or run it through `cmd /C`.
- The prompt wording and the pop-up layout.

## Tests
- Core: `delete_theme` refuses built-in ids and unknown ids. `duplicate_theme` with a name writes that name to `theme.json`.
- UI: the name-to-id function (vitest, next to `locate.test.ts`).
- Manual check on macOS: open in VS Code and Reveal. Windows and Linux paths compile but nobody has run them.
