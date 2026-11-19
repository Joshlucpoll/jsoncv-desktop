// Maps between CV fields (JSON Pointer paths) and elements of the rendered preview by matching text.
// Themes are free-form templates, so text is the one link every theme keeps.

/** A text value in the CV and where it lives, e.g. { path: '/work/0/position', norm: 'engineer' }. */
export type Leaf = { path: string; norm: string }
/** The parts of a DOM element this module needs (lets tests use plain objects). */
export type El = { parentElement: El | null; textContent: string | null }
export type Prepared = { els: El[]; texts: string[]; index: Map<El, number> }

/** Lowercase letters and digits only: markdown marks, separators, spacing and URL schemes drop out. */
export const norm = (s: string) =>
  s.replace(/\[([^\]]*)\]\([^)]*\)/g, '$1').replace(/^[a-z]+:\/\/(www\.)?/i, '').toLowerCase().replace(/[^\p{L}\p{N}]+/gu, '')

/** String and number values in document order; tool settings (meta, $schema) are not rendered. */
export function leaves(cv: unknown, path = ''): Leaf[] {
  if (typeof cv === 'string' || typeof cv === 'number') {
    const n = norm(String(cv))
    return n ? [{ path, norm: n }] : []
  }
  if (!cv || typeof cv !== 'object') return []
  return Object.entries(cv).flatMap(([k, v]) => (path === '' && (k === 'meta' || k === '$schema') ? [] : leaves(v, `${path}/${k}`)))
}

/** `els` must be in document order (querySelectorAll('*')). */
export function prepare(els: El[]): Prepared {
  return { els, texts: els.map((e) => norm(e.textContent ?? '')), index: new Map(els.map((e, i) => [e, i])) }
}

const contains = (a: El, b: El) => {
  for (let e: El | null = b; e; e = e.parentElement) if (e === a) return true
  return false
}
const textOf = (p: Prepared, e: El | null) => (e && p.index.has(e) ? p.texts[p.index.get(e)!] : undefined)
/** Elements whose text is exactly `t`, outermost of each nested run (an <li> and its lone <a> count once). */
const exactEls = (p: Prepared, t: string) => p.els.filter((e, i) => p.texts[i] === t && textOf(p, e.parentElement) !== t)
/** Elements whose text includes `t` but none of whose descendants does. */
const deepestEls = (p: Prepared, t: string) => {
  const hits = p.els.filter((_, i) => p.texts[i].includes(t))
  return hits.filter((e, i) => !(hits[i + 1] && contains(e, hits[i + 1])))
}
/** The k-th of `list`, or its last: the k-th leaf with some text most likely renders as the k-th element with it. */
const nth = <T>(list: T[], k: number): T | undefined => list[Math.min(k, list.length - 1)]

function elementForLeaf(p: Prepared, ls: Leaf[], leaf: Leaf): El | undefined {
  const k = ls.filter((l) => l.norm === leaf.norm).indexOf(leaf)
  const exact = exactEls(p, leaf.norm)
  if (exact.length) return nth(exact, k)
  // Short values ("Go", "UK") would match inside unrelated words.
  return leaf.norm.length >= 4 ? nth(deepestEls(p, leaf.norm), k) : undefined
}

const parentPath = (path: string) => path.slice(0, path.lastIndexOf('/'))

/** The element showing `path`: the field itself, or the smallest element around everything an item or section renders. */
export function elementFor(p: Prepared, ls: Leaf[], path: string): El | undefined {
  if (!path) return undefined
  const under = ls.filter((l) => l.path === path || l.path.startsWith(`${path}/`)).slice(0, 40)
  const found = under.map((l) => elementForLeaf(p, ls, l)).filter((e): e is El => !!e)
  // Nothing rendered as typed (a date shown as "Jan 2020"): show the item it belongs to instead.
  if (!found.length) return path.lastIndexOf('/') > 0 ? elementFor(p, ls, parentPath(path)) : undefined
  let lca: El | null = found[0]
  while (lca && !found.every((e) => contains(lca!, e))) lca = lca.parentElement
  return lca ?? undefined
}

const commonPrefix = (paths: string[]) => {
  const parts = paths.map((x) => x.split('/'))
  let n = 0
  while (parts.every((x) => x[n] !== undefined && x[n] === parts[0][n])) n++
  return parts[0].slice(0, n).join('/')
}

/** The CV path a click on `el` points at: an exact field, else the item or section around it. */
export function pathAt(p: Prepared, ls: Leaf[], el: El): string | undefined {
  for (let e: El | null = el; e && p.index.has(e); e = e.parentElement) {
    const t = textOf(p, e)!
    if (!t) continue
    const exact = ls.filter((l) => l.norm === t)
    if (exact.length) {
      let o = e
      while (o.parentElement && textOf(p, o.parentElement) === t) o = o.parentElement
      return nth(exact, Math.max(0, exactEls(p, t).indexOf(o)))!.path
    }
    const inside = ls.filter((l) => l.norm.length >= 4 && t.includes(l.norm))
    if (!inside.length) continue
    // Group by item (/work/0). One item with real text: that item (a stray short match elsewhere is ignored).
    // Several: whatever holds them all, such as the section.
    const groups = new Map<string, Leaf[]>()
    for (const l of inside) {
      const key = l.path.split('/').slice(0, 3).join('/')
      groups.set(key, [...(groups.get(key) ?? []), l])
    }
    const size = (g: Leaf[]) => g.reduce((n, l) => n + l.norm.length, 0)
    const all = [...groups.values()]
    const real = all.filter((g) => size(g) >= 8)
    const pick = real.length > 1 ? real.flat() : real[0] ?? all.reduce((a, b) => (size(b) > size(a) ? b : a))
    const prefix = commonPrefix(pick.map((l) => l.path))
    // Part of a field's text matched alongside other text: point at the item that holds it.
    return ls.some((l) => l.path === prefix) ? parentPath(prefix) || undefined : prefix || undefined
  }
  return undefined
}
