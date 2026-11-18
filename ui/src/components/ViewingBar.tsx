import { useState } from 'react'
import { ask } from '@tauri-apps/plugin-dialog'
import { MOCK, type Version, restore } from '../api.ts'
import { relativeTime } from '../lib/history.ts'
import { DiffView } from './DiffView.tsx'

type Props = { name: string; version: Version; onBack: () => void; onRestored: () => void; onError: (e: unknown) => void }

export function ViewingBar({ name, version, onBack, onRestored, onError }: Props) {
  const [diffOpen, setDiffOpen] = useState(false)
  const [busy, setBusy] = useState(false)
  const doRestore = async () => {
    const msg = 'Restore this version? Your current CV will be kept in history.'
    // The dialog plugin replaces window.confirm with an async version, so use ask().
    if (!(MOCK ? window.confirm(msg) : await ask(msg, { title: 'Restore version', kind: 'warning' }))) return
    setBusy(true)
    try {
      await restore(name, version.commit)
      onRestored()
    } catch (e) {
      onError(e)
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className="viewing-bar" role="status">
      <span>Viewing {relativeTime(version.time_ms)} · {version.author}</span>
      <span className="spacer" />
      <button onClick={() => setDiffOpen(true)}>Show diff</button>
      <button onClick={doRestore} disabled={busy}>Restore</button>
      <button onClick={onBack}>Back to current</button>
      {diffOpen && <DiffView name={name} commit={version.commit} onClose={() => setDiffOpen(false)} />}
    </div>
  )
}
