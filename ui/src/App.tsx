import { useCallback, useEffect, useRef, useState } from 'react'
import { type CvInfo, type ThemeInfo, type Version, errorMessage, listCvs, listThemes, onLibraryChanged, setCvTheme } from './api.ts'
import { Sidebar } from './components/Sidebar.tsx'
import { Toolbar } from './components/Toolbar.tsx'
import { EditPanel } from './components/EditPanel.tsx'
import { Preview } from './components/Preview.tsx'
import { ViewingBar } from './components/ViewingBar.tsx'

export default function App() {
  const [cvs, setCvs] = useState<CvInfo[]>([])
  const [themes, setThemes] = useState<ThemeInfo[]>([])
  const [selected, setSelected] = useState('main')
  const [viewingState, setViewing] = useState<{ cv: string; version: Version }>() // past version shown in the preview; undefined = current
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
  const viewing = viewingState?.cv === current ? viewingState.version : undefined // never apply CV A's commit to CV B
  // The watcher callback must see the latest selection without re-subscribing.
  useEffect(() => {
    selectedRef.current = current
  }, [current])

  // Viewing a past version closes the edit panel first (through the guard).
  const view = useCallback((v?: Version) => guarded(() => { setEditing(false); setViewing(v && { cv: current, version: v }) }), [guarded, current])
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'e') {
        e.preventDefault()
        if (!viewing) void guarded(() => setEditing((v) => !v))
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [guarded, viewing])

  return (
    <div className="app">
      <Sidebar cvs={cvs} selected={current} onSelect={(n) => (n === current ? undefined : void guarded(() => { setViewing(undefined); setSelected(n) }))} onChanged={reloadCvs} onError={fail} reloadKey={reloadKey} viewing={viewing?.commit} onView={(v) => void view(v)} />
      <main className="main">
        <Toolbar
          name={current}
          theme={cv?.theme ?? null}
          themes={themes}
          editing={editing}
          viewing={!!viewing}
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
        {viewing && (
          <ViewingBar
            key={viewing.commit}
            name={current}
            version={viewing}
            onBack={() => setViewing(undefined)}
            onRestored={() => { setViewing(undefined); setReloadKey((k) => k + 1) }}
            onError={fail}
          />
        )}
        <div className="workspace">
          <Preview cv={current} commit={viewing?.commit} reloadKey={reloadKey} />
          {editing && <EditPanel key={current} name={current} leaveRef={leaveRef} onError={fail} onClose={() => void guarded(() => setEditing(false))} />}
        </div>
      </main>
    </div>
  )
}
