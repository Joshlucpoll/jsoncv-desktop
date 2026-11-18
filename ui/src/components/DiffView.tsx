import { useEffect, useState } from 'react'
import { diff, errorMessage } from '../api.ts'

type Props = { name: string; commit: string; onClose(): void }

/** Unified diff overlay; shared by the edit panel (Task 13) and history (Task 14). */
export function DiffView({ name, commit, onClose }: Props) {
  const [text, setText] = useState<string>()
  const [error, setError] = useState<string>()

  useEffect(() => {
    diff(name, commit).then(setText, (e) => setError(errorMessage(e)))
  }, [name, commit])

  return (
    <div className="overlay" onClick={onClose}>
      <div className="dialog diff" role="dialog" aria-label="Changes" onClick={(e) => e.stopPropagation()}>
        <header>
          <strong>Changes</strong>
          <span className="spacer" />
          <button onClick={onClose}>Close</button>
        </header>
        <pre>
          {error && <span className="diff-del">{error}</span>}
          {text === undefined && !error && 'Loading…'}
          {text?.trim() === '' && 'No changes.'}
          {text?.split('\n').map((l, i) => (
            <div key={i} className={l.startsWith('+') && !l.startsWith('+++') ? 'diff-add' : l.startsWith('-') && !l.startsWith('---') ? 'diff-del' : l.startsWith('@@') ? 'diff-hunk' : undefined}>
              {l || ' '}
            </div>
          ))}
        </pre>
      </div>
    </div>
  )
}
