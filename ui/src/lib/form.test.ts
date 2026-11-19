import { describe, expect, it } from 'vitest'
import realSchema from '../../../crates/core/schema/jsoncv.schema.json'
import { ORDER, buildUiSchema, errorsUnder, humanize, itemSummary, pointer, shrank, splitTags } from './form.ts'

describe('humanize', () => {
  it('splits camelCase and keeps hand-written labels', () => {
    expect(humanize('postalCode')).toBe('Postal code')
    expect(humanize('sideProjects')).toBe('Side projects')
    expect(humanize('url')).toBe('URL')
    expect(humanize('basics')).toBe('Basics')
  })
})

describe('buildUiSchema', () => {
  const schema = {
    type: 'object',
    properties: {
      work: { type: 'array', items: { type: 'object', properties: { endDate: {}, summary: { type: 'string' }, highlights: { type: 'array', items: { type: 'string' } } } } },
      certificates: { type: 'array', items: { type: 'object', properties: { date: { type: 'string', format: 'date' } } } },
    },
  }
  const ui = buildUiSchema(schema)
  it('labels fields and picks widgets', () => {
    expect(ui.work['ui:title']).toBe('Work')
    expect(ui.work.items.endDate['ui:widget']).toBe('endDate')
    expect(ui.work.items.endDate['ui:placeholder']).toBe('YYYY-MM')
    expect(ui.work.items.summary['ui:widget']).toBe('textarea')
    expect(ui.certificates.items.date['ui:placeholder']).toBe('YYYY-MM-DD')
  })
  it('drops labels on primitive list items', () => {
    expect(ui.work.items.highlights.items['ui:label']).toBe(false)
    expect(ui.work.items.highlights.items['ui:widget']).toBe('textarea')
  })
  it('orders fields only by names the schema has (rjsf rejects others)', () => {
    type N = { properties?: Record<string, N>; items?: N }
    // The field names of the object at key `k` (or of its list's items), wherever it sits.
    const find = (n: N, k: string): string[] | undefined => {
      for (const [key, v] of Object.entries(n.properties ?? {})) {
        if (key === k) return Object.keys((v.items ?? v).properties ?? {})
        const deeper = find(v.items ?? v, k)
        if (deeper) return deeper
      }
    }
    const props = (k: string) => find(realSchema as N, k) ?? []
    for (const [k, order] of Object.entries(ORDER)) for (const f of order) if (f !== '*') expect(props(k), `${k}.${f}`).toContain(f)
    const real = buildUiSchema(realSchema as never)
    expect(real.work.items['ui:order'][0]).toBe('position')
    expect(real.work.items.name['ui:title']).toBe('Company')
    expect(real.basics['ui:order'][0]).toBe('name')
    expect(real.skills.items.keywords['ui:field']).toBe('tags')
  })
})

describe('itemSummary', () => {
  it('uses position and employer for work', () => {
    expect(itemSummary({ name: 'Northwind', position: 'Engineer', startDate: '2020-03' })).toEqual({ title: 'Engineer', sub: 'Northwind', when: '2020 – Present' })
    expect(itemSummary({ title: 'Prize', date: '2019-05-01' }).when).toBe('2019')
  })
  it('falls back to name alone, and to empty', () => {
    expect(itemSummary({ name: 'tinylog', url: 'x' })).toEqual({ title: 'tinylog', sub: '', when: '' })
    expect(itemSummary({})).toEqual({ title: '', sub: '', when: '' })
    expect(itemSummary('text')).toEqual({ title: '', sub: '', when: '' })
  })
})

describe('shrank', () => {
  it('detects a removed item at any depth', () => {
    expect(shrank({ work: [1, 2] }, { work: [1] })).toBe(true)
    expect(shrank({ work: [{ h: ['a', 'b'] }] }, { work: [{ h: ['a'] }] })).toBe(true)
  })
  it('ignores additions and edits', () => {
    expect(shrank({ work: [1] }, { work: [1, 2] })).toBe(false)
    expect(shrank({ basics: { name: 'a' } }, { basics: { name: '' } })).toBe(false)
  })
})

describe('errorsUnder', () => {
  it('counts errors at and below a path, not siblings', () => {
    const e = ['/work/0/url', '/work', '/workshop/0', '/basics/email']
    expect(errorsUnder(e, ['work'])).toBe(2)
    expect(errorsUnder(e, ['work', 0])).toBe(1)
    expect(errorsUnder(e, [])).toBe(4)
  })
})

describe('pointer and splitTags', () => {
  it('builds JSON pointers and splits pasted tag lists', () => {
    expect(pointer(['work', 0, 'name'])).toBe('/work/0/name')
    expect(pointer([])).toBe('')
    expect(splitTags(' Rust, C ,\nPython,, ')).toEqual(['Rust', 'C', 'Python'])
  })
})
