import { describe, expect, it } from 'vitest'
import { type Cursor, step } from './undo.ts'

/** Apply a step the way the app does: the restore adds version `added` on top. */
function apply(ids: string[], c: Cursor | undefined, dir: 'undo' | 'redo', added: string) {
  const s = step(ids, c, dir)
  if (!s) return { ids, c, target: undefined }
  return { ids: [added, ...ids], c: { at: s.at, redo: s.redo, last: added }, target: s.target }
}

describe('undo step', () => {
  it('walks back through versions, skipping its own restores', () => {
    let r = apply(['3', '2', '1'], undefined, 'undo', '4')
    expect(r.target).toBe('2')
    r = apply(r.ids, r.c, 'undo', '5')
    expect(r.target).toBe('1')
    expect(step(r.ids, r.c, 'undo')).toBeUndefined() // nothing older
  })

  it('redo replays undone versions in order', () => {
    let r = apply(['3', '2', '1'], undefined, 'undo', '4')
    r = apply(r.ids, r.c, 'undo', '5')
    r = apply(r.ids, r.c, 'redo', '6')
    expect(r.target).toBe('2')
    r = apply(r.ids, r.c, 'redo', '7')
    expect(r.target).toBe('3')
    expect(step(r.ids, r.c, 'redo')).toBeUndefined()
    r = apply(r.ids, r.c, 'undo', '8')
    expect(r.target).toBe('2')
  })

  it('an outside edit after an undo starts over and drops redo', () => {
    const r = apply(['3', '2', '1'], undefined, 'undo', '4')
    const ids = ['9', ...r.ids] // an agent saved version 9
    expect(step(ids, r.c, 'redo')).toBeUndefined()
    expect(step(ids, r.c, 'undo')?.target).toBe('4')
  })

  it('does nothing without history', () => {
    expect(step([], undefined, 'undo')).toBeUndefined()
    expect(step(['1'], undefined, 'undo')).toBeUndefined()
  })
})
