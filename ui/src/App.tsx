import { useCallback, useEffect, useRef, useState } from 'react'
import { type CvInfo, type ThemeInfo, errorMessage, listCvs, listThemes, onLibraryChanged, setCvTheme } from './api.ts'
import { Sidebar } from './components/Sidebar.tsx'
import { Toolbar } from './components/Toolbar.tsx'
import { Preview } from './components/Preview.tsx'

export default function App() {
  const [cvs, setCvs] = useState<CvInfo[]>([])
  const [themes, setThemes] = useState<ThemeInfo[]>([])
  const [selected, setSelected] = useState('main')
  const [commit] = useState<string>() // history commit preview: Task 14
  const [reloadKey, setReloadKey] = useState(0)
  const [error, setError] = useState<string>()
  // Edit-panel flag (Task 13) and cv/themes view (Task 15) are added with their features.

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

  return (
    <div className="app">
      <Sidebar cvs={cvs} selected={current} onSelect={setSelected} onChanged={reloadCvs} onError={fail} />
      <main className="main">
        <Toolbar
          name={current}
          theme={cv?.theme ?? null}
          themes={themes}
          onTheme={(id) =>
            setCvTheme(current, id).then(() => {
              reloadCvs()
              setReloadKey((k) => k + 1)
            }, fail)
          }
        />
        {error && (
          <div className="banner" role="alert">
            <span>{error}</span>
            <button className="link" onClick={() => setError(undefined)}>Dismiss</button>
          </div>
        )}
        <Preview cv={current} commit={commit} reloadKey={reloadKey} />
      </main>
    </div>
  )
}
