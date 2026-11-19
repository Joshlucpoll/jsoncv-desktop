# Flat CV library with folders and pins

## Intent

Today the library has one special "Main CV" (`main.cv.json`) and "tailored" CVs nested under it
(`tailored/<slug>.cv.json`, with `meta.tailoredFrom`). Nothing is synced after a duplicate, so the
parent/child display is misleading. All CVs become equal. The user organises them with real folders
on disk and pins the ones they use most.

## Goals

- Every CV is equal: any CV can be renamed, duplicated, moved or deleted.
- Folders in the sidebar are real folders on disk, one level deep.
- Pinned CVs show at the top of the sidebar.
- A CV keeps its history when it moves between folders, including moves made in Finder.
- Existing libraries migrate once, with no history lost.

## Non-goals

- No syncing between a CV and the CV it was copied from.
- No nested folders (folders inside folders).
- No drag and drop (moving is a menu action). Can come later.
- No app-only folders or tags.

## Anti-goals

- Losing or orphaning version history during migration or a move.
- Folders in the sidebar that differ from what Finder shows.

## Approaches considered

- **A. Name is the path** (`applications/stripe`). Moves in the app rename history rows; a move in
  Finder looks like delete + create, so history is stranded under the old name. Same slug allowed in
  two folders.
- **B. Name is the slug, wherever the file is** (`stripe`). *Chosen.* A move (app or Finder) never
  changes the name, so history, pins and agent references survive with no extra work. Cost: slugs are
  unique across the library; the app finds a file by scanning `cvs/` and one level of subfolders.
- App-only folders (stored in a settings file) were rejected by the user: they drift from Finder.

## Design

### On-disk layout

```
<library>/
  cvs/
    <slug>.cv.json
    <folder>/<slug>.cv.json     # one level only
  assets/  themes/  exports/
  .jsoncv-id
  .jsoncv-pins.json             # ["stripe", "monzo"] — pinned slugs, in pin order
```

- CV name = slug (`^[a-z0-9][a-z0-9-]{0,63}$`, the existing rule). Folder names use the same rule.
- `cvs/` and its first-level subfolders are scanned. Deeper folders and non-`.cv.json` files are ignored.
- Asset paths (`assets/...`) already resolve against the library root, so they are unaffected.
- A **new** library (no `.jsoncv-id` before this open) gets `cvs/my-cv.cv.json` from the sample.
  The app never re-adds it later. If the user empties `cvs/` in Finder, the sidebar shows an empty
  state with "New CV…", and the editor and preview show nothing selected.

### Duplicate slugs

If the same `<slug>.cv.json` exists in two places (possible via Finder), the CV list shows **one**
entry for that slug with an error: "stripe.cv.json is in both a/ and b/; rename or remove one".
Reads and writes of that name fail with that error. History sweep skips it until it is resolved.
File names that break the slug rule show with an error, as they do today.

### Migration (in `Library::open`, under the lock, on every open)

The file step and the history step are independent and both run on **every** open. A library on a
synced folder or external drive can be opened by a second Mac (its own history database), and an
agent session started before the update can still run old code that writes `main.cv.json` at the root.

**Files** — runs when `main.cv.json` or `tailored/*.cv.json` exists at the library root:

1. Create `cvs/`.
2. Move `main.cv.json` → `cvs/main.cv.json`; move each `tailored/<slug>.cv.json` → `cvs/<slug>.cv.json`.
3. Never overwrite: if the target slug already exists anywhere in `cvs/`, use `<slug>-2`, `-3`, ….
   (This covers `tailored/main.cv.json`, and an old process re-creating a root `main.cv.json`.)
4. Remove `tailored/` if it is empty.
5. Leave `meta.tailoredFrom` in existing files (harmless; nothing reads it any more).

**History** — rewrite this library's rows: `tailored/<slug>` → `<slug>`, including rows of deleted
CVs. `main` stays `main`. A file renamed to `<slug>-N` in step 3 also has its rows renamed. The rewrite
reads the history rows, not the files, so a second Mac whose files were already moved still migrates
its own history. It is a no-op once no `tailored/` rows remain.

### Core library (`crates/core`)

- `cv_names` / `cv_path` find CVs by slug across `cvs/` and its subfolders.
- `CvInfo`: remove `tailored_from`; add `folder: Option<String>` and `pinned: bool`.
- `create_tailored` → `duplicate_cv(slug, from)`: copies into the same folder as `from`, no `tailoredFrom`.
- New `create_cv(slug, folder)`: a blank CV, `{}` (the schema requires no fields). Not the sample,
  which is a fake person.
- `rename_cv`: works on any CV; keeps the folder; renames history rows (as today) and the pin.
- `delete_cv`: works on any CV, but refuses to delete the last CV. Drops its pin.
- New `move_cv(slug, folder: Option<String>)`: moves the file; history is unchanged (name is unchanged).
- Folders: `create_folder(name)`, `rename_folder(old, new)`, `delete_folder(name)` (only when empty),
  and the list of folders (including empty ones) returned with the CV list.
- Pins: `set_pinned(slug, bool)` writes `.jsoncv-pins.json`. Pins of slugs that no longer exist are ignored.
- The history sweep keys by slug, so a Finder move between folders records nothing new.

### Agent tools (`crates/mcp`)

- CV names in all tools are plain slugs. Update every "`main` or `tailored/<slug>`" doc string.
- `create_tailored_cv` → `duplicate_cv` with `from` required.
- `list_cvs` returns `folder` and `pinned` instead of `tailored_from`.
- No folder or pin tools for agents.

### App commands (`src-tauri`)

Mirror the core changes: `duplicate_cv`, `create_cv`, `move_cv`, `create_folder`, `rename_folder`,
`delete_folder`, `set_pinned`; `list_cvs` also returns folders. The watcher is already recursive.

### Sidebar (`ui/src/components/CvList.tsx`)

```
PINNED
  ★ stripe
  ★ monzo
applications ▾
    stripe
    monzo
archive ▸
general
+ New CV…
```

- Order: pinned CVs (pin order), then folders (alphabetical, collapsible), then loose CVs (alphabetical).
  A pinned CV also appears in its folder.
- Folder collapse state is remembered per viewer in `localStorage`.
- No branch icon or nesting by origin. Every CV shows its slug.
- Row menu: Rename…, Duplicate…, Move to ▸ (folders + "No folder"), Pin / Unpin, Delete… (disabled on the last CV).
- Folder row menu: Rename…, Delete (only when empty).
- "+ New CV…" offers "Blank CV" and "New folder". ⌘N duplicates the open CV (default name `<slug>-copy`).
- On launch, or when the selected CV disappears, select the most recently modified CV (no `'main'` fallback).

### Other touch points

- `ui/src/api.ts` types and calls; `ui/src/lib/names.ts` (`cvTitle` no longer special-cases `main`);
  `App.tsx` `'main'` defaults; `ui/src/dev/mockTauri.ts` fixtures.
- `export.rs` file names: `name.replace('/', "-")` becomes just the slug.
- README "Library layout"; the agent setup prompt if it mentions `main`/`tailored`.
- Tests in `crates/core/tests/*` that use `main` / `tailored/acme` paths.

### Tests to add

- Migration: old layout → new layout, history rows renamed, re-run is a no-op, `tailored/main`
  collision, a root `main.cv.json` re-created after migration becomes `main-2`, history-only
  migration (files already moved, rows still `tailored/…`).
- Lookup across folders; duplicate slug reported as an error.
- `move_cv` keeps history; a Finder-style move (rename on disk + sweep) keeps history.
- Delete-last refused; pins survive rename and move, dropped on delete.

## Left to the builder

- Exact UI styling of the pinned section and folder rows (follow the existing Roman inscription style).
- How the "New CV…" choice is presented (small menu vs two buttons).
