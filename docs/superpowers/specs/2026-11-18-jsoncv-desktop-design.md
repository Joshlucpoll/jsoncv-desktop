# jsoncv-desktop — design

Date: 2026-11-18 · Status: draft for approval

## 1. Intent

A desktop app for writing, previewing, versioning and exporting CVs stored in the
[jsoncv format](https://github.com/reorx/jsoncv/blob/master/schema/jsoncv.schema.json).

- **Who it is for:** the author, for their own CV. It may be open-sourced later. Keep the code clean enough to publish. Do not build distribution features yet.
- **How it is used:** an AI agent does most of the editing over MCP: it writes CV content, tailors CVs for jobs and creates themes. The user reads the preview and tweaks fields by hand.
- **Why:** the CV is plain JSON. The look comes from swappable themes. Every change can be undone. One main CV gives rise to tailored copies, one per job application.

## 2. Goals, non-goals, anti-goals

**Goals**

1. Show a live preview of a CV with a chosen theme. The preview updates when the file changes on disk, whoever changed it.
2. Hand-edit any field through a form generated from the jsoncv schema, plus a raw JSON tab.
3. Keep one main CV and any number of tailored CVs. A tailored CV is an independent copy.
4. Keep an undo history for every CV, with view, diff and restore.
5. Ship built-in themes. Let the user and the agent create new themes as template + CSS.
6. Warn when a theme does not render some schema fields.
7. Export to PDF, HTML (one self-contained file) and JSON. Also print through the system print dialog.
8. Provide an MCP server so an external agent (Claude Code, Claude Desktop, any MCP client) can do everything in goals 3–7.
9. Run on macOS. Build for Windows and Linux in CI.

**Non-goals (for now)**

- Code signing, notarisation, auto-update, onboarding.
- An in-app AI chat. Agents connect from outside over MCP.
- Bundling or downloading Chrome.
- Word (.docx) export.
- "Tailored CV = main CV + overrides". Tailored CVs are copies.
- Sync or cloud features. **Putting the library in iCloud Drive, Dropbox or a similar synced folder is not supported**: those services can corrupt the `.git` folder. The user backs up by pushing the library's Git repo to a private remote. If the chosen library path is inside a known sync folder (`~/Library/Mobile Documents`, `~/Library/CloudStorage`, `~/Dropbox`), Settings shows a warning but does not block the choice.

**Anti-goals (the app fails if it does these)**

- Loses or corrupts CV data, including when the app and an agent write at the same time.
- Saves a CV that fails the jsoncv schema. Show the error and keep the last valid file.
- Hides the user's data in a database or a proprietary format. Everything stays as plain files.
- Requires the user to know Git.

## 3. Constraints

- Rust does all core work. Tauri 2 hosts the UI.
- macOS first. Windows and Linux must build from the same code in GitHub Actions. Do not plan cross-compiling from macOS.
- Rust is not installed on the dev machine yet. Setup is part of the work.
- reorx/jsoncv has **no license file**. Do not copy its theme or code. You may use the schema for validation now; bundling it in a public release needs the author's permission. (Most of the schema comes from JSON Resume, which is MIT.)
- PDF export depends on an installed Chromium browser (Chrome, Edge, Chromium, Brave and similar). No bundled browser.

## 4. Approaches considered

| Decision | Options considered | Chosen | Why |
|---|---|---|---|
| App framework | **Tauri + React UI**; Dioxus (UI in Rust, same webview); native Rust GUI (egui/iced) | Tauri + React | An existing library (`@rjsf/core`) generates the edit form from the schema for free. Tauri is the most mature option. A native GUI can't show HTML themes without embedding a webview. |
| AI integration | **External agent over MCP**; chat panel inside the app with an API key | MCP server | The user already works in agent tools. No chat UI, streaming or key storage to build. |
| Versioning model | **Tailored CV = independent copy**; tailored CV = main + overrides | Copy | A tailored CV is a record of what was sent. The agent can re-tailor from main when needed. Overrides are much harder and break when main changes. |
| History storage | **Git repo in the CV folder** (through a library, no git CLI); snapshot files | Git | Compact history and diffs. The user can push the folder to a private remote as a backup. Hidden from the user. *(Builder's proposal, accepted without objection.)* |
| Theme format | **Jinja template + CSS**; CSS only over fixed markup; Typst | Template + CSS | Full layout freedom (sidebars, section order). Agents write HTML and CSS well. Typst was rejected because agents write it less well. |
| PDF export | **Installed Chrome, headless**; native webview print per OS; Typst; WeasyPrint/Prince | Installed headless Chrome, **plus** an in-app print dialog | Agents can export with the app closed. Output is the same every time. Adds 0 MB to the install. The print dialog covers paper and printer choices. |
| Layout | Three panes; **preview first with a slide-in edit panel**; one pane with tabs | Preview first | The AI makes most edits, so the preview gets the space. |

## 5. Design

### 5.1 Process and code layout

One Cargo workspace:

- `crates/core`: a library with no UI. It owns the CV folder, validation, Git history, theme rendering, the field-coverage check and export. The app and the MCP server both call it. Nothing else touches the files.
- `src-tauri`: the Tauri app. Its commands are thin wrappers over `core`.
- `ui/`: React + TypeScript (Vite).
- **The MCP server is the app binary run with a subcommand**: `jsoncv-desktop mcp` (stdio). The binary checks its arguments before it creates any window. In MCP mode it never opens a window. The user registers this command in their agent's MCP config.

### 5.2 CV folder ("library")

Default path is `~/Documents/CVs`. The user can change it in Settings. The setting lives in the app's config file (the Tauri app config dir). The MCP server reads the same setting. `--dir <path>` overrides it.

```
<library>/
  main.cv.json
  tailored/<slug>.cv.json
  assets/                   # CV files: photo, logos (versioned)
  themes/<theme-id>/
    template.html.j2
    style.css
    theme.json              # { name, description, page: { size: "A4" | "Letter" } }
    assets/                 # theme files: fonts, icons (optional)
  exports/                  # in .gitignore
  .gitignore
  .git/
```

- On first run, if the folder has no `main.cv.json`, create one from a sample CV (written by us, not copied from jsoncv) and initialise the Git repo.
- A CV's chosen theme is stored in the CV itself as `meta.theme` (the schema says `meta` is for tooling config and allows extra properties). A tailored CV also stores `meta.tailoredFrom: { cv: "main", commit: "<sha>" }`.
- Built-in themes ship inside the app binary and are read-only. "Duplicate" copies one into `themes/`. Theme ids: built-ins use a `builtin:` prefix (`builtin:classic`). Folder themes use the folder name.
- **Local files:**
  - A CV refers to a library file by a path relative to the library, for example `"image": "assets/photo.jpg"`. URLs still work as before.
  - A theme's CSS or template refers to its own files relative to its folder, for example `url("assets/Inter.woff2")`.
  - At render time, `core` replaces every local reference with a `data:` URI. That one step makes the preview, HTML export and headless Chrome all work, with no asset protocol or file base URL.
  - A path that leaves its folder (`..`, an absolute path) is rejected, so a CV or theme can't read arbitrary files.
  - The edit form shows a file picker for `basics.image`. It copies the chosen file into `assets/`.
  - MCP adds `add_asset(path_or_base64, name)`, so an agent can put a file into `assets/`.
- The app watches the folder (the `notify` crate). Any change on disk refreshes the CV list, the preview and the open form.

### 5.3 Validation and writes

- Every write goes through `core`. `core` validates against the jsoncv schema, writes atomically (temp file + rename), then commits.
- On validation failure, `core` writes nothing and returns the errors with their JSON paths. The UI shows them and the MCP tools return them.
- If a file was edited outside the app and is now invalid JSON or fails the schema, the app shows the error and keeps showing the last valid version. It never overwrites the file on its own.

### 5.4 History

- One Git repo per library, through the `git2` crate (vendored libgit2, so no git CLI is needed).
- **Commit authors** show who made a change: `You` (the app) or `Agent` (MCP). The agent's tools take an optional `message`. The history list shows, for example, "Agent: tightened summary · 2m" and "You: edited work[2] · 1h".
- **Every write commits at once**, from the app or from MCP. The form saves 1 s after the last keystroke, and that save is a commit. No changes stay uncommitted on disk, so another process can never attribute them to the wrong author. *(Builder's call.)*
- **The history list groups commits:** consecutive commits by the same author within 5 minutes show as one entry ("You: edited work, skills · 3m"). Expanding the entry shows the single commits.
- **Changes made outside `core` get an `External` commit.** Examples are an agent using its own file tools, a text editor, or a sync service. `core` checks for uncommitted changes before every write and commits them first as `External`, if they are valid. While the app is open, its file watcher does the same, 2 s after a change. This works even when the app is closed, because the check lives in `core`. *(Builder's call.)*
- History is per file: `git log -- <file>`. Restoring a version writes that old content as a **new** commit. History is never rewritten.
- A diff view shows the JSON difference between any version and the current one. A text diff of pretty-printed JSON with stable key order is enough.
- Writes from the app and from MCP can happen at the same time. `core` holds a lock file in `.git/` while it writes and commits. It retries for a short time, then reports an error.
- **No writer silently overwrites another (builder's call):**
  - Every write except `replace_cv` is a **JSON Patch applied to the latest file on disk**, never to a stale copy.
  - For a hand edit, the app computes the patch: it diffs its base version against the user's draft (`json-patch` `diff`). Two edits to different fields then both survive.
  - `replace_cv` must pass the `base` hash it got from `get_cv`. `core` rejects the write if the file has changed since, and the agent re-reads and retries.
  - If a patch can't apply (for example, the agent deleted the item the user was editing), the app keeps the user's draft and shows a banner: "This CV changed while you were editing", with the buttons "Show changes", "Discard my edits" and "Overwrite".
  - The app never throws away an unsaved draft because of a disk refresh. A draft that is still invalid stays in the panel and is marked as unsaved.

### 5.5 Themes and rendering

- Templates use **minijinja**. The context is `cv` (the CV JSON) plus a few helpers: `format_date(value, fmt)` (handles `YYYY`, `YYYY-MM` and `YYYY-MM-DD`), `markdown(text)` and `strip_scheme(url)`. Rendering produces one HTML document. `style.css` is inlined into it.
- The preview shows the rendered HTML in an iframe inside the app's webview, sandboxed, with scripts disabled for theme content. A theme's own `@page` CSS sets page size and margins.
- A render error (bad template) shows in place of the preview with the template line number. The MCP tools return it.
- **Built-in themes:** at least two, written from scratch: `classic` (one column) and `two-column` (sidebar). A third, `minimal`, is a nice-to-have.
- **Theme authoring guide:** a markdown document shipped in the binary. It lists the context, the helpers, the CSS conventions (`@page`, print rules) and one full built-in theme as an example. MCP serves it as a resource.

### 5.6 Field-coverage warning

When a theme is saved (from the app or MCP), and when a theme is viewed in the app:

1. Build a **probe CV** from the schema in which every field, including every array-item field, has a value.
2. Render the template with the probe CV wrapped in a minijinja `Object` that records each path read (for example `work[].highlights`).
3. Report every schema path the template never read, grouped by section. A section with no reads is reported as one entry (`awards (whole section)`).

Known limit: "read" is not the same as "shown". A field that only appears in an `{% if %}` counts as covered. That is acceptable.
Fallback, if path tracking through minijinja proves impractical: fill the probe CV with unique marker strings and search the rendered HTML for them. Dates need special handling there.

### 5.7 Export

- **Export PDF:** `core` renders the HTML and drives the installed Chromium browser headless (`chromiumoxide` crate) to print it to `exports/<cv>-<theme>.pdf`. Page size comes from the theme. `core` finds the browser in standard install paths on each OS. If it finds none, the app says so and offers Print….
- **Print…:** calls the webview's print for the preview frame. This opens the system dialog (on macOS it includes "Save as PDF").
- **HTML:** writes the rendered self-contained HTML. Local files are already inlined (§5.2). Images given by URL stay as URLs.
- **JSON:** copies the CV file.
- The export menu offers "Reveal in Finder" after an export.

### 5.8 App UI (layout B, "preview first")

```
┌─────────────┬──────────────────────────────┬──────────────┐
│ CVs         │                              │ (Edit panel, │
│  Main CV    │        Live preview          │  slides in   │
│  ↳ Stripe   │        (paged, zoomable)     │  with ⌘E)    │
│  ↳ Monzo    │                              │ Form | JSON  │
│ History     │                              │              │
│  Agent: …   │                              │              │
│  You: …     │                              │              │
└─────────────┴──────────────────────────────┴──────────────┘
 Toolbar: CV name · Theme picker ▾ · Edit ⌘E · Export ▾ (PDF, HTML, JSON, Print…)
```

- **Sidebar:** the CV list shows tailored CVs nested under main, with "New tailored CV…" (copies main) and rename/delete. Under it is the history of the selected CV. Clicking a history entry previews that version, with buttons for "Show diff" and "Restore".
- **Edit panel:** a Form tab (`@rjsf/core`, generated from the schema, one collapsible section per top-level key) and a JSON tab (a code editor; invalid JSON is not saved).
- **Theme picker:** built-in and library themes. Selecting a theme sets `meta.theme`. A "Themes" view lists themes and shows the coverage warning, with "Duplicate" and "Reveal in Finder" buttons. Editing theme files by hand happens in the user's own editor; the app reloads them on change.
- Light and dark mode follow the system.

### 5.9 MCP server

Stdio transport, using the official Rust MCP SDK (`rmcp`). It runs `core` in-process.

**Tools**

| Tool | Does |
|---|---|
| `list_cvs` | Names, paths, themes, last modified. |
| `get_cv(name)` | The full CV JSON and its `base` hash. |
| `update_cv(name, patch, message?)` | Applies an RFC 6902 JSON Patch. Validates, writes, commits as Agent. Returns the errors or the new commit. |
| `replace_cv(name, cv, base, message?)` | Full replacement, for large rewrites. Same checks, and rejected if `base` is stale. |
| `create_tailored_cv(name, from?="main")` | Copies a CV into `tailored/`. |
| `cv_history(name, limit?)` / `restore_cv_version(name, commit)` | Lists or restores versions. |
| `list_themes` / `get_theme(id)` | Theme metadata, or its template, CSS and metadata. |
| `save_theme(id, template, css, meta)` | Writes a library theme (built-ins can't be overwritten). Returns render errors and the coverage warning. |
| `set_cv_theme(name, theme)` | Sets `meta.theme`. |
| `preview(name, theme?)` | Returns a PNG of page 1, made with headless Chrome. This lets the agent see its theme. |
| `export_cv(name, format, theme?)` | Writes to `exports/` and returns the path. |

**Resources:** the jsoncv schema, the theme authoring guide, and the sample CV.

### 5.10 Build and release

- Local: `cargo tauri dev` / `cargo tauri build` on macOS (Apple Silicon; a universal binary is optional).
- CI: a GitHub Actions matrix (macOS, Windows, Ubuntu) using `tauri-apps/tauri-action`. It builds unsigned bundles (`.dmg`, `.msi`/NSIS, `.AppImage`/`.deb`) as artifacts on tag push.
- The README gives the MCP registration snippet for Claude Code and Claude Desktop, and the path to the binary inside the `.app` bundle.

## 6. Testing

- `core` carries the tests. Cover validation (good and bad CVs), atomic write + commit, restore-as-new-commit, concurrent writes from two handles, JSON Patch apply, rendering each built-in theme against the sample CV, the coverage report (a theme that skips `awards` reports it), and finding the browser.
- PDF export gets one test that runs only when a Chromium browser is found.
- UI: no automated tests at first. Do a manual check of the main flows.

## 7. Left to the builder

- Exact crate versions; React state management (keep it light); which code-editor component to use for the JSON tab.
- Visual design of the app chrome and of the built-in themes, within layout B.
- How the preview shows page breaks (paged view or a continuous page with break markers).
- Sample CV content (fictional person).
- Error copy and empty states.
