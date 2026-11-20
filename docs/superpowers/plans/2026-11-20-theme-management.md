# Theme Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make themes agents-first. The app adds New theme (name + start from), Delete (to Trash), Open in editor (split button), and a "How do I create a theme?" pop-up. It adds no theme editor.

**Architecture:** Core (`jsoncv_core`) gets the theme file operations. The Tauri app (`src-tauri`) gets thin commands plus a new `editors.rs` with a fixed editor list and per-OS detection and launch. The UI (`ui/src`) changes the Settings → Themes section and the toolbar Theme menu, and adds a `ThemeGuide` dialog.

**Tech Stack:** Rust (Tauri 2, minijinja, pulldown-cmark, new `trash` crate), React 19 + TypeScript, vitest.

**Spec:** `docs/superpowers/specs/2026-11-20-theme-management-design.md`

## Global Constraints

- No in-app template or CSS editing, no rename, and no `delete_theme` MCP tool.
- Ids that start with `builtin:` are read-only. Core refuses to delete them with `CoreError::ReadOnly`.
- Theme ids match `THEME_ID` (`ui/src/lib/platform.ts`): `/^[a-z0-9][a-z0-9-]{0,63}$/`.
- Delete moves the folder to the OS Trash (`trash` crate). It never calls `remove_dir_all`.
- Editors open by a fixed id from `EDITORS` only. The UI never sends a command or a path. The folder comes from `Library::theme_dir`.
- Editor detection: macOS uses the bundle id. Windows and Linux use the CLI on `PATH`, and Windows also honours `PATHEXT`.
- The guide text has one source: `THEME_GUIDE` (`crates/core/resources/theme-guide.md`).
- The default open action starts as Reveal, then becomes the last-used action. It is stored in `localStorage` key `theme-open-with`, inside try/catch.
- Shell setup for Rust: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"`. CI runs `cargo fmt --check`.
- Shared checkout: before each commit, run `git status` and stage only the files this task touched.

## Review Focus

1. **A name with no usable characters** ("!!!", "日本語") gives an empty id. Create is disabled and shows "Use at least one letter or digit." The test is in Task 2 (`themeIdFromName('!!!') === ''`, plus a manual check).
2. **A saved default editor that is no longer installed.** The main button falls back to Reveal. The test is in Task 3 (`openAction`).
3. **An editor launch that fails**, such as `open -b` exiting non-zero. The user sees an error toast, not silence. The test is in Task 3 (`launch_status` maps a non-zero exit to `Err`).
4. **The toolbar after you delete the selected theme.** The trigger shows "Classic" with Classic checked, not the raw id. Task 2 owns this as a manual check.
5. **Open or delete on a built-in id sent from outside the UI.** Core refuses both. The test is in Task 1 (`delete_theme_refuses_builtin_and_unknown`). The `theme_dir` BadName error already covers open.

---

### Task 1: Core and commands for New theme name and Delete

**Files:**
- Modify: `crates/core/src/render.rs` (`duplicate_theme`, add `delete_theme`)
- Modify: `crates/core/Cargo.toml` (add `trash = "5"`)
- Modify: `crates/core/tests/coverage.rs` (update the three `duplicate_theme` calls, add tests)
- Modify: `src-tauri/src/commands.rs` (`duplicate_theme` gains `name`, add `delete_theme`), `src-tauri/src/app.rs` (register)
- Modify: `ui/src/api.ts`, `ui/src/dev/mockTauri.ts`

**Interfaces:**
- Produces (Rust): `Library::duplicate_theme(&self, id: &str, new_id: &str, name: Option<&str>, _author: Author) -> Result<ThemeInfo, CoreError>`. With `name: None`, the name is still `"{src} copy"`.
- Produces (Rust): `Library::delete_theme(&self, id: &str, _author: Author) -> Result<(), CoreError>`
- Produces (TS): `duplicateTheme(id: string, newId: string, name?: string): Promise<ThemeInfo>`, invoked as `{ id, newId, name: name ?? null }`
- Produces (TS): `deleteTheme(id: string): Promise<void>`, invoking `'delete_theme'` with `{ id }`

- [ ] **Step 1: Write the failing tests in `crates/core/tests/coverage.rs`**

```rust
#[test]
fn duplicate_theme_uses_given_name() {
    let (_d, lib) = lib();
    let info = lib.duplicate_theme("builtin:classic", "navy", Some("Navy"), Author::You).unwrap();
    assert_eq!(info.meta.name, "Navy");
    assert_eq!(lib.get_theme("navy").unwrap().info.meta.name, "Navy");
}

#[test]
fn delete_theme_refuses_builtin_and_unknown() {
    let (_d, lib) = lib();
    assert!(matches!(lib.delete_theme("builtin:classic", Author::You), Err(CoreError::ReadOnly(_))));
    assert!(matches!(lib.delete_theme("nope", Author::You), Err(CoreError::NotFound(_))));
    assert!(matches!(lib.delete_theme("../x", Author::You), Err(CoreError::BadName(_))));
}
```

In the existing tests, change the `duplicate_theme(a, b, Author::You)` calls to `duplicate_theme(a, b, None, Author::You)`. The `"Classic copy"` assert stays.

The success path of delete is not unit-tested, because it would put temp folders in the real Trash. Task 2 checks it manually.

- [ ] **Step 2: Run and confirm the tests fail.** Run `cargo test -p jsoncv_core --test coverage`. Expected: compile error (wrong argument count, no `delete_theme`).

- [ ] **Step 3: Implement.** In `duplicate_theme`, use `name.map(str::to_string).unwrap_or_else(|| format!("{} copy", src.info.meta.name))`. For `delete_theme`:
  1. A `builtin:` prefix returns `ReadOnly("{id} is a built-in theme")`.
  2. `theme_dir(id)?` gives BadName for bad ids.
  3. Inside `self.locked(|tx| { self.sweep_external(tx)?; ... })`, a missing dir returns `NotFound(format!("Theme {id}"))`. Otherwise run `trash::delete(&dir)`, mapped to `CoreError::Io(std::io::Error::other(e.to_string()))`.
- [ ] **Step 4: Add the commands.** `duplicate_theme(state, id, new_id, name: Option<String>)` passes `name.as_deref()`. `delete_theme(state, id: String) -> Res<()>` calls `l.delete_theme(&id, Author::You)`. Register both in `app.rs` `generate_handler!`. In `api.ts`, update `duplicateTheme` and add `deleteTheme`. In `mockTauri.ts`, make `duplicate_theme` use `a.name ?? a.newId` as the name, and make `delete_theme` remove the theme from `themes`.
- [ ] **Step 5: Run and confirm the tests pass.** Run `cargo test -p jsoncv_core --test coverage && cargo build -p jsoncv-desktop && (cd ui && npx tsc -b)`. Expected: all pass, no type errors.
- [ ] **Step 6: Commit** with `feat(core): delete theme to Trash; duplicate takes a name`.

---

### Task 2: New theme dialog, Delete button and header copy (UI)

**Files:**
- Create: `ui/src/lib/themes.ts`, `ui/src/lib/themes.test.ts`
- Modify: `ui/src/components/ThemesView.tsx` (`ThemeRow`), `ui/src/components/Settings.tsx` (themes section), `ui/src/components/Toolbar.tsx` (stale-id fallback), `ui/src/components/Icon.tsx` (add `trash`), `ui/src/styles.css`, `README.md` (Themes section)

**Interfaces:**
- Consumes: `duplicateTheme(id, newId, name)` and `deleteTheme(id)` from Task 1.
- Produces: `themeIdFromName(name: string): string` in `ui/src/lib/themes.ts`.
- Produces: `ThemeRow` props become `{ theme: ThemeInfo; reloadKey: number; usedBy: string[]; onDuplicate: () => void; onDeleted: () => void; onError: (e: unknown) => void }`. Task 3 adds `editors` and `openWith` props to it.
- Produces: a `NewTheme` component in `ThemesView.tsx` with props `{ themes: ThemeInfo[]; from: string; onCreated: (t: ThemeInfo) => void; onCancel: () => void; onError: (e: unknown) => void }`.

- [ ] **Step 1: Write the failing test `ui/src/lib/themes.test.ts`**

```ts
import { describe, expect, it } from 'vitest'
import { themeIdFromName } from './themes.ts'
import { THEME_ID } from './platform.ts'

describe('themeIdFromName', () => {
  it('slugs names', () => {
    expect(themeIdFromName('Navy')).toBe('navy')
    expect(themeIdFromName('  My Cool Theme! ')).toBe('my-cool-theme')
    expect(themeIdFromName('Ünïcode Café')).toBe('unicode-cafe')
    expect(themeIdFromName('2 Column')).toBe('2-column')
  })
  it('returns empty when nothing is usable', () => {
    expect(themeIdFromName('!!!')).toBe('')
    expect(themeIdFromName('日本語')).toBe('')
  })
  it('caps at 64 with no trailing hyphen', () => {
    const id = themeIdFromName('a'.repeat(63) + ' b')
    expect(id.length).toBeLessThanOrEqual(64)
    expect(id.endsWith('-')).toBe(false)
    expect(THEME_ID.test(id)).toBe(true)
  })
})
```

- [ ] **Step 2: Run and confirm it fails.** Run `cd ui && npx vitest run src/lib/themes.test.ts`. Expected: FAIL, module not found.
- [ ] **Step 3: Implement `themeIdFromName`.** Steps: NFKD, strip combining marks, lowercase, turn runs of non-`[a-z0-9]` into `-`, trim `-`, slice to 64, trim the trailing `-` again.
- [ ] **Step 4: Run and confirm it passes.** Same command. Expected: PASS.
- [ ] **Step 5: Build the UI.**
  - **Settings → Themes.** The header row holds the lead copy, a **How do I create a theme?** button and a **New theme…** button. Task 4 wires the guide button. Until then, render it disabled.
  - **Lead copy (exact).** "Themes are files: a template and a stylesheet. The easiest way to make one is to ask your agent. You can also open a theme in your editor."
  - **New theme state.** Settings holds `newFrom: string | undefined`. **New theme…** sets it to `DEFAULT_THEME`. A row's **Duplicate…** sets it to that row's id. When `newFrom` is set, `<NewTheme>` renders above the rows.
  - **`NewTheme` fields.** A **Name** input (autofocus) and a **Start from** `<select>` over `themes`, default `from`. Under the name, show "Folder: themes/{id}" with the derived id.
  - **`NewTheme` validation.** An empty id disables Create and shows "Use at least one letter or digit." An existing id shows "A theme with this folder already exists." Server errors show inline with `errorMessage(e)`.
  - **`NewTheme` keys.** Escape cancels and calls `stopPropagation`, as the dup form does today. Remove the old inline dup form from `ThemeRow`.
  - **Delete.** On non-built-in rows, add an icon button (`trash` icon, `aria-label="Delete theme"`). It asks with `confirm` from `@tauri-apps/plugin-dialog`, or `window.confirm` under `MOCK`, the same as `CvList.tsx:339`. The message is "Move {name} to the Trash?", plus " {n} CV(s) use it and will switch to Classic: {names}." when `usedBy` is not empty. On yes, it calls `deleteTheme`, then `onDeleted`. Settings computes `usedBy` as `cvs.filter(c => c.theme === t.id).map(c => c.name)`.
  - **Toolbar.** In `ThemePicker`, if `value` is not in `themes`, treat it as `DEFAULT_THEME` for the label and the checkmark.
  - **README.** Replace the "Use 'Duplicate…'" sentence with New theme, Open in editor and the guide.
- [ ] **Step 6: Verify.** Run `cd ui && npx tsc -b && npx vitest run && npx oxlint`. Expected: clean. Then run `cargo tauri dev` and check each item by hand:
  1. Create "Navy" from Two column.
  2. The duplicate-name error appears.
  3. "!!!" disables Create.
  4. Delete Navy while a CV uses it. The theme goes to the Trash, and that CV shows Classic in the toolbar.
- [ ] **Step 7: Commit** with `feat(ui): New theme dialog, delete to Trash, agents-first Themes copy`.

---

### Task 3: Open in editor

**Files:**
- Create: `src-tauri/src/editors.rs` (with `#[cfg(test)] mod tests`)
- Modify: `src-tauri/src/main.rs` or the crate root (`mod editors;`), `src-tauri/src/commands.rs`, `src-tauri/src/app.rs`
- Modify: `ui/src/api.ts`, `ui/src/dev/mockTauri.ts`, `ui/src/lib/themes.ts`, `ui/src/lib/themes.test.ts`, `ui/src/components/ThemesView.tsx`, `ui/src/components/Settings.tsx`, `ui/src/styles.css`

**Interfaces:**
- Produces (Rust, `editors.rs`):
  - `pub struct Editor { pub id: &'static str, pub label: &'static str, pub mac: Option<&'static str>, pub cli: Option<&'static str>, pub args: &'static [&'static str] }`. Here `mac` is the bundle id, and `args` go before the folder.
  - `pub const EDITORS: &[Editor]`
  - `pub fn installed() -> Vec<&'static Editor>`
  - `pub fn open(editor_id: &str, dir: &Path) -> std::io::Result<()>`
  - `fn find_in(path_var: &OsStr, exts: &[&str], cmd: &str) -> Option<PathBuf>`
  - `fn launch_status(s: std::process::ExitStatus, label: &str) -> std::io::Result<()>`
- Produces (Rust commands): `list_editors() -> Res<Vec<EditorInfo>>` with `#[derive(Serialize)] struct EditorInfo { id: String, label: String }`, and `open_theme_in(state, editor: String, id: String) -> Res<()>`.
- Produces (TS): `type Editor = { id: string; label: string }`, `listEditors(): Promise<Editor[]>`, `openThemeIn(editor: string, id: string): Promise<void>`, and in `lib/themes.ts` `openAction(stored: string | null, editors: Editor[]): string`, which returns `'reveal'` or an installed editor id.
- Consumes: `ThemeRow` props from Task 2. This task adds `editors: Editor[]`, `openWith: string` and `onOpenWith: (action: string) => void`.

`EDITORS` entries (exact):

| id | label | mac bundle id | cli | args |
|---|---|---|---|---|
| vscode | VS Code | com.microsoft.VSCode | code | |
| cursor | Cursor | com.todesktop.230313mzl4w4u92 | cursor | |
| windsurf | Windsurf | com.exafunction.windsurf | windsurf | |
| zed | Zed | dev.zed.Zed | zed | |
| sublime | Sublime Text | com.sublimetext.4 | subl | |
| nova | Nova | com.panic.Nova | — | |
| bbedit | BBEdit | com.barebones.bbedit | — | |
| terminal | Terminal | com.apple.Terminal | — | |
| iterm | iTerm | com.googlecode.iterm2 | — | |
| wt | Windows Terminal | — | wt | `-d` |

Fleet from the spec is left out. JetBrains has stopped Fleet, and its bundle id is not stable.

- [ ] **Step 1: Write the failing Rust tests in `editors.rs`**

```rust
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
    assert_eq!(find_in(&path, &["", ".cmd"], "code"), Some(d.path().join("code.cmd")));
    assert_eq!(find_in(&path, &[""], "code"), None);
    assert_eq!(find_in(&path, &["", ".cmd"], "zed"), None);
}

#[test]
fn launch_status_reports_failure() {
    use std::process::Command;
    assert!(launch_status(Command::new("false").status().unwrap(), "X").is_err());
    assert!(launch_status(Command::new("true").status().unwrap(), "X").is_ok());
}
```

The `launch_status` test uses Unix `true` and `false`. Gate it with `#[cfg(unix)]`.

- [ ] **Step 2: Run and confirm the tests fail.** Run `cargo test -p jsoncv-desktop editors`. Expected: compile error.
- [ ] **Step 3: Implement `editors.rs`.**
  - **`find_in`.** Check each `split_paths(path_var)` dir and each ext for `dir/{cmd}{ext}`. Return the first that `is_file()`. Exts are `[""]` on Unix. On Windows, split `PATHEXT` on `;`, lowercased, with `""` first.
  - **`installed()` on macOS.** For each editor with a `mac` id, run `mdfind "kMDItemCFBundleIdentifier == '<id>'"`. Non-empty stdout means the editor is installed. Run this only when the list is requested. Add the comment `// shortcut: needs Spotlight; if users report missing editors, fall back to /Applications/*.app Info.plist`.
  - **`installed()` elsewhere.** Keep the editors whose `cli` passes `find_in`.
  - **`open` on macOS.** Run `open -b <mac> <dir>` with `.status()`, which returns quickly, and check it with `launch_status`.
  - **`open` elsewhere.** `spawn()` the resolved CLI path with `args` and then `dir`, and don't wait, because some editors block. A spawn error is the failure signal.
  - **Unknown or unsupported id.** An id that is not in `EDITORS`, or has no route on this OS, returns `io::ErrorKind::NotFound`.
  - **`launch_status`.** Return `Ok` on success. Otherwise return `Err(io::Error::other(format!("Could not open {label}")))`.
- [ ] **Step 4: Add the commands.** `list_editors` runs `blocking` over `editors::installed()`. `open_theme_in` gets `dir` from `with_lib` → `l.theme_dir(&id)`. It returns `NotFound` if the folder is missing, then calls `editors::open(&editor, &dir)` in `blocking`. Register both commands. In `mockTauri.ts`, `list_editors` returns `[{id:'vscode',label:'VS Code'},{id:'zed',label:'Zed'}]`, and `open_theme_in` does nothing.
- [ ] **Step 5: Run and confirm the tests pass.** Run `cargo test -p jsoncv-desktop editors`. Expected: PASS.
- [ ] **Step 6: Add the failing `openAction` test to `themes.test.ts`**

```ts
describe('openAction', () => {
  const eds = [{ id: 'vscode', label: 'VS Code' }]
  it('defaults to reveal', () => expect(openAction(null, eds)).toBe('reveal'))
  it('keeps an installed editor', () => expect(openAction('vscode', eds)).toBe('vscode'))
  it('falls back when the editor is gone', () => expect(openAction('zed', eds)).toBe('reveal'))
})
```

Run `npx vitest run src/lib/themes.test.ts`. It should fail. Then implement `openAction` and run again. Expected: PASS.

- [ ] **Step 7: Build the split button.**
  - **Loading.** Settings loads `listEditors()` once when the Themes section mounts. It reads `localStorage['theme-open-with']` in try/catch and resolves it with `openAction`.
  - **Main part.** It shows `revealLabel` for `'reveal'`, otherwise "Open in {label}". Reveal runs `themeDirPath(id).then(reveal)`. An editor runs `openThemeIn(editor, id)`.
  - **Arrow part.** It opens a `useMenu` menu: Reveal first, a separator, then each editor. Show the `Logo` icon for `vscode` and `cursor`, and the `folder` icon for the rest.
  - **Choosing.** Picking an item runs it and stores it as `theme-open-with`.
  - **Errors.** Errors go to `onError`, which shows the existing toast.
  - **Built-in rows.** These show no split button.
- [ ] **Step 8: Verify.** Run `cargo fmt --check && cargo build -p jsoncv-desktop && (cd ui && npx tsc -b && npx vitest run && npx oxlint)`. Expected: clean. Then run `cargo tauri dev`:
  1. Only installed editors appear.
  2. "Open in VS Code" opens the theme folder.
  3. The choice persists across a restart.
- [ ] **Step 9: Commit** with `feat: open a theme folder in an installed editor`.

---

### Task 4: "How do I create a theme?" pop-up

**Files:**
- Modify: `crates/core/src/render.rs` (markdown helper), `crates/core/tests/builtin_themes.rs` (test)
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/app.rs`
- Create: `ui/src/components/ThemeGuide.tsx`
- Modify: `ui/src/api.ts`, `ui/src/dev/mockTauri.ts`, `ui/src/lib/themes.ts`, `ui/src/App.tsx`, `ui/src/components/Settings.tsx`, `ui/src/components/Toolbar.tsx`, `ui/src/styles.css`

**Interfaces:**
- Produces (Rust): `pub fn markdown_html(text: &str) -> String` in `render.rs`. It renders with `Options::ENABLE_TABLES` and applies the same escaping of raw HTML and the same `safe_url` rule as the template `markdown` filter. Factor the shared event mapping into one private fn. The template filter keeps `Options::empty()`.
- Produces (command): `theme_guide_html() -> String` returns `markdown_html(THEME_GUIDE)`.
- Produces (TS): `themeGuideHtml(): Promise<string>`, and `themePrompt(cv: string | null): string` in `lib/themes.ts`.
- Produces (UI): `<ThemeGuide cv={string | null} onAgents={() => void} onClose={() => void} />`. App holds `guideOpen: boolean`.
- Consumes: the disabled guide button from Task 2. Settings gains an `onGuide: () => void` prop. `ThemePicker`/`Toolbar` gain `onGuide: () => void`.

- [ ] **Step 1: Write the failing test in `crates/core/tests/builtin_themes.rs`**

```rust
#[test]
fn theme_guide_renders_tables_and_code_safely() {
    let html = jsoncv_core::render::markdown_html(THEME_GUIDE);
    assert!(html.contains("<table>"));
    assert!(html.contains("<pre><code"));
    assert!(html.contains("format_date"));
    let evil = jsoncv_core::render::markdown_html("<script>x</script> [a](javascript:alert(1))");
    assert!(!evil.contains("<script>"));
    assert!(!evil.contains("javascript:"));
}
```

- [ ] **Step 2: Run and confirm it fails.** Run `cargo test -p jsoncv_core --test builtin_themes`. Expected: compile error, no `markdown_html`.
- [ ] **Step 3: Implement `markdown_html`.** Run the existing render tests too: `cargo test -p jsoncv_core`. Expected: all PASS, including `markdown_strips_javascript_links` and `markdown_renders_bold_and_escapes_html`.
- [ ] **Step 4: Add the command, the API and a mock.** The mock returns `'<h1>Writing a jsoncv theme</h1><p>…</p>'`.
- [ ] **Step 5: Implement `themePrompt`.** Use this exact copy. Use "my CV" when `cv` is null.

```ts
`Using the jsoncv MCP server, read jsoncv://theme-guide, then create a theme called <name> that looks like: <describe it>. Fix every render_error, then set it as the theme for my CV "${cv}".`
```

- [ ] **Step 6: Build `ThemeGuide`.**
  - **Dialog.** It uses `.scrim` and `.dialog`, the same as Settings. Escape and a scrim click call `onClose`.
  - **Section 1, "Ask your agent".** Add a "(recommended)" badge and one line: "An agent can write a theme, check which fields it hides, and preview it." Then a **Copy prompt** button (reuse `Copy` from `Settings.tsx`, exported) and a **Set up an agent** link that calls `onAgents`. The spec says to show this link only when no agent is connected. The app cannot tell whether an agent is connected, so it always shows the link.
  - **Section 2, "Edit the files yourself".** "Click New theme…, then open it in your editor. The preview reloads when you save."
  - **Section 3, "Reference".** The `themeGuideHtml()` output in a scrollable `div` with `dangerouslySetInnerHTML`. This is safe: the source is bundled and the renderer escapes raw HTML.
  - **App wiring.** `onAgents` closes the guide and runs `setSettingsOpen('agents')`. App renders `ThemeGuide` after `Settings`, so it stacks on top and its Escape does not bubble into the Settings scrim.
  - **Entry points.** Enable the Settings guide button (`onGuide`). In `ThemePicker`, add a menu item "How to create a theme…" under "Manage themes…".
- [ ] **Step 7: Verify.** Run `cargo fmt --check && cargo test --workspace && (cd ui && npx tsc -b && npx vitest run && npx oxlint)`. Expected: clean. Then run `cargo tauri dev`:
  1. Open the guide from the toolbar and from Settings.
  2. Check that the table renders.
  3. Click Copy prompt, paste, and check that the current CV name is in it.
  4. Click Set up an agent and check that it opens Settings → Agents.
  5. Press Escape on the guide over Settings. Only the guide closes.
- [ ] **Step 8: Commit** with `feat: theme guide pop-up that leads with agents`.
