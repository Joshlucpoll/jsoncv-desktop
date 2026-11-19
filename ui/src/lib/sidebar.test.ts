import { expect, test } from 'vitest'
import type { CvInfo } from '../api.ts'
import { type TopItem, cvFileChanged, dropPlace, groupCvs, moveKey, nudge, orderKeys, pickDefault } from './sidebar.ts'

const cv = (name: string, over: Partial<CvInfo> = {}): CvInfo =>
  ({ name, folder: null, pin: null, theme: null, continuous: false, modified_ms: 0, error: null, ...over })

const names = (l: CvInfo[]) => l.map((c) => c.name)
const tops = (l: TopItem[]) => l.map((t) => (t.kind === 'folder' ? `${t.name}/` : t.cv.name))

test('groups pinned by pin order and keeps them in their place', () => {
  const g = groupCvs([cv('a', { pin: 1 }), cv('b', { folder: 'jobs', pin: 0 }), cv('c')], ['jobs'])
  expect(names(g.pinned)).toEqual(['b', 'a'])
  expect(tops(g.top)).toEqual(['jobs/', 'a', 'c'])
  expect(g.top[0].kind === 'folder' && names(g.top[0].cvs)).toEqual(['b'])
})

test('without an order: folders first, then CVs, each A–Z; empty folders listed', () => {
  const g = groupCvs([cv('zeta'), cv('alpha'), cv('x', { folder: 'jobs' })], ['jobs', 'archive'])
  expect(tops(g.top)).toEqual(['archive/', 'jobs/', 'alpha', 'zeta'])
})

test('the order list sorts every level and mixes folders with CVs', () => {
  const cvs = [cv('a'), cv('b'), cv('x', { folder: 'jobs' }), cv('y', { folder: 'jobs' }), cv('new')]
  const g = groupCvs(cvs, ['jobs', 'old'], ['b', 'y', 'jobs/', 'a', 'x', 'gone'])
  // Listed first in list order; unlisted after (folders first, then A–Z).
  expect(tops(g.top)).toEqual(['b', 'jobs/', 'a', 'old/', 'new'])
  expect(g.top[1].kind === 'folder' && names(g.top[1].cvs)).toEqual(['y', 'x'])
})

test('orderKeys lists each folder followed by its CVs', () => {
  const g = groupCvs([cv('a'), cv('x', { folder: 'jobs' })], ['jobs'])
  expect(orderKeys(g.top)).toEqual(['jobs/', 'x', 'a'])
})

test('moveKey moves before a key, to the end, or not at all onto itself', () => {
  expect(moveKey(['a', 'b', 'c'], 'c', 'a')).toEqual(['c', 'a', 'b'])
  expect(moveKey(['a', 'b', 'c'], 'a', null)).toEqual(['b', 'c', 'a'])
  expect(moveKey(['a', 'b', 'c'], 'b', 'b')).toEqual(['a', 'b', 'c'])
  expect(moveKey(['a', 'b'], 'new', 'b')).toEqual(['a', 'new', 'b'])
})

const sample = () => groupCvs([cv('a'), cv('b'), cv('x', { folder: 'jobs' }), cv('y', { folder: 'jobs' })], ['jobs'])
// Display: jobs/ (x, y), a, b

test('dropPlace: CV rows give before/after places in their own container', () => {
  const { top } = sample()
  expect(dropPlace(top, 'b', 'a', 'before')).toEqual({ before: 'a', into: null })
  expect(dropPlace(top, 'b', 'a', 'after')).toEqual({ before: 'b', into: null })
  expect(dropPlace(top, 'a', 'b', 'after')).toEqual({ before: null, into: null })
  expect(dropPlace(top, 'a', 'x', 'after')).toEqual({ before: 'y', into: 'jobs' })
  expect(dropPlace(top, 'a', 'y', 'after')).toEqual({ before: null, into: 'jobs' })
})

test('dropPlace: folder rows take CVs into the folder or place before it', () => {
  const { top } = sample()
  expect(dropPlace(top, 'a', 'jobs/', 'into')).toEqual({ before: null, into: 'jobs' })
  expect(dropPlace(top, 'a', 'jobs/', 'before')).toEqual({ before: 'jobs/', into: null })
})

test('dropPlace: a dragged folder stays at the top level', () => {
  const g = groupCvs([cv('a'), cv('x', { folder: 'jobs' })], ['jobs', 'old'])
  // Display: jobs/ (x), old/, a
  expect(dropPlace(g.top, 'old/', 'jobs/', 'into')).toEqual({ before: 'jobs/', into: null })
  // Over a CV inside jobs: lands after jobs.
  expect(dropPlace(g.top, 'old/', 'x', 'before')).toEqual({ before: 'old/', into: null })
  expect(dropPlace(g.top, 'jobs/', 'a', 'after')).toEqual({ before: null, into: null })
})

test('nudge moves one place among siblings and stops at the ends', () => {
  const { top } = sample()
  expect(nudge(top, 'b', -1)).toEqual(['jobs/', 'x', 'y', 'b', 'a'])
  expect(nudge(top, 'jobs/', 1)).toEqual(['x', 'y', 'a', 'jobs/', 'b'])
  expect(nudge(top, 'x', 1)).toEqual(['jobs/', 'y', 'a', 'b', 'x'])
  expect(nudge(top, 'jobs/', -1)).toBeNull()
  expect(nudge(top, 'y', 1)).toBeNull()
})

test('pickDefault picks most recently modified', () => {
  expect(pickDefault([cv('old', { modified_ms: 1 }), cv('new', { modified_ms: 5 }), cv('mid', { modified_ms: 3 })])).toBe('new')
})

test('pickDefault returns undefined for no CVs', () => {
  expect(pickDefault([])).toBeUndefined()
})

test('cvFileChanged matches the CV file in any folder', () => {
  expect(cvFileChanged(['cvs/stripe.cv.json'], 'stripe')).toBe(true)
  expect(cvFileChanged(['cvs/jobs/stripe.cv.json'], 'stripe')).toBe(true)
  expect(cvFileChanged(['cvs/my-stripe.cv.json'], 'stripe')).toBe(false)
  expect(cvFileChanged(['themes/x/style.css'], 'stripe')).toBe(false)
  expect(cvFileChanged(['cvs/a.cv.json'], '')).toBe(false)
})
