# Sidebar reordering (drag and drop)

## Intent
Let the user put CVs and folders in their own order instead of A–Z, and drag CVs into and out of folders.

## Approaches
- **A. Drag and drop** (chosen by the user). Natural; also replaces most uses of "Move to".
- B. "Move up / Move down" menu items. Small but slow for long lists.

## Design
- **Order file:** `<library>/.jsoncv-order.json`, one JSON array of keys in display order. A CV key is its slug
  (`stripe`); a folder key is its name plus `/` (`jobs/`), so a CV and a folder can share a name.
  One list covers every level: the top level (folders and loose CVs, interleaved) and the CVs inside each folder
  use their relative positions in it. Items missing from the list go after the listed ones, A–Z.
  Unknown keys are ignored. Missing or unreadable file = no custom order (today's A–Z).
- **Pinned** CVs keep their own order in `.jsoncv-pins.json`; dragging within Pinned rewrites that list.
- **Core:** `order()`/`set_order(keys)` (atomic write, under the lock). `rename_cv` and `rename_folder` replace
  the old key; `delete_cv`/`delete_folder` drop it. `set_pin_order(slugs)` rewrites the pins list.
  Tauri commands `get_order`, `set_order`, `set_pin_order`. No agent (MCP) tools.
- **Drag:** pointer events (not HTML5 drag and drop, which Tauri's file-drop handling can swallow). A drag starts
  after 4 px of movement, so clicks still select. While dragging, a line shows where the item will land.
  - Drop between rows: reorder there. If that place is in another folder (or the top level), the CV moves there too.
  - Drop on the middle of a folder row: move the CV into that folder, at its end.
  - Folders move only at the top level (no nested folders). Pinned rows reorder only within Pinned.
  - Escape cancels a drag.
- **Keyboard:** with a row focused, Alt+↑ / Alt+↓ moves it one place (same rules). "Move to" stays in the menu.
- **Pure logic** in `ui/src/lib/sidebar.ts`, unit-tested: `groupCvs(cvs, folders, order)` sorts by the order;
  `reorder(...)` returns the new key list.

## Non-goals
Nested folders; agent tools for order; reordering by sort options (name/date).
