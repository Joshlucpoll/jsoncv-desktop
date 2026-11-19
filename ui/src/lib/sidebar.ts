import type { CvInfo } from '../api.ts'

/** A top-level sidebar entry: a folder with its CVs, or a CV outside any folder. */
export type TopItem = { kind: 'folder'; name: string; cvs: CvInfo[] } | { kind: 'cv'; cv: CvInfo }

/** Order-file key of a folder; a trailing `/` keeps it apart from a CV of the same name. */
export const folderKey = (name: string) => `${name}/`
const keyOf = (t: TopItem) => (t.kind === 'folder' ? folderKey(t.name) : t.cv.name)

/**
 * Sidebar sections: pinned CVs (pin order), then the top level, where folders and CVs follow `order`
 * (the `.jsoncv-order.json` keys). Unlisted items come after the listed ones: folders first, then A–Z.
 * CVs inside a folder follow the same rule. A pinned CV also stays in its place.
 */
export function groupCvs(cvs: CvInfo[], folders: string[], order: string[] = []) {
  const rank = new Map(order.map((k, i) => [k, i]))
  const sorted = <T,>(items: T[], key: (t: T) => string) =>
    [...items].sort((a, b) => {
      const ka = key(a), kb = key(b)
      const ra = rank.get(ka) ?? Infinity, rb = rank.get(kb) ?? Infinity
      if (ra !== rb) return ra < rb ? -1 : 1
      if (ka.endsWith('/') !== kb.endsWith('/')) return ka.endsWith('/') ? -1 : 1
      return ka.localeCompare(kb)
    })
  const items: TopItem[] = [
    ...folders.map((name): TopItem => ({ kind: 'folder', name, cvs: sorted(cvs.filter((c) => c.folder === name), (c) => c.name) })),
    ...cvs.filter((c) => c.folder === null).map((cv): TopItem => ({ kind: 'cv', cv })),
  ]
  return {
    pinned: cvs.filter((c) => c.pin !== null).sort((a, b) => a.pin! - b.pin!),
    top: sorted(items, keyOf),
  }
}

/** Every key in display order, each folder followed by its CVs: a complete order list. */
export const orderKeys = (top: TopItem[]) => top.flatMap((t) => (t.kind === 'folder' ? [folderKey(t.name), ...t.cvs.map((c) => c.name)] : [t.cv.name]))

/** `seq` with `key` moved to just before `before` (to the end when null). Only relative order within a level matters. */
export function moveKey(seq: string[], key: string, before: string | null): string[] {
  if (before === key) return seq
  const s = seq.filter((k) => k !== key)
  const i = before === null ? -1 : s.indexOf(before)
  if (i < 0) s.push(key)
  else s.splice(i, 0, key)
  return s
}

/** Where a drop lands: insert before `before` (null: at the end), in folder `into` (null: the top level). */
export type Place = { before: string | null; into: string | null }

/** The place for dropping `drag` on row `anchor`: above it, below it, or (folder rows) into it. */
export function dropPlace(top: TopItem[], drag: string, anchor: string, side: 'before' | 'after' | 'into'): Place {
  const topKeys = top.map(keyOf)
  const nextTop = (k: string) => topKeys[topKeys.indexOf(k) + 1] ?? null
  const parent = top.find((t) => t.kind === 'folder' && t.cvs.some((c) => c.name === anchor)) as Extract<TopItem, { kind: 'folder' }> | undefined
  if (drag.endsWith('/')) {
    // Folders stay at the top level; over a row inside a folder, the folder lands after that folder.
    if (parent) return { before: nextTop(folderKey(parent.name)), into: null }
    return { before: side === 'after' ? nextTop(anchor) : anchor, into: null }
  }
  if (anchor.endsWith('/')) return side === 'into' ? { before: null, into: anchor.slice(0, -1) } : { before: anchor, into: null }
  if (parent) {
    const sib = parent.cvs.map((c) => c.name)
    return { before: side === 'after' ? (sib[sib.indexOf(anchor) + 1] ?? null) : anchor, into: parent.name }
  }
  return { before: side === 'after' ? nextTop(anchor) : anchor, into: null }
}

/** The order after moving `key` one place up (-1) or down (1) among its siblings; null at either end. */
export function nudge(top: TopItem[], key: string, dir: -1 | 1): string[] | null {
  const parent = top.find((t) => t.kind === 'folder' && t.cvs.some((c) => c.name === key)) as Extract<TopItem, { kind: 'folder' }> | undefined
  const sib = parent ? parent.cvs.map((c) => c.name) : top.map(keyOf)
  const i = sib.indexOf(key)
  if (i < 0 || !sib[i + dir]) return null
  return moveKey(orderKeys(top), key, dir < 0 ? sib[i - 1] : (sib[i + 2] ?? null))
}

/** The CV to open when none is selected: the most recently modified one. */
export const pickDefault = (cvs: CvInfo[]): string | undefined =>
  cvs.reduce<CvInfo | undefined>((best, c) => (!best || c.modified_ms > best.modified_ms ? c : best), undefined)?.name

/** Whether a `library-changed` path list touches CV `name`'s file, in whichever folder it sits. */
export const cvFileChanged = (paths: string[], name: string) => !!name && paths.some((p) => p.endsWith(`/${name}.cv.json`))
