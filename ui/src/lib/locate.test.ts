import { describe, expect, it } from 'vitest'
import { type El, elementFor, leaves, norm, pathAt, prepare } from './locate.ts'

// A tiny DOM stand-in: text is the concatenation of children, as textContent is.
type N = El & { tag: string; kids: N[] }
function h(tag: string, ...kids: (N | string)[]): N {
  const n: N = { tag, kids: [], parentElement: null, textContent: '' }
  const own: string[] = []
  for (const k of kids) {
    if (typeof k === 'string') own.push(k)
    else { k.parentElement = n; n.kids.push(k); own.push(k.textContent ?? '') }
  }
  n.textContent = own.join('')
  return n
}
const flat = (n: N): N[] => [n, ...n.kids.flatMap(flat)]

const cv = {
  meta: { theme: 'x' },
  basics: { name: 'Ada Lovelace', url: 'https://ada.dev', location: { city: 'London' } },
  work: [
    { position: 'Programmer', name: 'Engines Ltd', startDate: '1842-01', highlights: ['Wrote the **first** program'] },
    { position: 'Analyst', name: 'Babbage & Co', startDate: '1840-01' },
  ],
  skills: [{ name: 'Maths', keywords: ['Algebra', 'Calculus'] }],
}
const name = h('h1', 'Ada Lovelace')
const url = h('a', 'ada.dev')
const city = h('li', 'London')
const job0 = h('article', h('h3', 'Programmer'), h('span', 'Jan 1842 – Present'), h('p', 'Engines Ltd'), h('li', 'Wrote the first program'))
const job1 = h('article', h('h3', 'Analyst'), h('span', 'Jan 1840 – Present'), h('p', 'Babbage · Co'))
const work = h('section', h('h2', 'Work'), job0, job1)
const kw = h('dd', 'Algebra · Calculus')
const skill = h('div', h('dt', 'Maths'), kw)
const body = h('body', h('header', name, h('ul', h('li', url), city)), work, h('section', h('h2', 'Skills'), skill))
const p = prepare(flat(body))
const ls = leaves(cv)

describe('norm and leaves', () => {
  it('drops markdown, separators and URL schemes', () => {
    expect(norm('Wrote the **first** [program](https://x.y)')).toBe('wrotethefirstprogram')
    expect(norm('https://www.ada.dev')).toBe('adadev')
  })
  it('skips meta', () => {
    expect(ls.some((l) => l.path.startsWith('/meta'))).toBe(false)
    expect(ls[0]).toEqual({ path: '/basics/name', norm: 'adalovelace' })
  })
})

describe('pathAt', () => {
  it('finds the exact field, markdown and URLs included', () => {
    expect(pathAt(p, ls, name)).toBe('/basics/name')
    expect(pathAt(p, ls, url)).toBe('/basics/url')
    expect(pathAt(p, ls, job0.kids[3])).toBe('/work/0/highlights/0')
    expect(pathAt(p, ls, job1.kids[2])).toBe('/work/1/name')
  })
  it('falls back to the item, then the section', () => {
    expect(pathAt(p, ls, job0.kids[1])).toBe('/work/0') // dates: the row climbs to the article
    expect(pathAt(p, ls, kw)).toBe('/skills/0/keywords')
    expect(pathAt(p, ls, work.kids[0])).toBe('/work') // the "Work" heading
    expect(pathAt(p, ls, body)).toBeUndefined()
  })
})

describe('elementFor', () => {
  it('finds a field, an item and a section', () => {
    expect(elementFor(p, ls, '/basics/location/city')).toBe(city)
    expect(elementFor(p, ls, '/work/1')).toBe(job1)
    expect(elementFor(p, ls, '/work')).toBe(work)
    expect(elementFor(p, ls, '/skills/0/keywords/1')).toBe(kw)
  })
  it('shows the item when a value is rendered differently', () => {
    expect(elementFor(p, ls, '/work/0/startDate')).toBe(job0)
    expect(elementFor(p, ls, '/nope')).toBeUndefined()
  })
})
