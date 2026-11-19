# SQLite version history (replaces Git)

Status: approved 2026-11-19.

## Why

The CV folder should be able to live in iCloud Drive or Dropbox. A Git repo inside a synced folder
corrupts, and pushing history to GitHub is a workflow the user does not want. History moves out of the
CV folder into one SQLite file in app storage.

## Goals

- CV files stay plain JSON on disk and remain the source of truth (MCP agents and text editors edit them).
- Every CV save, restore, rename, delete and outside edit still adds a version, exactly as today.
- History lives at `<app data dir>/history.db` by default
  (`~/Library/Application Support/dev.jsoncv.jsoncv-desktop/history.db` on macOS).
- Settings lets the user pick another folder for the history file.
- The UI and MCP APIs keep their shape (`Version.commit`, `Saved.commit` stay strings).

## Non-goals

- iCloud / CloudKit sync of history (later, needs a signed app).
- Importing existing Git history. Old `.git` folders stay on disk untouched.
- Versioning themes and assets (nothing reads their history today).
- A "reset to default location" button.

## Approaches considered

1. **History-only SQLite, files on disk stay the truth** (chosen). Keeps MCP, editors and folder sync working.
2. CVs stored inside SQLite. Rejected: breaks MCP/editor edits and iCloud Drive sync of the CVs.
3. SQLite file inside the CV folder. Rejected: same sync-corruption problem as Git.

## Design

- **Dependency:** `rusqlite` with `bundled`. Remove `git2` and `fd-lock`.
- **Schema** (one table):
  `versions(id INTEGER PRIMARY KEY, library TEXT, cv TEXT, author TEXT, message TEXT, time_ms INTEGER,
  sha TEXT, body BLOB)`, index on `(library, cv, id)`. `body`/`sha` NULL = the CV was deleted.
  Full snapshots (CVs are 10–50 KB).
- **Library identity:** `.jsoncv-id` file (random hex id) in the CV folder, created on first open.
  Rows are keyed by it, so history follows a moved folder and libraries never mix.
- **Locking:** each write runs in one `BEGIN IMMEDIATE` transaction with a 2 s busy timeout. SQLite's
  lock replaces `.git/jsoncv.lock`; a busy database maps to `CoreError::Locked`. Default journal mode
  (no WAL), so the file also works on external drives.
- **Saving:** write the file atomically (as today), then insert a row. Unchanged content (same sha as
  the newest row) inserts nothing and returns commit `""`, as Git did.
- **Outside edits (sweep):** for each CV file that parses and validates, compare its SHA-256 with the
  newest row; insert an `External` / "External changes" row when different. A CV whose newest row has a
  body but whose file is gone gets a deletion row.
- **First open:** if the library has no rows, the first sweep records every valid CV as `You` /
  "Create library".
- **Rename:** existing rows move to the new name, then a "Renamed" row is added. History follows the CV.
- **Version ids:** the row id as a string. Diff headers and the restore message use `v<id>`.
  `meta.tailoredFrom.commit` stores the source CV's newest version id.
- **Config:** `config.json` gains optional `history` (path to the `.db` file). Missing = default path.
  The MCP process reads the same config.
- **Changing location:** Settings → "History" → Change… picks a folder. If it already holds
  `history.db`, that file is used. Otherwise the current database is copied there with `VACUUM INTO`
  and the old file is removed. The app reopens the library with the new path.
- **Sync warning:** the existing iCloud/Dropbox warning moves from the library folder to the history
  file ("syncing can corrupt the history file if two Macs write to it").
- **Removed:** `.gitignore` writing in `Library::open`, Git commits for themes and assets.

## Left to the builder

- Exact test layout; tests must keep covering every case the Git tests covered.
- A running MCP process keeps the history path it started with until restarted (documented shortcut).
