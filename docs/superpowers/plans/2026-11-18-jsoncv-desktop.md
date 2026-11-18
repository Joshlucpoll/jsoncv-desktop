# jsoncv-desktop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Tauri 2 desktop app plus an MCP server for previewing, hand-editing, versioning, theming and exporting jsoncv CVs.

**Architecture:** All file, history, rendering, coverage and export logic lives in a UI-free Rust library (`jsoncv_core`). An MCP crate (`jsoncv_mcp`) and the Tauri app are thin layers over it. The app binary runs as the MCP stdio server when started with the `mcp` argument. The React UI talks to Rust only through Tauri commands, plus a custom URI scheme that serves rendered CV HTML.

**Tech Stack:**
- Rust: tauri 2.12, minijinja 3.1, jsonschema 0.61, git2 0.21 (vendored libgit2), chromiumoxide 0.9, rmcp 3.11, notify 8, json-patch 5, serde_json 1 (`preserve_order`), fd-lock 4, similar, regex, pulldown-cmark 0.13, base64 0.23, sha2 0.11, directories 6, thiserror 2, tokio 1.
- UI: React 19, Vite, TypeScript, @rjsf/core + @rjsf/validator-ajv8 6.11, @uiw/react-codemirror + @codemirror/lang-json, vitest.
- These versions were current on 2026-11-18. Use them or newer compatible releases. **Before you write code against minijinja's `Object` trait, rmcp's tool macros or chromiumoxide's PDF API, check the current API with Context7 or docs.rs.** All three changed between major versions.

**Spec:** `docs/superpowers/specs/2026-11-18-jsoncv-desktop-design.md`. Read it before you start any task. Section references (§) point to it.

## Global Constraints

- No file write bypasses `jsoncv_core`. The app and MCP never touch library files directly.
- Every CV write validates against the bundled jsoncv schema first. An invalid CV is never written.
- Writes are atomic: write a temp file in the same directory, then rename it over the target.
- Every successful write commits at once. Commit author names are exactly `You`, `Agent` or `External`, and the email is `jsoncv@localhost`.
- A history restore adds a new commit. Never rewrite history (no reset, amend or rebase).
- The library layout is exactly as in §5.2. A CV's **name** is its path relative to the library root without `.cv.json`: `main`, `tailored/<slug>`.
- `exports/` is in the library's `.gitignore`.
- Built-in theme ids start with `builtin:`. Built-in themes are read-only.
- CV JSON files are written as pretty JSON with 2-space indent and a trailing newline. Keys keep their order (serde_json `preserve_order` is on).
- Do not copy code or themes from reorx/jsoncv (it has no license). Bundling its schema file is allowed for now.
- Path checks: a relative asset path containing `..`, or any absolute path, is rejected.
- Default library path: `~/Documents/CVs`. Config file: `<directories::ProjectDirs::from("dev","jsoncv","jsoncv-desktop").config_dir()>/config.json` with `{ "library": "<path>" }`.
- Minimum platform: macOS on Apple Silicon. Windows and Linux are built in CI only.

## Review Focus

1. **A sparse CV**, for example only `{"basics":{"name":"A"}}`, or sections with empty arrays. Every built-in theme must render it without an error and without empty headings. *Test: Task 7, `builtin_themes_render_minimal_cv`.*
2. **Dates in all three schema formats** (`YYYY`, `YYYY-MM`, `YYYY-MM-DD`), a missing `endDate` ("Present"), and an invalid date string. None of them may crash rendering. *Test: Task 6, `format_date_cases`; Task 7, `builtin_themes_render_present_job`.*
3. **HTML and non-ASCII text in CV fields** (`<script>`, `Glücklichkeit`, emoji). Text must be escaped and must survive unchanged. *Test: Task 6, `render_escapes_html_and_keeps_unicode`.*
4. **Opening a folder that already has content**: an existing `main.cv.json`, an existing Git repo, or unrelated files. `Library::open` must not overwrite or re-initialise anything. *Test: Task 2, `open_existing_folder_preserves_content`.*
5. **A CV file broken outside the app** (bad JSON or a schema failure). Listing must still work and show the error. The sweep must not commit it. Writes to that CV must fail with a clear error, not overwrite it. *Test: Task 3, `sweep_skips_invalid_file`; Task 2, `list_reports_broken_cv`.*

---

## File Structure

```
Cargo.toml                         # workspace: crates/core, crates/mcp, src-tauri
crates/core/
  Cargo.toml
  schema/jsoncv.schema.json        # bundled copy of the upstream schema
  src/lib.rs                       # pub use; CoreError; Author
  src/schema.rs                    # validate()
  src/config.rs                    # Config load/save, sync-folder check
  src/library.rs                   # Library: open, list/read, write pipeline, tailored/rename/delete
  src/history.rs                   # lock, commit, sweep_external, log, read_at, restore, diff
  src/assets.rs                    # safe_join, data URIs, inline CV and theme assets, add_asset
  src/render.rs                    # theme registry, minijinja env + helpers, render()
  src/coverage.rs                  # probe CV + tracked rendering → missing fields
  src/export.rs                    # find_browser, html_to_pdf, html_to_png, Library::export
  themes/classic/{template.html.j2,style.css,theme.json}
  themes/two-column/{template.html.j2,style.css,theme.json}
  resources/sample.cv.json         # fictional person
  resources/theme-guide.md
  tests/*.rs                       # integration tests per module
crates/mcp/
  Cargo.toml
  src/lib.rs                       # JsoncvServer (rmcp), serve_stdio()
  tests/tools.rs
src-tauri/
  Cargo.toml, tauri.conf.json, capabilities/default.json, build.rs
  src/main.rs                      # dispatch: `mcp` → jsoncv_mcp, else app
  src/app.rs                       # Builder, state, preview URI scheme, watcher
  src/commands.rs                  # #[tauri::command] wrappers
ui/
  package.json, vite.config.ts, index.html
  src/main.tsx, src/App.tsx, src/api.ts, src/styles.css
  src/components/{Sidebar,CvList,HistoryList,Preview,Toolbar,EditPanel,ConflictBanner,DiffView,ThemesView,Settings}.tsx
  src/lib/history.ts, src/lib/history.test.ts
.github/workflows/release.yml
README.md
```

---

### Task 1: Toolchain, workspace and schema validation

**Files:**
- Create: `Cargo.toml`, `crates/core/Cargo.toml`, `crates/core/src/lib.rs`, `crates/core/src/schema.rs`, `crates/core/schema/jsoncv.schema.json`
- Test: `crates/core/tests/schema.rs`

**Interfaces:**
- Produces:
  - `jsoncv_core::schema::SCHEMA: &str` (the bundled schema text)
  - `jsoncv_core::schema::validate(cv: &serde_json::Value) -> Result<(), Vec<SchemaError>>`
  - `pub struct SchemaError { pub path: String /* JSON pointer, "" = root */, pub message: String }` (Serialize)
  - `pub enum CoreError { Invalid(Vec<SchemaError>), Stale, PatchFailed(String), NotFound(String), BadName(String), BadPath(String), Locked, ReadOnly(String), Render(RenderError), Browser(String), Io(std::io::Error), Git(git2::Error), Json(serde_json::Error) }` (thiserror). Add `RenderError` in Task 6, and use a placeholder variant until then.
  - `pub enum Author { You, Agent, External }`, with `Author::name(&self) -> &'static str`

- [ ] **Step 1:** Install Rust with `brew install rustup && rustup-init -y`, then check that `cargo --version` prints 1.9x or newer. Install the Tauri CLI with `cargo install tauri-cli --version ^2 --locked`.
- [ ] **Step 2:** Create the workspace `Cargo.toml` (members `crates/core`, `crates/mcp`, `src-tauri`; for now only `crates/core` exists, so list just that member) and the core crate. Download the schema with `curl -o crates/core/schema/jsoncv.schema.json https://raw.githubusercontent.com/reorx/jsoncv/master/schema/jsoncv.schema.json`.
- [ ] **Step 3: Write the failing tests** in `crates/core/tests/schema.rs`:

```rust
#[test] fn valid_minimal_cv_passes() {
    assert!(validate(&json!({"basics":{"name":"A"}})).is_ok());
}
#[test] fn unknown_top_level_key_fails_with_root_path() {
    let errs = validate(&json!({"bogus": 1})).unwrap_err();
    assert_eq!(errs[0].path, "");
}
#[test] fn bad_date_fails_with_pointer() {
    let errs = validate(&json!({"work":[{"startDate":"March 2020"}]})).unwrap_err();
    assert_eq!(errs[0].path, "/work/0/startDate");
}
#[test] fn meta_allows_extra_tooling_keys() {
    assert!(validate(&json!({"meta":{"theme":"builtin:classic","tailoredFrom":{"cv":"main","commit":"abc"}}})).is_ok());
}
```

- [ ] **Step 4:** Run `cargo test -p jsoncv_core --test schema`. Expected: FAIL to compile (`validate` not defined).
- [ ] **Step 5:** Implement `validate`. Compile the schema once into a `std::sync::LazyLock<jsonschema::Validator>` from `include_str!("../schema/jsoncv.schema.json")`. Map each error's `instance_path` to `path`.
- [ ] **Step 6:** Run `cargo test -p jsoncv_core`. Expected: 4 passed.
- [ ] **Step 7:** Commit: `git add -A && git commit -m "feat(core): workspace and jsoncv schema validation"`.

---

### Task 2: Library open, list and read

**Files:**
- Create: `crates/core/src/library.rs`, `crates/core/src/config.rs`, `crates/core/resources/sample.cv.json`
- Test: `crates/core/tests/library.rs` (use `tempfile::TempDir`)

**Interfaces:**
- Consumes: `schema::validate`, `CoreError`
- Produces:
  - `pub struct Library` (Clone). `Library::open(root: &Path) -> Result<Library, CoreError>`, `Library::root(&self) -> &Path`
  - `Library::list_cvs(&self) -> Result<Vec<CvInfo>, CoreError>`, sorted with `main` first, then tailored CVs by name
  - `pub struct CvInfo { pub name: String, pub theme: Option<String>, pub tailored_from: Option<String>, pub modified_ms: i64, pub error: Option<String> }` (Serialize)
  - `Library::read_cv(&self, name: &str) -> Result<CvDoc, CoreError>`
  - `pub struct CvDoc { pub name: String, pub cv: Value, pub base: String }`, where `base` is the lowercase hex SHA-256 of the file bytes
  - `pub(crate) fn cv_path(&self, name: &str) -> Result<PathBuf, CoreError>`. Valid names are `main` or `tailored/<slug>`, where slug matches `^[a-z0-9][a-z0-9-]{0,63}$`. Anything else is `BadName`.
  - `jsoncv_core::config::{Config { library: PathBuf }, load() -> Config, save(&Config) -> io::Result<()>, is_in_sync_folder(&Path) -> bool}`

- [ ] **Step 1: Write the failing tests:**
  - `open_creates_layout`: on an empty temp dir, `open` creates `main.cv.json` (equal to the sample, and valid), the `tailored/`, `themes/`, `assets/` and `exports/` folders, a `.gitignore` containing the line `exports/`, a Git repo, and one initial commit by `You`. Use git2 to check the repo and the commit; this needs `history::commit` from Task 3, so in this task assert only the repo and the files. Task 3 adds the commit assertion.
  - `open_existing_folder_preserves_content`: a folder holding a custom `main.cv.json`, a `notes.txt`, and an existing Git repo with one commit. After `open`, the bytes of both files are unchanged, and the HEAD commit is the same.
  - `list_reports_broken_cv`: write `tailored/bad.cv.json` containing `{` by hand. `list_cvs` returns two entries, and `bad` has `error: Some(_)`.
  - `read_returns_stable_base`: `read_cv("main")` twice returns the same `base`. After the file bytes change, the base differs.
  - `bad_names_rejected`: `read_cv("../x")`, `read_cv("tailored/A B")` and `read_cv("other")` all return `BadName`.
  - `sync_folder_detection`: `is_in_sync_folder` returns true for `~/Library/Mobile Documents/x`, `~/Library/CloudStorage/Dropbox/x` and `~/Dropbox/x`, and false for `~/Documents/CVs`.
- [ ] **Step 2:** Run `cargo test -p jsoncv_core --test library`. Expected: FAIL to compile.
- [ ] **Step 3:** Implement `config.rs` and `library.rs` as specified in Interfaces. Write `sample.cv.json` for a fictional person: two work entries (one with no `endDate`), education, skills with keywords, a project and one profile. It must pass `validate`. In `open`, only create what is missing. Only `git init` when `root/.git` does not exist.
- [ ] **Step 4:** Run `cargo test -p jsoncv_core`. Expected: all pass.
- [ ] **Step 5:** Commit: `feat(core): library open/list/read and config`.

---

### Task 3: History (lock, commit, sweep, log, restore, diff)

**Files:**
- Create: `crates/core/src/history.rs`
- Modify: `crates/core/src/library.rs` (`open` makes the initial commit)
- Test: `crates/core/tests/history.rs`

**Interfaces:**
- Consumes: `Library`, `Author`, `schema::validate`
- Produces:
  - `pub(crate) fn with_lock<T>(root: &Path, f: impl FnOnce() -> Result<T, CoreError>) -> Result<T, CoreError>`. Uses an exclusive `fd-lock` on `.git/jsoncv.lock`. It retries every 50 ms for up to 2 s, then returns `Locked`.
  - `pub(crate) fn commit(root: &Path, paths: &[&Path], author: Author, message: &str) -> Result<String /*sha*/, CoreError>`. It stages the paths (`git add`, or a removal when the path no longer exists) and commits. It returns `""` and makes no commit when nothing changed.
  - `Library::sweep_external(&self) -> Result<Option<String>, CoreError>`. It finds changed or untracked files under the library, excluding `exports/`. It drops any `.cv.json` file that fails to parse or validate. It commits the rest as `External` with the message `External changes`. It must be called inside `with_lock`; Task 4 calls it at the start of every write. Also expose `Library::sweep_external_locked(&self)`, which takes the lock itself, for the app watcher.
  - `pub struct Version { pub commit: String, pub author: String, pub message: String, pub time_ms: i64 }` (Serialize)
  - `Library::history(&self, name: &str, limit: usize) -> Result<Vec<Version>, CoreError>`, newest first, only commits that touch that CV's file
  - `Library::read_cv_at(&self, name: &str, commit: &str) -> Result<Value, CoreError>`
  - `Library::diff(&self, name: &str, commit: &str) -> Result<String, CoreError>`. A unified diff (the `similar` crate) from the version at `commit` to the current file, both canonicalised: keys sorted recursively, pretty-printed.
  - `Library::restore(&self, name: &str, commit: &str, author: Author) -> Result<Saved, CoreError>`. Writes the old content through the Task 4 write pipeline with the message `Restored version <short sha>`. Implement it in this task after Task 4's `write_validated` exists, or move it to Task 4. Put it wherever the executor builds `write_validated` first.

- [ ] **Step 1: Write the failing tests:**
  - `open_makes_initial_commit`: after `open` on an empty dir, HEAD has author `You` and the message `Create library`.
  - `commit_noop_when_unchanged`: `commit` twice with no change in between. The second call returns `""`.
  - `sweep_commits_external_edit`: change `main.cv.json` with `fs::write` to a valid CV. `sweep_external_locked` returns `Some(sha)`, and that commit's author is `External`.
  - `sweep_skips_invalid_file`: write `{` to `main.cv.json`. `sweep_external_locked` returns `None`, and the file stays uncommitted (`git status` shows it as modified).
  - `history_filters_by_file`: commit a change to `tailored/a` and then to `main`. `history("tailored/a", 10)` contains only the first change (and the creation of `a`).
  - `diff_shows_changed_field`: change `basics.name` from `Ada` to `Grace`. `diff("main", <old>)` contains `-    "name": "Ada"` and `+    "name": "Grace"`, in the canonical pretty format.
  - `lock_times_out`: hold the lock in one thread. `with_lock` in another thread returns `Locked` after about 2 s.
- [ ] **Step 2:** Run `cargo test -p jsoncv_core --test history`. Expected: FAIL.
- [ ] **Step 3:** Implement as specified in Interfaces.
- [ ] **Step 4:** Run `cargo test -p jsoncv_core`. Expected: all pass.
- [ ] **Step 5:** Commit: `feat(core): git-backed history with external sweep`.

---

### Task 4: Write pipeline (patch, replace, draft, tailored, rename, delete, theme)

**Files:**
- Modify: `crates/core/src/library.rs`
- Test: `crates/core/tests/writes.rs`

**Interfaces:**
- Consumes: `with_lock`, `sweep_external`, `commit`, `validate`, `read_cv`
- Produces:
  - `pub struct Saved { pub commit: String, pub base: String }` (Serialize)
  - `fn write_validated(&self, name: &str, cv: &Value, author: Author, message: &str) -> Result<Saved, CoreError>` (private). Inside the lock it: sweeps external changes, validates, writes atomically, then commits.
  - `Library::patch_cv(&self, name: &str, patch: &json_patch::Patch, author: Author, message: Option<&str>) -> Result<Saved, CoreError>`. Reads the latest file and applies the patch to a clone. On failure it returns `PatchFailed(msg)`. Default message: `Edited <comma-separated top-level keys touched>`.
  - `Library::replace_cv(&self, name: &str, cv: Value, base: &str, author: Author, message: Option<&str>) -> Result<Saved, CoreError>`. Returns `Stale` if the current file's base differs from `base`.
  - `Library::save_draft(&self, name: &str, base_cv: &Value, draft: &Value) -> Result<Saved, CoreError>`. Author is `You`. It computes `json_patch::diff(base_cv, draft)`. Before each `replace` or `remove` op, it adds a `test` op holding the `base_cv` value at that path. This turns an array that shifted under the user's edit into `PatchFailed` instead of a silent wrong edit. Then it calls `patch_cv`.
  - `Library::create_tailored(&self, slug: &str, from: &str) -> Result<CvInfo, CoreError>`. Copies the source CV, sets `meta.tailoredFrom = {cv: from, commit: <HEAD sha>}`, and fails if the target exists. Message: `Created tailored/<slug> from <from>`.
  - `Library::rename_cv(&self, name: &str, new_slug: &str, author: Author) -> Result<CvInfo, CoreError>`. Only tailored CVs can be renamed or deleted. `main` returns `ReadOnly`.
  - `Library::delete_cv(&self, name: &str, author: Author) -> Result<(), CoreError>`
  - `Library::set_cv_theme(&self, name: &str, theme_id: &str, author: Author) -> Result<Saved, CoreError>`. Sets `meta.theme`. Checks the theme exists once Task 6 lands; until then it doesn't check.
  - When a CV file on disk is broken, `patch_cv`, `save_draft` and `set_cv_theme` return `Invalid` or `Json` and do not write.

- [ ] **Step 1: Write the failing tests:**
  - `patch_writes_and_commits`: patch `replace /basics/name` → `Grace`. The file has the new name, HEAD's author is `Agent`, and the returned `base` equals `read_cv` base.
  - `invalid_patch_result_not_written`: a patch that sets `/work/0/startDate` to `"March"` returns `Invalid`, and the file bytes are unchanged.
  - `replace_rejects_stale_base`: read the CV, then patch it as `Agent`, then call `replace_cv` with the old base. Returns `Stale`.
  - `draft_edits_on_different_fields_both_survive`: base = read. The agent patches `/basics/label`. Then `save_draft(base, draft with a changed /basics/name)`. The file has both changes.
  - `draft_detects_shifted_array`: base = read (work has 2 items). The agent inserts at `/work/0`. Then save a draft that edits `work[1].position`. Returns `PatchFailed`, and the agent's insert is intact.
  - `tailored_copy_records_origin`: `create_tailored("stripe","main")` makes `tailored/stripe.cv.json`, whose `meta.tailoredFrom.cv` is `"main"`. Calling it again returns an error.
  - `cannot_delete_main`: returns `ReadOnly`.
  - `write_to_broken_file_refused`: write `{` to `main.cv.json` by hand. `patch_cv` returns an error, and the file still contains `{`.
- [ ] **Step 2:** Run `cargo test -p jsoncv_core --test writes`. Expected: FAIL.
- [ ] **Step 3:** Implement as specified in Interfaces, including `restore` from Task 3 if it isn't done yet. Add its test `restore_adds_new_commit`: after a change, `restore(old)` brings back the old content, and the history has grown by one.
- [ ] **Step 4:** Run `cargo test -p jsoncv_core`. Expected: all pass.
- [ ] **Step 5:** Commit: `feat(core): validated write pipeline with conflict-safe drafts`.

---

### Task 5: Local assets

**Files:**
- Create: `crates/core/src/assets.rs`
- Test: `crates/core/tests/assets.rs`

**Interfaces:**
- Produces:
  - `pub(crate) fn safe_join(base: &Path, rel: &str) -> Option<PathBuf>`. Returns `None` for absolute paths, `..` components, or a result outside `base`.
  - `pub(crate) fn data_uri(path: &Path) -> io::Result<String>`. The MIME type comes from the extension: png, jpg/jpeg, gif, webp, svg (`image/svg+xml`), woff, woff2, ttf, otf. Anything else is `application/octet-stream`.
  - `pub(crate) fn inline_cv_assets(cv: &mut Value, root: &Path)`. Every string value starting with `assets/` that `safe_join`s to an existing file is replaced with its data URI. Anything else is left unchanged.
  - `pub(crate) fn inline_theme_assets(text: &str, theme_dir: &Path) -> String`. Rewrites `url(assets/…)`, `url('assets/…')`, `url("assets/…")` and `src="assets/…"`/`href="assets/…"` when the file exists under `theme_dir`.
  - `Library::add_asset(&self, file_name: &str, bytes: &[u8], author: Author) -> Result<String, CoreError>`. The file name is sanitised to `[a-zA-Z0-9._-]`. If the name exists, the content is compared: the same content is reused, and different content gets a `-2`, `-3`… suffix. Writes and commits, then returns `"assets/<name>"`.

- [ ] **Step 1: Write the failing tests:** `safe_join_rejects_escape` (`../x`, `/etc/passwd`, `assets/../../x` → `None`); `cv_image_inlined` (`basics.image = "assets/p.png"` with a 1×1 PNG becomes `data:image/png;base64,…`); `missing_asset_left_alone`; `url_kept`; `css_font_inlined` (`url("assets/f.woff2")` → `data:font/woff2;base64,…`); `add_asset_dedupes_and_suffixes`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement as specified in Interfaces.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): local asset inlining with path checks`.

---

### Task 6: Theme registry and rendering

**Files:**
- Create: `crates/core/src/render.rs`
- Test: `crates/core/tests/render.rs` (uses a library theme written in the test; built-ins come in Task 7)

**Interfaces:**
- Consumes: `inline_cv_assets`, `inline_theme_assets`, `Library`
- Produces:
  - `pub enum PageSize { A4, Letter }` (serde uses the variant names unchanged: `"A4"`/`"Letter"`, exactly as in `theme.json`)
  - `pub struct ThemeMeta { pub name: String, pub description: String, pub page: PageMeta { size: PageSize } }` (serde, for `theme.json`)
  - `pub struct ThemeInfo { pub id: String, pub builtin: bool, pub meta: ThemeMeta }`
  - `pub struct ThemeFiles { pub info: ThemeInfo, pub template: String, pub css: String }`
  - `pub struct RenderError { pub message: String, pub line: Option<usize> }`
  - `Library::list_themes(&self) -> Result<Vec<ThemeInfo>, CoreError>` (built-ins first)
  - `Library::get_theme(&self, id: &str) -> Result<ThemeFiles, CoreError>`
  - `Library::render(&self, cv: &Value, theme_id: &str) -> Result<String, CoreError>`. Inlines CV assets, renders with `cv` in the context, inlines theme assets, and puts the CSS in `<style>` before `</head>`. If the template output has no `</head>`, wrap it in a minimal `<!doctype html><html><head><meta charset="utf-8">…</head><body>…</body></html>`.
  - `Library::render_cv(&self, name: &str, theme: Option<&str>, commit: Option<&str>) -> Result<String, CoreError>`. Theme order is the argument, then `meta.theme`, then `builtin:classic`.
  - `pub(crate) fn environment() -> minijinja::Environment<'static>`. Auto-escape is always `Html` (set a callback; don't rely on file extensions). Registered filters and functions:
    - `format_date(value, fmt)`: parses `YYYY`, `YYYY-MM` or `YYYY-MM-DD`. Supports `%Y %m %d %b %B`. If the format needs a part the input lacks, the output drops that token and its trailing separator. Unparsable input is returned unchanged. Undefined or none gives `""`.
    - `markdown(text)`: pulldown-cmark to HTML, raw HTML in the input escaped, the result marked safe.
    - `strip_scheme(url)`: removes `http://`/`https://` and a trailing `/`.
  - `save_theme` and `duplicate_theme` come in Task 8, together with coverage.

- [ ] **Step 1: Write the failing tests:**

```rust
#[test] fn format_date_cases() {
    assert_eq!(fd("2020-03", "%b %Y"), "Mar 2020");
    assert_eq!(fd("2020", "%b %Y"), "2020");
    assert_eq!(fd("2020-03-05", "%d %b %Y"), "05 Mar 2020");
    assert_eq!(fd("2020-03", "%B %Y"), "March 2020");
    assert_eq!(fd("garbage", "%Y"), "garbage");
}
```
  Also: `strip_scheme_cases` (`https://a.com/` → `a.com`); `markdown_renders_bold_and_escapes_html` (`**a** <b>` → contains `<strong>a</strong>` and `&lt;b&gt;`); `render_escapes_html_and_keeps_unicode` (name `<script>x</script> Glücklichkeit 🚀` → output contains `&lt;script&gt;` and `Glücklichkeit 🚀`, and does not contain `<script>x`); `render_inlines_css` (the theme's CSS text appears inside `<style>`); `render_error_has_line` (a template with `{{ cv.basics.name | nope }}` on line 3 → `Render(RenderError{line: Some(3), ..})`); `render_cv_falls_back_to_classic_when_theme_missing` (write this test after Task 7, or mark it ignored until then).
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement as specified in Interfaces.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): minijinja theme rendering with helpers`.

---

### Task 7: Built-in themes, sample CV and theme guide

**Files:**
- Create: `crates/core/themes/classic/*`, `crates/core/themes/two-column/*`, `crates/core/resources/theme-guide.md`
- Modify: `crates/core/src/render.rs` (embed the built-ins with `include_str!`)
- Test: `crates/core/tests/builtin_themes.rs`

**Interfaces:**
- Produces: theme ids `builtin:classic` (one column, A4) and `builtin:two-column` (sidebar with contact, skills, languages and interests; main column with summary, work, projects and education; A4). Both render every schema section that has data. They hide headings for empty or missing sections, and they show "Present" when `endDate` is missing. Each has `@page { size: A4; margin: … }` and print-safe CSS (no fixed-height page wrappers, `break-inside: avoid` on entries).
- `jsoncv_core::THEME_GUIDE: &str` and `jsoncv_core::SAMPLE_CV: &str`
- `theme-guide.md` documents the context (`cv`), the three helpers with examples, the `assets/` convention, the `@page` rule and `theme.json`, and quotes the full `classic` template and CSS. Keep the quote in sync with an `include_str!`-based test, not by hand: the test asserts the guide contains the template text.

- [ ] **Step 1: Write the failing tests:** `builtin_themes_render_sample` (both themes, `SAMPLE_CV` → `Ok`, and the output contains the sample's name and each work position); `builtin_themes_render_minimal_cv` (`{"basics":{"name":"A"}}` → `Ok`, and the output does not contain `Work`, `Education` or `Skills` headings); `builtin_themes_render_present_job` (a work item with no `endDate` → output contains `Present`); `guide_quotes_classic_template`; `builtins_are_listed_first`.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Write both themes and the guide. Before you commit, open the rendered sample in a browser and check that it looks right as a CV: run a throwaway test that writes `target/classic.html`, then open that file.
- [ ] **Step 4:** Run `cargo test -p jsoncv_core`. Expected: all pass.
- [ ] **Step 5:** Commit: `feat(core): classic and two-column built-in themes, theme guide`.

---

### Task 8: Field-coverage check and theme saving

**Files:**
- Create: `crates/core/src/coverage.rs`
- Modify: `crates/core/src/render.rs`, `crates/core/src/library.rs`
- Test: `crates/core/tests/coverage.rs`

**Interfaces:**
- Consumes: `environment()`, `schema::SCHEMA`
- Produces:
  - `pub(crate) fn probe_cv(schema: &Value) -> Value`. Every property gets a value: strings `"x"`, booleans `true`, numbers `1`. Strings whose `$ref` is `iso8601` get `"2020-01-01"`, `format: email` gets `"a@b.co"`, and `format: uri` gets `"https://x.co"`. Arrays hold exactly one item built from their `items` schema. Skip `$schema` and `meta`. The result must pass `validate`.
  - `pub fn missing_fields(template: &str) -> Result<Vec<String>, CoreError>`. It renders `template` with `cv` = the probe, wrapped in a tracking `minijinja::value::Object`. A map lookup records `parent.key`, and iterating an array records `parent[]` and wraps each item with the path `parent[]`. It returns every leaf path in the probe that was never recorded, sorted. If no path under a top-level section was recorded, it collapses them to the single entry `"<section> (whole section)"`. Path format: `basics.location.region`, `work[].highlights`.
  - `pub struct ThemeReport { pub render_error: Option<RenderError>, pub missing: Vec<String> }` (Serialize)
  - `Library::theme_report(&self, id: &str) -> Result<ThemeReport, CoreError>`
  - `Library::save_theme(&self, id: &str, template: &str, css: &str, meta: &ThemeMeta, author: Author) -> Result<ThemeReport, CoreError>`. A `builtin:` id gives `ReadOnly`. The id must match `^[a-z0-9][a-z0-9-]{0,63}$`. It writes the three files and commits (message `Saved theme <id>`). It saves even if rendering fails, so the agent can iterate, and returns the report.
  - `Library::duplicate_theme(&self, id: &str, new_id: &str, author: Author) -> Result<ThemeInfo, CoreError>`

- [ ] **Step 1: Write the failing tests:**
  - `probe_is_valid`: `validate(&probe_cv(schema)).is_ok()`
  - `template_skipping_awards_reports_section`: a template that prints only `cv.basics.name` → `missing` contains `"awards (whole section)"` and `"basics.label"`, and does not contain `"basics.name"`.
  - `loop_fields_tracked`: `{% for w in cv.work %}{{ w.position }}{% endfor %}` → `missing` does not contain `"work[].position"` and does contain `"work[].highlights"`.
  - `filters_still_count_as_read`: `{{ cv.work[0].startDate | format_date("%Y") }}` → `"work[].startDate"` is not missing.
  - `builtin_classic_has_small_report`: `theme_report("builtin:classic")` has no `render_error`. Print its `missing` list in the test output and check by hand that only rarely used fields are in it.
  - `save_builtin_refused`: returns `ReadOnly`.
  - `save_theme_with_broken_template_still_saves`: the report has a `render_error`, and the files exist.
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement. If the tracking `Object` can't support `| length`, `is defined` and iteration at once, switch to the fallback in spec §5.6 (unique marker strings, searched in the rendered HTML), keep the same function signature, and say so in the commit message.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): theme coverage report and theme saving`.

---

### Task 9: Export (browser, PDF, PNG, HTML, JSON)

**Files:**
- Create: `crates/core/src/export.rs`
- Test: `crates/core/tests/export.rs`

**Interfaces:**
- Produces:
  - `pub fn find_browser() -> Option<PathBuf>`. Checks the env var `JSONCV_BROWSER` first, then known paths. macOS: `/Applications/{Google Chrome,Microsoft Edge,Chromium,Brave Browser}.app/Contents/MacOS/<binary>`. Windows: `%ProgramFiles%`/`%ProgramFiles(x86)%`/`%LocalAppData%` Chrome and Edge. Linux: `which` for `google-chrome`, `chromium`, `chromium-browser` and `microsoft-edge`.
  - `pub async fn html_to_pdf(html: &str, out: &Path) -> Result<(), CoreError>`. Uses chromiumoxide with the found browser, headless: `page.set_content(html)`, then `print_to_pdf` with `prefer_css_page_size: true` and `print_background: true`. Without a browser it returns `Browser("No Chrome, Edge or Chromium found")`.
  - `pub async fn html_to_png(html: &str, size: PageSize) -> Result<Vec<u8>, CoreError>`. Viewport 794×1123 (A4 at 96 dpi) or 816×1056 (Letter), device scale 2, screenshot of the first page only.
  - `pub enum ExportFormat { Pdf, Html, Json }` (serde lowercase)
  - `Library::export(&self, name: &str, format: ExportFormat, theme: Option<&str>) -> Result<PathBuf, CoreError>` (async). Writes `exports/<name with / → ->-<theme id without builtin:>.<ext>`. JSON export copies the CV file as `exports/<name>.json`.

- [ ] **Step 1: Write the failing tests:** `html_export_is_self_contained` (with an `assets/` photo, the HTML contains `data:image/png` and no `assets/`); `json_export_matches_file`; `pdf_export_writes_pdf` (skip unless `find_browser()` is `Some`; the file starts with `%PDF`); `png_preview_is_png` (same skip; the bytes start with `\x89PNG`); `missing_browser_errors` (set `JSONCV_BROWSER=/nonexistent` → `Browser` error).
- [ ] **Step 2:** Run the tests. Expected: FAIL.
- [ ] **Step 3:** Implement. Run the chromiumoxide handler on a spawned tokio task. Close the browser on every path, including errors.
- [ ] **Step 4:** Run the tests. Expected: PASS (the PDF and PNG tests run on this Mac, because Chrome is installed).
- [ ] **Step 5:** Commit: `feat(core): pdf/png via headless chrome, html/json export`.

---

### Task 10: MCP server

**Files:**
- Create: `crates/mcp/Cargo.toml`, `crates/mcp/src/lib.rs`
- Test: `crates/mcp/tests/tools.rs`

**Interfaces:**
- Consumes: the whole public `Library` API
- Produces:
  - `pub struct JsoncvServer { lib: Library }`, implementing the rmcp server handler, with `JsoncvServer::new(lib)`
  - `pub async fn serve_stdio(lib: Library) -> anyhow::Result<()>`
  - The tools, with exact names and the parameters from spec §5.9. All writes use `Author::Agent`.
    - `list_cvs`, `get_cv {name}` → `{cv, base}`, `update_cv {name, patch, message?}`, `replace_cv {name, cv, base, message?}`, `create_tailored_cv {name, from?}` (`name` is the slug)
    - `cv_history {name, limit?=20}`, `restore_cv_version {name, commit}`
    - `list_themes`, `get_theme {id}`, `save_theme {id, template, css, meta}` → `ThemeReport`, `set_cv_theme {name, theme}`
    - `preview {name, theme?}` → MCP image content (`image/png`), `export_cv {name, format, theme?}` → `{path}`
    - `add_asset {name, path? | base64?}`: exactly one of `path` or `base64`. A `path` is read from disk by the server.
  - Errors become tool results with `is_error: true`. The text is the `CoreError` message. For `Invalid` it is JSON `{"errors":[{path,message}]}`, so the agent can fix the exact fields.
  - Resources: `jsoncv://schema` (application/json), `jsoncv://theme-guide` (text/markdown), `jsoncv://sample-cv` (application/json).
  - Server instructions string: `"Edit jsoncv CVs. Read jsoncv://schema before editing and jsoncv://theme-guide before writing themes. Prefer update_cv (JSON Patch) for small edits. After save_theme, fix every render_error and consider every missing field. Use preview to see your theme."`

- [ ] **Step 1: Write the failing tests.** Call the handler methods directly on `JsoncvServer` with a temp library:
  - `update_cv_invalid_returns_field_errors` (the result is an error, and its text parses as JSON with `errors[0].path == "/work/0/startDate"`)
  - `get_then_replace_roundtrip`
  - `save_theme_returns_report`
  - `tools_list_has_14_tools_with_exact_names`
- [ ] **Step 2:** Run `cargo test -p jsoncv_mcp`. Expected: FAIL.
- [ ] **Step 3:** Implement with rmcp's tool router macros. Check the current macro API first.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(mcp): MCP server over jsoncv_core`.

---

### Task 11: Tauri shell, `mcp` subcommand, commands, preview scheme, watcher

**Files:**
- Create: `src-tauri/*` (generate with `cargo tauri init`, frontend dist `../ui/dist`, dev URL `http://localhost:5173`), `src-tauri/src/app.rs`, `src-tauri/src/commands.rs`
- Create: `ui/` (Vite React TS: `npm create vite@latest ui -- --template react-ts`)
- Modify: the workspace `Cargo.toml` (add `crates/mcp` and `src-tauri`)

**Interfaces:**
- Consumes: `jsoncv_core`, `jsoncv_mcp::serve_stdio`
- Produces:
  - `main.rs`: if `args[1] == "mcp"`, parse an optional `--dir <path>` (default `config::load().library`), call `Library::open`, then run `serve_stdio` on a tokio runtime, and **never** build the Tauri app. Otherwise call `app::run()`.
  - App state: `Mutex<Library>` (swapped when the library path changes).
  - Commands (camelCase on the JS side):
    - `list_cvs`, `read_cv(name)`, `save_draft(name, baseCv, draft)`, `create_tailored(slug, from)`, `rename_cv(name, newSlug)`, `delete_cv(name)`
    - `history(name, limit)`, `diff(name, commit)`, `restore(name, commit)`
    - `list_themes`, `theme_report(id)`, `duplicate_theme(id, newId)`, `set_cv_theme(name, themeId)`
    - `export(name, format)`, `print_cv(name)`, `add_asset_from_path(path) -> String`
    - `get_config() -> {library, inSyncFolder}`, `set_library(path) -> {library, inSyncFolder}`, `reveal(path)`, `binary_path() -> String` (`std::env::current_exe`)
    - Errors are serialised as `{kind, message, errors?}`, where `kind` is the `CoreError` variant name.
  - URI scheme `jsoncv-preview`: `…/render?cv=<name>&theme=<id?>&commit=<sha?>` returns `render_cv` HTML (`text/html`). On a render error it returns an HTML error page with the message and the line.
  - `print_cv(name)` opens a new window titled `Print – <name>`, loading the preview URL, and calls `WebviewWindow::print()` once it has loaded.
  - Watcher: `notify` recommended watcher on the library root (excluding `.git/` and `exports/`), debounced by 300 ms. It emits the event `library-changed` with `{paths: string[]}`. 2 s after the last change, it calls `sweep_external_locked`.
  - Plugins: `tauri-plugin-dialog` (folder and file pickers) and `tauri-plugin-opener` (reveal). Capabilities allow only these.

- [ ] **Step 1:** Scaffold both projects and wire `ui` scripts so that `cargo tauri dev` starts Vite.
- [ ] **Step 2:** Implement `main.rs`, `app.rs` and `commands.rs` as specified in Interfaces.
- [ ] **Step 3: Verify the MCP mode.** Run:

```bash
cargo build -p jsoncv-desktop
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}' '{"jsonrpc":"2.0","method":"notifications/initialized"}' '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | ./target/debug/jsoncv-desktop mcp --dir "$(mktemp -d)" | grep -c '"name":"update_cv"'
```
Expected: `1`, and no window appears.
- [ ] **Step 4: Verify the app.** `cargo tauri dev` opens a window. In the devtools console, `await window.__TAURI__.core.invoke('list_cvs')` returns `[{name:"main",…}]`. (Set `withGlobalTauri: true` in dev only, or test from the UI in Task 12.)
- [ ] **Step 5:** Commit: `feat(app): tauri shell, mcp subcommand, commands, preview scheme, watcher`.

---

### Task 12: UI shell: layout B, CV list, preview, theme picker

**Files:**
- Create: `ui/src/api.ts`, `ui/src/App.tsx`, `ui/src/components/{Sidebar,CvList,Preview,Toolbar}.tsx`, `ui/src/styles.css`

**Interfaces:**
- Consumes: the Task 11 commands and the `library-changed` event
- Produces:
  - `api.ts`: one typed async function per command, with TS types mirroring `CvInfo`, `CvDoc`, `Saved`, `Version`, `ThemeInfo`, `ThemeReport` and `ApiError`
  - `previewUrl(cv: string, theme?: string, commit?: string): string`. It returns `http://jsoncv-preview.localhost/render?…` on Windows and `jsoncv-preview://localhost/render?…` elsewhere, with the query values URL-encoded. Detect the OS with `navigator.userAgent`, or use `@tauri-apps/plugin-os` if it's already installed.
  - App state: the selected CV, the selected history commit (or none), the edit panel open flag, and the current view (`cv` or `themes`)

- [ ] **Step 1:** Build layout B from spec §5.8. The sidebar has the CV list (tailored CVs indented under main, with "New tailored CV…", rename and delete through a context menu), and the history list comes in Task 14. The centre is the `<iframe sandbox="allow-same-origin" src=previewUrl(...)>`. The toolbar has the CV name, the theme picker (`list_themes` → `set_cv_theme`), and the Edit (⌘E) and Export buttons, which stay disabled until Tasks 13 and 15.
- [ ] **Step 2:** On `library-changed`, reload the CV list. If the selected CV's file is in `paths`, reload the iframe (bump a `key`).
- [ ] **Step 3:** Use system light and dark mode with `prefers-color-scheme`.
- [ ] **Step 4: Verify by hand** with `cargo tauri dev`:
  - The sample CV renders.
  - Switching the theme changes the preview, and `main.cv.json` now has `meta.theme`.
  - Editing `main.cv.json` in a text editor updates the preview within about 1 s. Within 3 s, `git -C ~/Documents/CVs log -1 --format=%an` prints `External`.
  - "New tailored CV…" with `stripe` shows `stripe` under main.
- [ ] **Step 5:** Commit: `feat(ui): preview-first shell with cv list and theme picker`.

---

### Task 13: Edit panel: form, JSON tab, conflict banner, image picker

**Files:**
- Create: `ui/src/components/{EditPanel,ConflictBanner}.tsx`
- Modify: `ui/src/App.tsx`

**Interfaces:**
- Consumes: `read_cv`, `save_draft`, `add_asset_from_path`, and `SCHEMA` (exposed through a command `get_schema() -> Value`, added to Task 11's command list in this task)
- Produces: `EditPanel` props `{ name: string; onClose(): void }`

- [ ] **Step 1: Form tab.** Use `@rjsf/core` `Form` with the jsoncv schema and `@rjsf/validator-ajv8`, `liveValidate`. Add a custom `ObjectFieldTemplate` that renders each top-level property in a `<details>`. Add a custom widget for `basics.image`: a text field plus a "Choose file…" button (dialog plugin, image filters) → `add_asset_from_path` → set the field to the returned `assets/…` path.
- [ ] **Step 2: JSON tab.** Use CodeMirror with JSON language. Parse on change. Invalid JSON shows an inline error and is not saved.
- [ ] **Step 3: Saving.** Keep `base` (the `CvDoc.cv` at load) and `draft`. Call `save_draft(name, base, draft)` 1 s after the last change, only if ajv reports no errors. On `Saved`, set `base = draft`. On `PatchFailed`, show `ConflictBanner`, worded as in spec §5.4: "This CV changed while you were editing", with "Show changes" (opens `DiffView` from Task 14 against HEAD), "Discard my edits" (reload) and "Overwrite" (`read_cv` for the fresh base, then `save_draft` with that base). Show unsaved state with a dot on the panel title.
- [ ] **Step 4: Disk refresh.** On `library-changed` for this CV with no unsaved draft, reload the form. With an unsaved draft, keep the draft. The next save then patches against the new file, and a conflict raises the banner.
- [ ] **Step 5: Verify by hand:**
  - Edit the name in the form. Within about 2 s the preview updates, and `git log -1 --format=%an` prints `You`.
  - While the panel is open with an unsaved edit to `basics.name`, run `jq '.basics.label="X"' main.cv.json | sponge main.cv.json`. Both changes survive.
  - Pick a photo. It appears in `assets/` and in the preview.
  - ⌘E toggles the panel.
- [ ] **Step 6:** Commit: `feat(ui): schema form and json editor with conflict-safe autosave`.

---

### Task 14: History sidebar, version preview, diff, restore

**Files:**
- Create: `ui/src/lib/history.ts`, `ui/src/lib/history.test.ts`, `ui/src/components/{HistoryList,DiffView}.tsx`
- Modify: `ui/package.json` (add `vitest` and the `"test": "vitest run"` script)

**Interfaces:**
- Produces: `groupVersions(versions: Version[], windowMs = 300_000): VersionGroup[]`, where `VersionGroup = { author: string; label: string; newest: Version; versions: Version[] }`. Consecutive versions by the same author whose times are each within `windowMs` of the previous one form one group. The label is `"<author>: <message>"` for a single version, or `"<author>: <n> edits"` for several.

- [ ] **Step 1: Write the failing test** `history.test.ts`:
  - Three `You` versions 1 min apart → 1 group with label `You: 3 edits`.
  - `You`, `Agent`, `You` → 3 groups.
  - Two `You` versions 6 min apart → 2 groups.
- [ ] **Step 2:** Run `npm --prefix ui test`. Expected: FAIL.
- [ ] **Step 3:** Implement `groupVersions`. Then build `HistoryList`: groups with relative time ("2m", "1h", "Tue"), expandable to single versions. Clicking a version sets the selected commit; the preview loads `previewUrl(cv, theme, commit)` and the toolbar shows "Viewing <time> · Show diff · Restore · Back to current". `DiffView` shows `diff(name, commit)` with `+`/`-` lines coloured. Restore calls `restore`.
- [ ] **Step 4:** Run `npm --prefix ui test`. Expected: PASS. Then verify by hand that restoring an older version adds a new top entry and the preview matches.
- [ ] **Step 5:** Commit: `feat(ui): grouped history with version preview, diff and restore`.

---

### Task 15: Export menu, print, themes view, settings

**Files:**
- Create: `ui/src/components/{ThemesView,Settings}.tsx`
- Modify: `ui/src/components/Toolbar.tsx`

- [ ] **Step 1: Export menu.** PDF, HTML and JSON call `export`. Show the result path with a "Reveal in Finder" button (`reveal`). A `Browser` error shows "No Chrome, Edge or Chromium found — use Print… instead" with a Print… button. Print… calls `print_cv`.
- [ ] **Step 2: Themes view.** It lists the themes with a built-in badge. Selecting one shows the preview of the current CV with that theme, plus the `theme_report`: a render error, or a warning listing the missing fields (in the form shown in the mockup, `⚠ Not shown: …`). Buttons: Duplicate (prompts for an id) and Reveal in Finder (library themes only). The report refreshes on `library-changed` for that theme's folder.
- [ ] **Step 3: Settings.** Show the library path with "Change…" (folder picker → `set_library`). When `inSyncFolder` is true, show the warning: "This folder is synced by iCloud or Dropbox. Syncing can corrupt the CV history. Pick a folder outside synced storage." Also show the MCP setup snippets for Claude Code (`claude mcp add jsoncv -- "<path to binary>" mcp`) and Claude Desktop (JSON), with a copy button. Get the binary path from a `binary_path` command (`std::env::current_exe`).
- [ ] **Step 4: Verify by hand.** PDF export writes `exports/main-classic.pdf`, and it opens as a correct A4 CV. Print… opens the macOS print dialog showing only the CV. A theme that skips awards shows the warning. Choosing an iCloud Drive folder shows the warning.
- [ ] **Step 5:** Commit: `feat(ui): export, print, themes view and settings`.

---

### Task 16: CI builds and README

**Files:**
- Create: `.github/workflows/release.yml`
- Modify: `README.md`

- [ ] **Step 1:** Write `release.yml`:
  - Triggers: tag `v*` and `workflow_dispatch`.
  - Matrix: `macos-latest` (args `--target aarch64-apple-darwin`), `windows-latest`, `ubuntu-22.04`.
  - Ubuntu step: `sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`.
  - Steps: `actions/setup-node`, `dtolnay/rust-toolchain@stable`, `npm ci --prefix ui`, `cargo test -p jsoncv_core -p jsoncv_mcp`, then `tauri-apps/tauri-action@v0` with `tagName`, `releaseDraft: true` and no signing secrets.
- [ ] **Step 2:** Validate the workflow with `npx --yes action-validator .github/workflows/release.yml` or `actionlint`, if installed. Expected: no errors. Run `cargo tauri build` locally. Expected: a `.dmg` appears under `target/release/bundle/dmg/`.
- [ ] **Step 3:** In `README.md`, cover: what the app is, building it (`cargo tauri dev` / `cargo tauri build`), the library layout, MCP setup (the binary path inside the `.app`: `/Applications/jsoncv-desktop.app/Contents/MacOS/jsoncv-desktop mcp`), the "no iCloud or Dropbox" note, and the jsoncv schema license note from spec §3.
- [ ] **Step 4:** Commit: `chore: CI release builds and README`.
