import { describe, expect, it } from 'vitest'
import type { Version } from '../api.ts'
import { groupVersions, relativeTime } from './history.ts'

const MIN = 60_000
const v = (author: string, min: number, message = 'edit'): Version => ({ commit: `${author}${min}`, author, message, time_ms: min * MIN })

describe('groupVersions', () => {
  it('groups same-author versions 1 min apart', () => {
    const g = groupVersions([v('You', 3), v('You', 2), v('You', 1)])
    expect(g).toHaveLength(1)
    expect(g[0].label).toBe('You: 3 edits')
    expect(g[0].newest.commit).toBe('You3')
  })
  it('splits on author change', () => {
    expect(groupVersions([v('You', 3), v('Agent', 2), v('You', 1)])).toHaveLength(3)
  })
  it('splits when 6 min apart, chains adjacent gaps', () => {
    expect(groupVersions([v('You', 7), v('You', 1)])).toHaveLength(2)
    expect(groupVersions([v('You', 10), v('You', 6), v('You', 2)])).toHaveLength(1)
  })
  it('labels a single version with its message', () => {
    expect(groupVersions([v('Agent', 1, 'tailor')])[0].label).toBe('Agent: tailor')
  })
})

describe('relativeTime', () => {
  const now = Date.UTC(2026, 10, 18, 12, 0) // Wed 18 Nov
  const ago = (ms: number) => relativeTime(now - ms, now, 'en-GB')
  it('formats ranges', () => {
    expect(ago(30_000)).toBe('now')
    expect(ago(2 * MIN)).toBe('2m')
    expect(ago(60 * MIN)).toBe('1h')
    expect(ago(23 * 60 * MIN)).toBe('23h')
    expect(ago(2 * 86_400_000)).toBe('Mon')
    expect(ago(15 * 86_400_000)).toBe('3 Nov')
  })
})
