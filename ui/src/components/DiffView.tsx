import { useEffect, useRef, useState } from 'react'
import { changeDiff, diff, errorMessage } from '../api.ts'

export type DiffMode = 'change' | 'current'
type Props = { name: string; commit: string; onClose(): void; mode?: DiffMode; toggle?: boolean; title?: string }

const cls = (l: string) =>
  l.startsWith('@@') ? 'diff-hunk' : l.startsWith('+') && !l.startsWith('+++') ? 'diff-add' : l.startsWith('-') && !l.startsWith('---') ? 'diff-del' : undefined

/** Diff overlay: what a version changed ("change"), or that version against the current CV ("current"). */
export function DiffView({ name, commit, onClose, mode: initial = 'current', toggle = false, title = 'Changes' }: Props) {
  const [mode, setMode] = useState<DiffMode>(initial)
  const [result, setResult] = useState<{ key: string; text?: string; error?: string }>()
  const box = useRef<HTMLDivElement>(null)
  const key = `${mode}|${commit}`

  useEffect(() => {
    let live = true
    ;(mode === 'change' ? changeDiff : diff)(name, commit).then(
      (text) => live && setResult({ key, text }),
      (e) => live && setResult({ key, error: errorMessage(e) }),
    )
    return () => { live = false }
  }, [name, commit, mode, key])

  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null
    box.current?.querySelector<HTMLElement>('button')?.focus()
    return () => prev?.focus()
  }, [])

  const shown = result?.key === key ? result : undefined
  return (
    <div className="overlay" onClick={onClose}>
      <div
        ref={box}
        className="dialog diff"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === 'Escape') { e.stopPropagation(); onClose() }
          else if (e.key === 'Tab') {
            // Keep Tab inside the modal.
            const els = [...(box.current?.querySelectorAll<HTMLElement>('button') ?? [])]
            const i = els.indexOf(document.activeElement as HTMLElement)
            e.preventDefault()
            els[(i + (e.shiftKey ? -1 : 1) + els.length) % els.length]?.focus()
          }
        }}
      >
        <header>
          <strong>{title}</strong>
          {toggle && (
            <div className="tabs" role="tablist" aria-label="Compare">
              <button role="tab" aria-selected={mode === 'change'} onClick={() => setMode('change')}>This change</button>
              <button role="tab" aria-selected={mode === 'current'} onClick={() => setMode('current')}>Compared with now</button>
            </div>
          )}
          <span className="spacer" />
          <button onClick={onClose}>Close</button>
        </header>
        <pre tabIndex={-1}>
          {shown?.error && <span className="diff-del">{shown.error}</span>}
          {!shown && 'Loading…'}
          {shown?.text?.trim() === '' && 'No changes.'}
          {shown?.text?.split('\n').map((l, i) => <div key={i} className={cls(l)}>{l || ' '}</div>)}
        </pre>
      </div>
    </div>
  )
}
