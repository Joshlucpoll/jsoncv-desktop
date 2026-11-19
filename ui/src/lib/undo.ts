/** Where ⌘Z/⇧⌘Z stand for one CV. Undo and redo restore an old version, which adds a new one. */
export type Cursor = {
  /** Version whose content the CV shows now. */
  at: string
  /** The version our last undo/redo added; any other newest version means someone edited since. */
  last: string
  /** Versions undone, newest last. */
  redo: string[]
}

/**
 * The version to restore for an undo or redo, and the cursor to keep once it is restored
 * (minus `last`, which the restore returns). `ids` is the CV's history, newest first.
 */
export function step(ids: string[], cursor: Cursor | undefined, dir: 'undo' | 'redo'): { target: string; at: string; redo: string[] } | undefined {
  const live = cursor && ids[0] === cursor.last ? cursor : undefined // an outside edit resets undo and drops redo
  const at = live?.at ?? ids[0]
  const redo = live?.redo ?? []
  if (at === undefined) return
  if (dir === 'redo') {
    const target = redo.at(-1)
    return target === undefined ? undefined : { target, at: target, redo: redo.slice(0, -1) }
  }
  const i = ids.indexOf(at)
  const target = i < 0 ? undefined : ids[i + 1]
  return target === undefined ? undefined : { target, at: target, redo: [...redo, at] }
}
