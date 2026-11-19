import type { UiSchema } from '@rjsf/utils'

type Node = { type?: string | string[]; properties?: Record<string, Node>; items?: Node; format?: string }

const WORDS: Record<string, string> = { url: 'URL', isDetailed: 'Show details', studyType: 'Degree', countryCode: 'Country code', sideProjects: 'Side projects', entity: 'Organisation' }

/** "postalCode" -> "Postal code"; a few keys get a hand-written label. */
export function humanize(key: string): string {
  if (WORDS[key]) return WORDS[key]
  const words = key.replace(/([a-z0-9])([A-Z])/g, '$1 $2').toLowerCase()
  return words.charAt(0).toUpperCase() + words.slice(1)
}

const TEXTAREA = new Set(['summary', 'description', 'reference', 'highlights'])
const DATES = new Set(['startDate', 'endDate', 'date', 'releaseDate'])
/** Lists of short words, edited as chips. */
const TAGS = new Set(['keywords', 'courses', 'roles'])
const PLACEHOLDER: Record<string, string> = {
  label: 'Software Engineer', email: 'name@example.com', phone: '+44 20 7946 0000', url: 'https://', network: 'GitHub',
  username: 'username', countryCode: 'GB', fluency: 'Native', level: 'Advanced', studyType: 'BSc', area: 'Computer Science',
}

/**
 * Field order for an object (basics, location) or a list's items (work, ...). Fields sit two to a row,
 * so pairs that belong together (start and end date) are adjacent. Names must exist in the schema.
 */
export const ORDER: Record<string, string[]> = {
  basics: ['name', 'label', 'email', 'phone', 'url', 'image', 'summary', 'location', 'profiles', '*'],
  location: ['address', 'city', 'region', 'postalCode', 'countryCode', '*'],
  profiles: ['network', 'username', 'url', '*'],
  // The role reads first on a CV, so it comes before the employer.
  work: ['position', 'name', 'startDate', 'endDate', 'location', 'url', 'description', 'summary', 'highlights', '*'],
  volunteer: ['position', 'organization', 'startDate', 'endDate', 'url', 'summary', 'highlights', '*'],
  education: ['institution', 'url', 'studyType', 'area', 'startDate', 'endDate', 'score', 'courses', '*'],
  awards: ['title', 'awarder', 'date', 'summary', '*'],
  certificates: ['name', 'issuer', 'date', 'url', '*'],
  publications: ['name', 'publisher', 'releaseDate', 'url', 'summary', '*'],
  skills: ['name', 'level', 'keywords', 'summary', '*'],
  languages: ['language', 'fluency', 'summary', '*'],
  interests: ['name', 'keywords', '*'],
  references: ['name', 'reference', '*'],
  projects: ['name', 'entity', 'type', 'url', 'startDate', 'endDate', 'description', 'highlights', 'roles', 'keywords', '*'],
  sideProjects: ['name', 'url', 'startDate', 'endDate', 'description', 'keywords', '*'],
}

/** Labels that depend on the list: a job's "name" is the employer. */
const ITEM_TITLES: Record<string, Record<string, string>> = { work: { name: 'Company' } }

/** What one item of a list is called, for "Add …" buttons. */
export const SINGULAR: Record<string, string> = {
  work: 'position', volunteer: 'role', education: 'education', awards: 'award', certificates: 'certificate',
  publications: 'publication', skills: 'skill', languages: 'language', interests: 'interest', references: 'reference',
  projects: 'project', sideProjects: 'side project', profiles: 'profile', highlights: 'highlight',
}

/** Labels, widgets, placeholders and order for every field, derived from the schema's keys. */
export function buildUiSchema(node: Node, key?: string): UiSchema {
  const ui: UiSchema = {}
  if (key) ui['ui:title'] = humanize(key)
  if (key && PLACEHOLDER[key]) ui['ui:placeholder'] = PLACEHOLDER[key]
  if (key && DATES.has(key)) {
    ui['ui:placeholder'] = node.format === 'date' ? 'YYYY-MM-DD' : 'YYYY-MM'
    if (key === 'endDate') ui['ui:widget'] = 'endDate'
  } else if (key && TEXTAREA.has(key) && node.type === 'string') {
    ui['ui:widget'] = 'textarea'
  }
  if (key && TAGS.has(key) && node.type === 'array') return { ...ui, 'ui:field': 'tags' }
  if (key && ORDER[key] && node.properties) ui['ui:order'] = ORDER[key]
  for (const [k, v] of Object.entries(node.properties ?? {})) ui[k] = buildUiSchema(v, k)
  if (node.items?.type === 'object') {
    // List items get a card of their own, so the "Work-1" style label is noise.
    const items = buildUiSchema(node.items)
    for (const [k, t] of Object.entries(ITEM_TITLES[key ?? ''] ?? {})) items[k] = { ...items[k], 'ui:title': t }
    if (key && ORDER[key]) items['ui:order'] = ORDER[key]
    ui.items = items
  } else if (node.items) {
    ui.items = { 'ui:label': false, ...(key && TEXTAREA.has(key) ? { 'ui:widget': 'textarea', 'ui:options': { rows: 1 } } : {}) }
  }
  return ui
}

/** True for fields that take a whole row: long text, lists, groups and the photo. */
export const isWide = (key: string, node?: Node) =>
  TEXTAREA.has(key) || key === 'image' || node?.type === 'array' || node?.type === 'object'

const PRIMARY = ['position', 'title', 'institution', 'organization', 'language', 'network', 'name']
const SECONDARY = ['name', 'organization', 'area', 'fluency', 'username', 'awarder', 'issuer', 'publisher', 'level']

/** The heading of a list item's card, e.g. { title: "Senior Engineer", sub: "Northwind", when: "2020 – Present" }. */
export function itemSummary(item: unknown): { title: string; sub: string; when: string } {
  if (!item || typeof item !== 'object') return { title: '', sub: '', when: '' }
  const o = item as Record<string, unknown>
  const str = (k: string) => (typeof o[k] === 'string' ? (o[k] as string).trim() : '')
  const pk = PRIMARY.find((k) => str(k))
  const sk = SECONDARY.find((k) => k !== pk && str(k))
  const year = (k: string) => str(k).slice(0, 4)
  const when = year('startDate') ? `${year('startDate')} – ${year('endDate') || 'Present'}` : year('date') || year('releaseDate')
  return { title: pk ? str(pk) : '', sub: sk ? str(sk) : '', when }
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

/** ['work', 0, 'name'] -> '/work/0/name' (CV keys hold no '/' or '~'). */
export const pointer = (path: (string | number)[]) => (path.length ? `/${path.join('/')}` : '')

/** Splits a chip input's text into new tags: commas or new lines separate them. */
export const splitTags = (text: string) => text.split(/[,\n]/).map((t) => t.trim()).filter(Boolean)
