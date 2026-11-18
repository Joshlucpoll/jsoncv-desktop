import { useEffect, useRef, useState } from 'react'
import { diff, errorMessage } from '../api.ts'

type Props = { name: string; commit: string; onClose(): void }

const cls = (l: string) =>
  l.startsWith('@@') ? 'diff-hunk' : l.startsWith('+') && !l.startsWith('+++') ? 'diff-add' : l.startsWith('-') && !l.startsWith('---') ? 'diff-del' : undefined

/** Unified diff overlay (version -> current); shared by the edit panel and history. */
export function DiffView({ name, commit, onClose }: Props) {
  const [text, setText] = useState<string>()
  const [error, setError] = useState<string>()
  const close = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    diff(name, commit).then(setText, (e) => setError(errorMessage(e)))
  }, [name, commit])

  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null
    close.current?.focus()
    return () => prev?.focus()
  }, [])

  return (
    <div className="overlay" onClick={onClose}>
      <div
        className="dialog diff"
        role="dialog"
        aria-modal="true"
        aria-label="Changes"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === 'Escape') { e.stopPropagation(); onClose() }
          // Only one focusable control (Close): keep Tab inside the modal.
          else if (e.key === 'Tab') { e.preventDefault(); close.current?.focus() }
        }}
      >
        <header>
          <strong>Changes</strong>
          <span className="spacer" />
          <button ref={close} onClick={onClose}>Close</button>
        </header>
        <pre tabIndex={-1}>
          {error && <span className="diff-del">{error}</span>}
          {text === undefined && !error && 'Loading…'}
          {text?.trim() === '' && 'No changes.'}
          {text?.split('\n').map((l, i) => <div key={i} className={cls(l)}>{l || ' '}</div>)}
        </pre>
      </div>
    </div>
  )
}
