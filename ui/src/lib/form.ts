import type { UiSchema } from '@rjsf/utils'

type Node = { type?: string | string[]; properties?: Record<string, Node>; items?: Node; format?: string }

const WORDS: Record<string, string> = { url: 'URL', isDetailed: 'Show details', studyType: 'Degree', countryCode: 'Country code' }

/** "postalCode" -> "Postal code"; a few keys get a hand-written label. */
export function humanize(key: string): string {
  if (WORDS[key]) return WORDS[key]
  const words = key.replace(/([a-z0-9])([A-Z])/g, '$1 $2').toLowerCase()
  return words.charAt(0).toUpperCase() + words.slice(1)
}

const TEXTAREA = new Set(['summary', 'description', 'reference', 'highlights'])
const DATES = new Set(['startDate', 'endDate', 'date', 'releaseDate'])

/** Labels, widgets and placeholders for every field, derived from the schema's keys. */
export function buildUiSchema(node: Node, key?: string): UiSchema {
  const ui: UiSchema = {}
  if (key) ui['ui:title'] = humanize(key)
  if (key && DATES.has(key)) {
    ui['ui:placeholder'] = node.format === 'date' ? 'YYYY-MM-DD' : 'YYYY-MM'
    if (key === 'endDate') ui['ui:widget'] = 'endDate'
  } else if (key && TEXTAREA.has(key) && node.type === 'string') {
    ui['ui:widget'] = 'textarea'
  }
  for (const [k, v] of Object.entries(node.properties ?? {})) ui[k] = buildUiSchema(v, k)
  if (node.items) {
    // List items get a card or a row of their own, so the "Work-1" style label is noise.
    ui.items = node.items.type === 'object' ? buildUiSchema(node.items) : { 'ui:label': false, ...(key && TEXTAREA.has(key) ? { 'ui:widget': 'textarea', 'ui:options': { rows: 2 } } : {}) }
  }
  return ui
}

const PRIMARY = ['position', 'title', 'institution', 'organization', 'language', 'network', 'name']
const SECONDARY = ['name', 'organization', 'area', 'fluency', 'username', 'awarder', 'issuer', 'publisher', 'level']

/** The heading of a list item's card, e.g. { title: "Senior Engineer", sub: "Northwind" }. */
export function itemSummary(item: unknown): { title: string; sub: string } {
  if (!item || typeof item !== 'object') return { title: '', sub: '' }
  const o = item as Record<string, unknown>
  const str = (k: string) => (typeof o[k] === 'string' ? (o[k] as string).trim() : '')
  const pk = PRIMARY.find((k) => str(k))
  const sk = SECONDARY.find((k) => k !== pk && str(k))
  return { title: pk ? str(pk) : '', sub: sk ? str(sk) : '' }
}

/** True when some list in `next` is shorter than the same list in `prev` (an item was removed). */
export function shrank(prev: unknown, next: unknown): boolean {
  if (Array.isArray(prev) && Array.isArray(next)) return next.length < prev.length || next.some((x, i) => shrank(prev[i], x))
  if (prev && next && typeof prev === 'object' && typeof next === 'object') {
    return Object.keys(next).some((k) => shrank((prev as Record<string, unknown>)[k], (next as Record<string, unknown>)[k]))
  }
  return false
}

/** Number of validation errors at `path` or below it; `errors` are JSON Pointer instance paths. */
export function errorsUnder(errors: string[], path: (string | number)[]): number {
  const prefix = path.length ? `/${path.join('/')}` : ''
  return errors.filter((p) => p === prefix || p.startsWith(`${prefix}/`)).length
}
