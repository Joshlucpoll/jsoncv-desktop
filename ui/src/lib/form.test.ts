import { describe, expect, it } from 'vitest'
import { buildUiSchema, errorsUnder, humanize, itemSummary, shrank } from './form.ts'

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
})

describe('itemSummary', () => {
  it('uses position and employer for work', () => {
    expect(itemSummary({ name: 'Northwind', position: 'Engineer' })).toEqual({ title: 'Engineer', sub: 'Northwind' })
  })
  it('falls back to name alone, and to empty', () => {
    expect(itemSummary({ name: 'tinylog', url: 'x' })).toEqual({ title: 'tinylog', sub: '' })
    expect(itemSummary({})).toEqual({ title: '', sub: '' })
    expect(itemSummary('text')).toEqual({ title: '', sub: '' })
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
