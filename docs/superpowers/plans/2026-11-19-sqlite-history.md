# SQLite Version History Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Git-backed CV history with one SQLite file in app storage, with a user-chosen location.

**Architecture:** CV files stay on disk as the truth. `jsoncv_core::history` owns a `versions` table; every
locked write runs inside one `BEGIN IMMEDIATE` transaction that both writes the file and inserts the row.
`Library` carries the history db path and a library id read from `.jsoncv-id`.

**Tech Stack:** Rust (rusqlite `bundled`), Tauri 2, React/TS.

**Spec:** `docs/superpowers/specs/2026-11-19-sqlite-history-design.md`

## Global Constraints

- Remove `git2` and `fd-lock` from every crate; add `rusqlite = { version = "0.43", features = ["bundled"] }` to core.
- Default db: `ProjectDirs::from("dev", "jsoncv", "jsoncv-desktop").data_dir().join("history.db")`.
- Busy timeout 2 s; busy → `CoreError::Locked`. No WAL.
- `Version { commit, author, message, time_ms }` and `Saved { commit, base }` keep their field names; ids are decimal strings.
- Unchanged content inserts no row and returns `commit: ""`.
- Only `main.cv.json` and `tailored/*.cv.json` are versioned.

## Review Focus

1. History file folder missing or unwritable → open fails with an Io error, not a panic.
2. Two processes (app + MCP) writing at once → second waits up to 2 s, then `Locked`.
3. Rename then history → old versions appear under the new name.
4. Delete then recreate the same tailored slug → history shows old versions plus a "Deleted" row; restore of a deletion row → NotFound.
5. Moving history to a folder that already has `history.db` → existing file used, current file untouched.

---

### Task 1: Core history store

**Files:**
- Modify: `crates/core/Cargo.toml`, `crates/core/src/{config,error,history,library,render,assets}.rs`
- Test: `crates/core/tests/{history,library,writes,coverage,assets}.rs` (rewrite every git2 use)

**Interfaces:**
- Produces:
  - `config::Config { library: PathBuf, #[serde(default)] history: Option<PathBuf> }`, `config::default_history() -> PathBuf`, `Config::history_path(&self) -> PathBuf`
  - `CoreError::Db(rusqlite::Error)` (replaces `Git`), kind string `"Db"`
  - `Library::open(root: &Path, db: &Path) -> Result<Library>`; `Library::open_configured(configured, default, db)`; `Library::db(&self) -> &Path`
  - `Library::locked<T>(&self, f: impl FnOnce(&rusqlite::Transaction) -> Result<T>) -> Result<T>` (replaces `with_lock`)
  - `Library::sweep_external(&self, tx) -> Result<Option<String>>`, `sweep_external_locked(&self)`
  - `history`, `read_cv_at`, `diff`, `change_diff` unchanged signatures
  - `history::move_db(from: &Path, to_dir: &Path) -> Result<PathBuf>` (VACUUM INTO when target absent)

- [ ] **Step 1: Rewrite tests against the new API** — `tests/history.rs`: `open_records_create_library` (one "You"/"Create library" row for main), `save_noop_when_unchanged` (second identical `replace_cv` returns commit ""), `sweep_records_external_edit` (author "External", message "External changes", second sweep None), `sweep_skips_invalid_file`, `history_filters_by_cv`, `diff_shows_changed_field`, `change_diff_shows_only_that_version` (oldest diffs against "--- empty"), `lock_times_out` (hold `locked` 2.6 s in a thread → `Locked` after ≥1.9 s), `unknown_version_is_not_found` (`"999999"` and `"abc"`), `rename_moves_history`, `delete_then_recreate_keeps_history`, `history_follows_moved_folder` (copy folder incl. `.jsoncv-id`, same db → same history), `move_db_uses_existing_target`. Other test files: replace `Library::open(d.path())` with a helper that passes `d.path().join(".h.db")`-style db in a second TempDir, and replace git asserts with `lib.history(..)` asserts (themes/assets: drop the commit asserts).
- [ ] **Step 2: Run** `cargo test -p jsoncv_core` → compile FAIL.
- [ ] **Step 3: Implement.** Schema per spec; `open` creates parent dir of db, creates `.jsoncv-id` (32 hex from sha of time+pid+root) if missing, creates table/index, and if no rows exist for the library runs sweep as You/"Create library". Drop `.gitignore` writing and `git2::Repository::init`. `write_locked(tx, …)` inserts after the atomic write. Rename: `UPDATE versions SET cv=?new WHERE library=? AND cv=?old` then insert "Renamed …". Delete: insert row with NULL sha/body. Tailored `meta.tailoredFrom.commit` = newest id of `from`. Theme/asset writes keep `locked` + sweep, no row. Diff headers `v{id}`; restore message `Restored version v{id}`.
- [ ] **Step 4: Run** `cargo test -p jsoncv_core` → PASS; `cargo clippy -p jsoncv_core` clean.
- [ ] **Step 5: Commit** `feat(core): store CV history in SQLite instead of Git`

### Task 2: App and MCP wiring

**Files:** `src-tauri/src/{main,app,commands}.rs`, `src-tauri/Cargo.toml`, `crates/mcp/src/lib.rs` (tests only if they open a Library)

**Interfaces:**
- Consumes: Task 1.
- Produces: `LibraryConfig { library, history, history_in_sync_folder, fallback_from }` (camelCase); command `set_history(path: PathBuf /* folder */) -> LibraryConfig`.

- [ ] **Step 1:** Update the `kind` test (`CoreError::Db` → "Db") and app tests to the new `Library::open`.
- [ ] **Step 2:** Implement: `run_mcp` uses `config::load().history_path()` for both arms; `open_with_fallback(configured, default, db)`; `set_library` saves config preserving `history`; `set_history` requires an absolute path, runs `move_db` under `blocking`, saves config, reopens Library, restarts watcher. Remove `in_sync_folder` for the library; add `history_in_sync_folder`.
- [ ] **Step 3: Run** `cargo test --workspace` and `cargo clippy --workspace` → PASS.
- [ ] **Step 4: Commit** `feat(app): history file location setting`

### Task 3: Settings UI

**Files:** `ui/src/api.ts`, `ui/src/components/Settings.tsx`, `ui/src/App.tsx`

- [ ] **Step 1:** `LibraryConfig = { library; history; historyInSyncFolder; fallbackFrom? }`; `setHistory(path)`.
- [ ] **Step 2:** Settings: "History" section with path + Change… (folder picker, same pattern as library). Warning text: "This file is in iCloud or Dropbox. Syncing can corrupt the history if two Macs write to it." Remove the library sync warning.
- [ ] **Step 3: Run** `npm run build` (and `npm test` if present) in `ui/` → PASS.
- [ ] **Step 4: Commit** `feat(ui): choose history file location`
