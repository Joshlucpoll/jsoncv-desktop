# Flat CV Library Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the main/tailored CV model with equal CVs stored in `cvs/` (one level of real folders), with pins, migrating existing libraries without losing history.

**Architecture:** A CV's name is its slug, wherever its file sits under `cvs/`. Core (`crates/core`) finds files by scanning `cvs/` and one level of subfolders, so moves never change names, history or pins. A migration in `Library::open` moves old files and rewrites old history names on every open. MCP, Tauri commands and the React sidebar follow the core API.

**Tech Stack:** Rust (rusqlite, serde_json), Tauri 2, rmcp, React 19 + TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-11-19-flat-cv-library-design.md`

## Global Constraints

- Slug rule for CV and folder names: `^[a-z0-9][a-z0-9-]{0,63}$` (existing `valid_slug`).
- Layout: `cvs/<slug>.cv.json` or `cvs/<folder>/<slug>.cv.json`; deeper folders are ignored.
- Pins file: `<library>/.jsoncv-pins.json`, a JSON array of slugs in pin order.
- New-library sample: `cvs/my-cv.cv.json`, content = the repo-root `sample.cv.json` (user request). It replaces `crates/core/resources/sample.cv.json`.
- "Blank CV" content: `{}`.
- Duplicate-slug error text: `<slug>.cv.json is in both <a>/ and <b>/; rename or remove one` (use `cvs/` for the top level).
- No `tailoredFrom` is written. Existing values are left alone and nothing reads them.
- Rust toolchain: `export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"`. CI runs `cargo fmt --check`.
- Commit straight to `main` after each task.

## Review Focus

1. **An old library with history opens after the update.** Every CV shows its full history under its new name (Task 2: `migrates_old_layout_with_history`).
2. **A CV moved between folders in Finder** keeps its history and pin, and the app does not record a delete (Task 1: `finder_move_keeps_history`).
3. **The same slug in two folders** shows one entry with an error, and writes to it fail (Task 1: `duplicate_slug_is_one_error_entry`).
4. **A folder that holds only `.DS_Store`** can be deleted (Task 3: `delete_folder_ignores_ds_store`).
5. **The open CV is deleted or emptied out in Finder.** The UI selects the most recently modified CV, or shows the empty state. It never falls back to `'main'` (Task 6: `pickDefault` tests).

---

### Task 1: Core layout and slug lookup

**Files:**
- Modify: `crates/core/src/library.rs` (`Library::open`, `cv_names`, `cv_path`, `list_cvs`, `CvInfo`, `write_locked`)
- Modify: `crates/core/src/history.rs` (`sweep`: use the new lookup)
- Modify: `crates/core/src/export.rs:225` (`flat` = `name`)
- Replace: `crates/core/resources/sample.cv.json` with the repo-root `sample.cv.json` (`git mv -f sample.cv.json crates/core/resources/sample.cv.json`)
- Test: new `crates/core/tests/layout.rs`; update the existing `crates/core/tests/*.rs`

**Interfaces:**
- Produces:
  - `CvInfo { name, folder: Option<String>, pin: Option<usize>, theme, continuous, modified_ms, error }` (`tailored_from` removed; `pin` is always `None` until Task 4)
  - `pub(crate) fn cv_entries(&self) -> Result<Vec<(String, Option<String>, PathBuf)>, CoreError>`: every `*.cv.json` file as (slug, folder, path), invalid names included
  - `pub(crate) fn cv_path(&self, name: &str) -> Result<PathBuf, CoreError>`: validates the slug. It returns the existing file's path, or `cvs/<name>.cv.json` if no file exists. If the slug is in more than one place, it returns `BadName(<duplicate-slug text>)`.
  - `pub(crate) fn new_cv_path(&self, slug: &str, folder: Option<&str>) -> Result<PathBuf, CoreError>`: returns `BadName("<slug> already exists")` if the slug is used anywhere, and `BadName` if the folder name is invalid.
  - `write_locked(&self, tx, path: &Path, name: &str, cv, author, message)`: takes the target path explicitly.
  - `pub fn list_folders(&self) -> Result<Vec<String>, CoreError>`: sorted names of the subfolders of `cvs/`, empty ones included.

- [ ] **Step 1: Write failing tests in `crates/core/tests/layout.rs`**
  - `new_library_gets_sample_in_cvs`: `open` on an empty dir creates `cvs/my-cv.cv.json` equal to `include_str!("../resources/sample.cv.json")`, which passes `schema::validate`. `tailored/` is not created. `themes/`, `assets/` and `exports/` exist.
  - `sample_not_readded`: delete `cvs/my-cv.cv.json`, reopen, and `list_cvs()` is empty.
  - `finds_cv_in_folder`: write `cvs/jobs/stripe.cv.json` (`{}`). `read_cv("stripe")` works. `list_cvs()` has `stripe` with `folder == Some("jobs")`. `list_folders() == ["jobs"]`.
  - `duplicate_slug_is_one_error_entry`: put `stripe.cv.json` in `cvs/a/` and `cvs/b/`. `list_cvs()` has exactly one `stripe`, with `error == Some("stripe.cv.json is in both a/ and b/; rename or remove one")`. `read_cv("stripe")` is `Err(BadName(_))`.
  - `finder_move_keeps_history`: edit `my-cv` once through `patch_cv`, `fs::rename` it to `cvs/x/my-cv.cv.json`, then `sweep_external_locked()`. `history("my-cv", 10)` has the same length as before, and no entry has a deleted body.
  - `deeper_folders_ignored`: `cvs/a/b/deep.cv.json` is not listed.
- [ ] **Step 2: Run** `cargo test -p jsoncv_core --test layout`. Expected: FAIL (compile errors or assertions).
- [ ] **Step 3: Implement.**
  - `open`: check `fresh = !root.join(".jsoncv-id").exists()` before `library_id`. Create `cvs/` instead of `tailored/`. Write the sample only when `fresh` and `cv_entries()` is empty. This runs after Task 2's migration hook, so leave a call site for it.
  - `list_cvs`: group `cv_entries` by slug. A slug found more than once gives one entry with the duplicate error.
  - `sweep`: walk `cv_entries`, skipping duplicates and invalid names. The deleted-CV check uses `cv_path(name).is_ok_and(|p| !p.exists())` as it does today.
- [ ] **Step 4: Update the existing core tests.** Use `"my-cv"` where they relied on the sample `"main"`, `cvs/my-cv.cv.json` for `main.cv.json` paths, and `<slug>` for `tailored/<slug>` names. Replace `create_tailored(...)` calls with a direct file write until Task 3 lands. `open_creates_layout` moves into `layout.rs`.
- [ ] **Step 5: Run** `cargo test -p jsoncv_core && cargo fmt --check`. Expected: PASS.
- [ ] **Step 6: Commit** `feat(core): CVs live in cvs/ with one folder level, found by slug`

### Task 2: Migration of old libraries

**Files:**
- Create: `crates/core/src/migrate.rs` (declare it in `lib.rs` as `mod migrate;`)
- Modify: `crates/core/src/library.rs` (`open` calls it)
- Test: `crates/core/tests/migrate.rs`

**Interfaces:**
- Consumes: `cv_entries`, `valid_slug`, `Library::locked`.
- Produces: `pub(crate) fn migrate(lib: &Library) -> Result<(), CoreError>`. `open` calls it after `history::init` and before the sample and the first sweep.

- [ ] **Step 1: Write failing tests** (build the old layout by hand: `main.cv.json`, `tailored/<s>.cv.json`, and history from an old-style open). To create old-named history rows, insert into the `versions` table directly with `rusqlite` and `cv = 'tailored/acme'`.
  - `migrates_old_layout_with_history`: `main` and `tailored/acme` with rows for both. After `open`, the files are at `cvs/main.cv.json` and `cvs/acme.cv.json`, `tailored/` is gone, and `history("acme", 10)` returns the old rows.
  - `migration_rerun_is_noop`: a second `open` changes no files and no row counts.
  - `tailored_main_collides`: `main.cv.json` plus `tailored/main.cv.json` gives `cvs/main.cv.json` and `cvs/main-2.cv.json`. The `tailored/main` rows become `main-2`.
  - `recreated_root_main_becomes_main_2`: after a migration, write a new root `main.cv.json` and reopen. It lands at `cvs/main-2.cv.json`, and `cvs/main.cv.json` is unchanged.
  - `history_only_migration`: the files are already in `cvs/` (a second Mac), but the rows are still `tailored/acme`. After `open`, `history("acme", 10)` is non-empty.
  - `deleted_cv_rows_renamed`: a `tailored/gone` row with no file becomes `gone`.
- [ ] **Step 2: Run** `cargo test -p jsoncv_core --test migrate`. Expected: FAIL.
- [ ] **Step 3: Implement `migrate`** inside one `lib.locked` transaction.
  - **Files:** move each root `main.cv.json` and each `tailored/*.cv.json` into `cvs/`. Never overwrite. Pick the first free slug among `s`, `s-2`, `s-3` and so on, checking both `cv_entries` and the slugs already used in this pass. Record the mapping from the old name (`main` or `tailored/<s>`) to the new slug. Remove `tailored/` if it is empty (ignore `.DS_Store`).
  - **History:** for each distinct `cv LIKE 'tailored/%'` of this library, the target is the mapping entry if there is one. Otherwise the target is the stripped slug, and if that slug already has rows, the first free `-N`. Then run `UPDATE versions SET cv = target`. The root `main` → `main-N` mapping also renames rows only when the move happened in this pass. Add the comment `// shortcut: a re-created root main shares main's earlier rows, fine while old processes are rare`.
- [ ] **Step 4: Run** `cargo test -p jsoncv_core && cargo fmt --check`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): migrate main/tailored libraries to cvs/ on open`

### Task 3: CV and folder operations

**Files:**
- Modify: `crates/core/src/library.rs`
- Test: `crates/core/tests/cv_ops.rs`

**Interfaces:**
- Consumes: `cv_path`, `new_cv_path`, `write_locked(tx, path, ...)`.
- Produces (all `pub`, all take `author: Author` except the folder operations, and all run in `self.locked` after `sweep_external`):
  - `duplicate_cv(&self, slug: &str, from: &str, author) -> Result<CvInfo, CoreError>`: copies into `from`'s folder. History message: `Duplicated <from> as <slug>`. Replaces `create_tailored`.
  - `create_cv(&self, slug: &str, folder: Option<&str>, author) -> Result<CvInfo, CoreError>`: content `{}`. Message: `Created <slug>`.
  - `rename_cv(&self, name, new_slug, author) -> Result<CvInfo, CoreError>`: any CV, same folder. The `main` guard is removed.
  - `delete_cv(&self, name, author) -> Result<(), CoreError>`: any CV. Returns `ReadOnly("You can't delete the last CV")` if it is the only valid CV.
  - `move_cv(&self, name: &str, folder: Option<&str>) -> Result<CvInfo, CoreError>`: `create_dir_all` the target folder, then `fs::rename`. Records no history. Returns `BadName` if a file is already at the target.
  - `create_folder(&self, name: &str) -> Result<(), CoreError>`, `rename_folder(&self, name: &str, new_name: &str) -> Result<(), CoreError>` and `delete_folder(&self, name: &str) -> Result<(), CoreError>`. `delete_folder` removes a `.DS_Store` first. If the folder still has files, it returns `BadName("Folder <name> is not empty")`.

- [ ] **Step 1: Write failing tests.**
  - `duplicate_goes_to_source_folder`: duplicate `cvs/jobs/a` as `b`. `b` is in `jobs`, and its JSON has no `meta.tailoredFrom`.
  - `duplicate_existing_slug_fails`: a `BadName` error.
  - `create_cv_is_empty_object`: `read_cv` returns `json!({})` in the given folder.
  - `rename_keeps_folder_and_history`
  - `delete_last_cv_refused`: in a library with only `my-cv`, `delete_cv` returns `ReadOnly`. With two CVs it succeeds.
  - `move_cv_keeps_history`: history length is unchanged, the file is in the new folder, and `move_cv(name, None)` moves it back to the top level.
  - `delete_folder_ignores_ds_store`: a folder holding only `.DS_Store` is deleted. A folder with a CV returns `BadName`.
  - `rename_folder_moves_cvs`: the CVs inside keep their names and history.
- [ ] **Step 2: Run** `cargo test -p jsoncv_core --test cv_ops`. Expected: FAIL.
- [ ] **Step 3: Implement** the signatures above. Remove `create_tailored`. Replace the stand-in file writes from Task 1 Step 4 with `duplicate_cv` where tests copied CVs.
- [ ] **Step 4: Run** `cargo test -p jsoncv_core && cargo fmt --check`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): duplicate, create, move and folder operations; any CV can be renamed or deleted`

### Task 4: Pins

**Files:**
- Modify: `crates/core/src/library.rs`
- Test: `crates/core/tests/pins.rs`

**Interfaces:**
- Produces:
  - `pub fn set_pinned(&self, name: &str, pinned: bool) -> Result<(), CoreError>`. Pinning appends to the end, and unpinning removes the slug. Returns `NotFound` if the CV does not exist. Writes `.jsoncv-pins.json` atomically (tmp file + rename, like `write_locked`).
  - `list_cvs` fills `CvInfo.pin` with the slug's index in the file. Unknown slugs in the file are ignored. A missing or unparsable file counts as `[]`.
  - `rename_cv` replaces the slug in the pins file. `delete_cv` removes it. `move_cv` leaves it alone.

- [ ] **Step 1: Write failing tests:** `pin_order_is_file_order`, `pin_survives_rename_and_move`, `pin_dropped_on_delete`, `corrupt_pins_file_is_empty`.
- [ ] **Step 2: Run** `cargo test -p jsoncv_core --test pins`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test -p jsoncv_core && cargo fmt --check`. Expected: PASS.
- [ ] **Step 5: Commit** `feat(core): pin CVs, stored in .jsoncv-pins.json`

### Task 5: MCP tools and Tauri commands

**Files:**
- Modify: `crates/mcp/src/lib.rs`, `crates/mcp/tests/tools.rs`
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/app.rs` (register commands, menu item, `relevant` test)

**Interfaces:**
- Consumes: the Task 3 and Task 4 core functions.
- Produces:
  - **MCP:**
    - Rename the tool `create_tailored_cv` to `duplicate_cv` with `DuplicateCvArgs { name: String, from: String }`. Description: "Duplicate a CV as `name`, in the same folder as `from`. The copy is independent."
    - Every name doc string becomes "CV name (its slug, e.g. `stripe`)."
    - The `list_cvs` description lists `(name, folder, pin, theme, modified_ms, error)`.
  - **Tauri commands** (names and camelCase args as the UI calls them):
    - `duplicate_cv(slug, from)` and `create_cv(slug, folder: Option<String>)`, which return `CvInfo`
    - `move_cv(name, folder: Option<String>)`, which returns `CvInfo`
    - `list_folders()`, which returns `Vec<String>`
    - `create_folder(name)`, `rename_folder(name, new_name)`, `delete_folder(name)` and `set_pinned(name, pinned: bool)`, which return `()`
    - Remove `create_tailored`.
  - **Menu:** the `new-cv` item label becomes `"Duplicate CV…"`, keeping `CmdOrCtrl+N`.

- [ ] **Step 1: Update `crates/mcp/tests/tools.rs`.**
  - Use `"my-cv"` instead of `"main"`.
  - Put `duplicate_cv` in the tool-name list.
  - Add `duplicate_cv_copies`: duplicate `my-cv` as `b`, and `list_cvs` text contains `"b"`.
  - Update the `relevant_filters_paths` test in `app.rs` to `cvs/my-cv.cv.json` and `cvs/jobs/a.cv.json`.
  - Update the preview URL test to use `cv=nope`.
- [ ] **Step 2: Run** `cargo test --workspace`. Expected: FAIL.
- [ ] **Step 3: Implement** the changes above. Each new command uses `with_lib` like its neighbours and passes `Author::You`.
- [ ] **Step 4: Run** `cargo test --workspace && cargo fmt --check && cargo clippy --workspace -- -D warnings`. Expected: PASS.
- [ ] **Step 5: Commit** `feat: duplicate_cv agent tool and folder/pin commands`

### Task 6: UI data layer and App selection

**Files:**
- Modify: `ui/src/api.ts`, `ui/src/lib/names.ts` (delete it, and inline `name` wherever `slugOf` or `cvTitle` was used in `Toolbar.tsx` and `CvList.tsx`), `ui/src/App.tsx`, `ui/src/components/Settings.tsx:119-122`, `ui/src/dev/mockTauri.ts`, `ui/src/components/CvForm.tsx:320` (comment)
- Create: `ui/src/lib/sidebar.ts`, `ui/src/lib/sidebar.test.ts`

**Interfaces:**
- Consumes: the Task 5 command names and args.
- Produces:
  - `CvInfo = { name: string; folder: string | null; pin: number | null; theme: string | null; continuous: boolean; modified_ms: number; error: string | null }`
  - api calls: `duplicateCv(slug, from)`, `createCv(slug, folder: string | null)`, `moveCv(name, folder: string | null)`, `listFolders()`, `createFolder(name)`, `renameFolder(name, newName)`, `deleteFolder(name)` and `setPinned(name, pinned)`. Remove `createTailored`.
  - `groupCvs(cvs: CvInfo[], folders: string[]): { pinned: CvInfo[]; folders: { name: string; cvs: CvInfo[] }[]; loose: CvInfo[] }` in `lib/sidebar.ts`:
    - `pinned` is sorted by `pin`.
    - `folders` holds every folder name alphabetically, empty folders included, each with its CVs alphabetically.
    - `loose` holds the top-level CVs alphabetically.
    - A pinned CV also appears in its folder or in `loose`.
  - `pickDefault(cvs: CvInfo[]): string | undefined`: the name with the largest `modified_ms`, or `undefined` if the list is empty.

- [ ] **Step 1: Write `sidebar.test.ts`:** `groups pinned by pin order and keeps them in their folder`, `lists empty folders`, `sorts loose CVs by name`, `pickDefault picks most recently modified`, `pickDefault returns undefined for no CVs`.
- [ ] **Step 2: Run** `npm --prefix ui test`. Expected: FAIL.
- [ ] **Step 3: Implement** `sidebar.ts` and update the api types and calls.
- [ ] **Step 4: Update `App.tsx`.**
  - `selected` starts as `''`. `current` = `selected` if that CV exists, else `pickDefault(cvs) ?? ''`.
  - `changeLibrary` resets the selection to `''`.
  - With no CV, the preview, toolbar and edit actions show nothing. Guard every `cv`/`current` use, so no command is invoked with `''`.
  - The watcher match becomes `paths.some((p) => p.endsWith(`/${cv}.cv.json`))`.
  - Store `folders` from `listFolders()` beside `cvs`, reload both together, and pass `folders` to `Sidebar`.
- [ ] **Step 5: Update `Settings.tsx`.** Show the CV count and the folder count, with no "main/tailored" split.
- [ ] **Step 6: Update `mockTauri.ts`.** Fixtures: `general` (pin 0, top level), `stripe` and `monzo` in folder `applications` (`monzo` pin 1), `broken-draft` with an error, and an empty folder `archive`. Handle the new commands.
- [ ] **Step 7: Run** `npm --prefix ui test && npm --prefix ui run build && npm --prefix ui run lint`. Expected: PASS. The build fails until Task 7 updates `CvList` props, so do Task 7 before you commit, or stub the props.
- [ ] **Step 8: Commit** together with Task 7.

### Task 7: Sidebar

**Files:**
- Modify: `ui/src/components/CvList.tsx`, `ui/src/components/Sidebar.tsx`, `ui/src/styles.css:103-130`

**Interfaces:**
- Consumes: `groupCvs`, the api calls from Task 6, and the `folders: string[]` prop.

- [ ] **Step 1: Render the sidebar from `groupCvs`.**
  - A "Pinned" heading, shown only when something is pinned.
  - Folder rows with a disclosure toggle, then the loose CVs, then `+ New CV…`.
  - Folder collapse state goes in `localStorage` key `jsoncv.collapsedFolders` (a JSON array). Wrap reads and writes in try/catch.
  - Remove the branch icon, the `nested` origin styling and the `↳` rule. Keep indentation only for CVs inside a folder.
  - With no CVs, show an empty-state line, "No CVs yet", above `+ New CV…`.
- [ ] **Step 2: Build the CV row menu.**
  - Items: Rename…, Duplicate…, Move to ▸, Pin / Unpin, Delete….
  - Move to lists "No folder" plus each folder except the current one, as plain menu items under a "Move to" label. No submenu.
  - Disable Delete when `cvs.length === 1`, with the title "You can't delete the last CV".
- [ ] **Step 3: Build the folder row menu.** Items: Rename… (reuse `SlugInput`) and Delete. Show Delete only when the folder is empty. On error, call `onError`.
- [ ] **Step 4: Build `+ New CV…`.** It opens a two-item menu: "Blank CV" (a `SlugInput`, then `createCv(slug, null)` and select it) and "New folder" (a `SlugInput`, then `createFolder`). The `newSignal` (⌘N) opens the Duplicate `SlugInput` for the selected CV, with the initial value `<name>-copy`.
- [ ] **Step 5: Verify.** Run `npm --prefix ui test && npm --prefix ui run build && npm --prefix ui run lint`. Expected: PASS. Then `npm --prefix ui run dev` and open `/?mock`. Check that pinned, applications (open), archive (empty), general and broken-draft render, and that each menu shows the right items. Take one screenshot of the sidebar.
- [ ] **Step 6: Commit** (Tasks 6 and 7) `feat(ui): flat CV list with folders and pins`

### Task 8: Docs

**Files:**
- Modify: `README.md` (lines 7, 16, 30-46)

- [ ] **Step 1: Update the README.**
  - Intro: "Keep as many CVs as you like (for example, one per job), organised in folders. Duplicate one to start another."
  - Features: replace "Tailored CVs…" with "Folders and pinned CVs".
  - Layout block: replace `main.cv.json` and `tailored/<slug>.cv.json` with `cvs/<slug>.cv.json`, `cvs/<folder>/<slug>.cv.json` and `.jsoncv-pins.json`.
  - Add one line: "Libraries from earlier versions are moved to `cvs/` the first time they open."
- [ ] **Step 2: Check for leftovers.** Run `grep -rn -i "tailored" --exclude-dir=node_modules --exclude-dir=target --exclude-dir=docs crates src-tauri ui/src README.md`. Expected: matches only in `migrate.rs` and its tests.
- [ ] **Step 3: Commit** `docs: flat CV library layout`
