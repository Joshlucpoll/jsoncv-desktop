import { expect, test } from 'vitest'
import type { CvInfo } from '../api.ts'
import { cvFileChanged, groupCvs, pickDefault } from './sidebar.ts'

const cv = (name: string, over: Partial<CvInfo> = {}): CvInfo =>
  ({ name, folder: null, pin: null, theme: null, continuous: false, modified_ms: 0, error: null, ...over })

const names = (l: CvInfo[]) => l.map((c) => c.name)

test('groups pinned by pin order and keeps them in their folder', () => {
  const g = groupCvs([cv('a', { pin: 1 }), cv('b', { folder: 'jobs', pin: 0 }), cv('c')], ['jobs'])
  expect(names(g.pinned)).toEqual(['b', 'a'])
  expect(names(g.folders[0].cvs)).toEqual(['b'])
  expect(names(g.loose)).toEqual(['a', 'c'])
})

test('lists empty folders', () => {
  const g = groupCvs([cv('a', { folder: 'jobs' })], ['archive', 'jobs'])
  expect(g.folders.map((f) => f.name)).toEqual(['archive', 'jobs'])
  expect(g.folders[0].cvs).toEqual([])
  expect(g.pinned).toEqual([])
})

test('sorts loose CVs by name', () => {
  expect(names(groupCvs([cv('zeta'), cv('alpha'), cv('mid')], []).loose)).toEqual(['alpha', 'mid', 'zeta'])
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
