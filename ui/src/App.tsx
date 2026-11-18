import { useCallback, useEffect, useRef, useState } from 'react'
import { type CvInfo, type ThemeInfo, errorMessage, listCvs, listThemes, onLibraryChanged, setCvTheme } from './api.ts'
import { Sidebar } from './components/Sidebar.tsx'
import { Toolbar } from './components/Toolbar.tsx'
import { EditPanel } from './components/EditPanel.tsx'
import { Preview } from './components/Preview.tsx'

export default function App() {
  const [cvs, setCvs] = useState<CvInfo[]>([])
  const [themes, setThemes] = useState<ThemeInfo[]>([])
  const [selected, setSelected] = useState('main')
  const [commit] = useState<string>() // history commit preview: Task 14
  const [reloadKey, setReloadKey] = useState(0)
  const [error, setError] = useState<string>()
  const [editing, setEditing] = useState(false)
  const leaveRef = useRef<(() => Promise<boolean>) | null>(null)
  // Run `go` only if the edit panel (when open) agrees to leave: it saves or asks before dropping a draft.
  const guarded = useCallback(async (go: () => void) => {
    if (!leaveRef.current || (await leaveRef.current())) go()
  }, [])
  // cv/themes view (Task 15) is added with its feature.

  const fail = useCallback((e: unknown) => setError(errorMessage(e)), [])
  const reloadCvs = useCallback(() => listCvs().then(setCvs, fail), [fail])
  const reloadThemes = useCallback(() => listThemes().then(setThemes, fail), [fail])

  const selectedRef = useRef('main')

  useEffect(() => {
    reloadCvs()
    reloadThemes()
    const un = onLibraryChanged((paths) => {
      reloadCvs()
      const themesChanged = paths.some((p) => p.startsWith('themes/'))
      if (themesChanged) reloadThemes()
      if (themesChanged || paths.includes(`${selectedRef.current}.cv.json`)) setReloadKey((k) => k + 1)
    })
    return () => void un.then((f) => f())
  }, [reloadCvs, reloadThemes])

  // A CV removed outside the app falls back to main.
  const current = cvs.length && !cvs.some((c) => c.name === selected) ? 'main' : selected
  const cv = cvs.find((c) => c.name === current)
  // The watcher callback must see the latest selection without re-subscribing.
  useEffect(() => {
    selectedRef.current = current
  }, [current])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'e') {
        e.preventDefault()
        void guarded(() => setEditing((v) => !v))
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [guarded])

  return (
    <div className="app">
      <Sidebar cvs={cvs} selected={current} onSelect={(n) => (n === current ? undefined : void guarded(() => setSelected(n)))} onChanged={reloadCvs} onError={fail} />
      <main className="main">
        <Toolbar
          name={current}
          theme={cv?.theme ?? null}
          themes={themes}
          editing={editing}
          onEdit={() => void guarded(() => setEditing((v) => !v))}
          onTheme={(id) =>
            setCvTheme(current, id).then(reloadCvs, fail)
          }
        />
        {error && (
          <div className="banner" role="alert">
            <span>{error}</span>
            <button className="link" onClick={() => setError(undefined)}>Dismiss</button>
          </div>
        )}
        <div className="workspace">
          <Preview cv={current} commit={commit} reloadKey={reloadKey} />
          {editing && <EditPanel key={current} name={current} leaveRef={leaveRef} onError={fail} onClose={() => void guarded(() => setEditing(false))} />}
        </div>
      </main>
    </div>
  )
}
