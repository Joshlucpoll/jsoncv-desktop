import { useCallback, useEffect, useRef, useState } from 'react'
import { ask, open } from '@tauri-apps/plugin-dialog'
import { MOCK, type ApiError, type CvInfo, type ExportFormat, type LibraryConfig, type ThemeInfo, type Version, binaryPath, errorMessage, exitApp, exportCv, getConfig, listCvs, listThemes, onCloseRequested, onLibraryChanged, onQuitRequested, printCv, reveal, setCvTheme, setLibrary } from './api.ts'
import { revealLabel } from './lib/platform.ts'
import { Settings } from './components/Settings.tsx'
import { ThemeDetail } from './components/ThemesView.tsx'
import { type View, Sidebar } from './components/Sidebar.tsx'
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
  const saveRef = useRef<(() => Promise<boolean>) | null>(null)
  // Run `go` only if the edit panel (when open) agrees to leave: it saves or asks before dropping a draft.
  const guarded = useCallback(async (go: () => void) => {
    if (!leaveRef.current || (await leaveRef.current())) go()
  }, [])
  const [view, setView] = useState<View>('cvs')
  const [themeSel, setThemeSel] = useState('builtin:classic')
  const [config, setConfig] = useState<LibraryConfig>()
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [binary, setBinary] = useState('')
  const [fallbackDismissed, setFallbackDismissed] = useState(false)
  const [exported, setExported] = useState<string>() // path of the last export (transient toast)
  const [needsBrowser, setNeedsBrowser] = useState(false)

  const fail = useCallback((e: unknown) => setError(errorMessage(e)), [])
  const reloadCvs = useCallback(() => listCvs().then(setCvs, fail), [fail])
  const reloadThemes = useCallback(() => listThemes().then(setThemes, fail), [fail])

  const selectedRef = useRef('main')

  useEffect(() => {
    reloadCvs()
    reloadThemes()
    getConfig().then(setConfig, fail)
    binaryPath().then(setBinary, () => {})
    const un = onLibraryChanged((paths) => {
      reloadCvs()
      const themesChanged = paths.some((p) => p.startsWith('themes/'))
      if (themesChanged) reloadThemes()
      if (themesChanged || paths.includes(`${selectedRef.current}.cv.json`)) setReloadKey((k) => k + 1)
    })
    return () => void un.then((f) => f())
  }, [reloadCvs, reloadThemes, fail])

  // Closing the window or quitting runs the same guard as switching CV (no-op when nothing is open or unsaved).
  useEffect(() => {
    const unClose = onCloseRequested(async (e) => {
      if (leaveRef.current && !(await leaveRef.current())) e.preventDefault()
    })
    const unQuit = onQuitRequested(() => void guarded(() => void exitApp()))
    return () => { void unClose.then((f) => f()); void unQuit.then((f) => f()) }
  }, [guarded])
  // Renaming or deleting the open CV: let the edit panel save (or ask) first, then close it.
  const leaveSelected = useCallback(async () => {
    if (leaveRef.current && !(await leaveRef.current())) return false
    // The panel agreed to leave; drop its guard now so the follow-up select does not ask again before unmount.
    leaveRef.current = saveRef.current = null
    setEditing(false)
    return true
  }, [])

  useEffect(() => {
    if (!exported) return
    const t = setTimeout(() => setExported(undefined), 10_000)
    return () => clearTimeout(t)
  }, [exported])

  // A CV removed outside the app falls back to main.
  const current = cvs.length && !cvs.some((c) => c.name === selected) ? 'main' : selected
  const cv = cvs.find((c) => c.name === current)
  const viewing = viewingState?.cv === current ? viewingState.version : undefined // never apply CV A's commit to CV B
  // The watcher callback must see the latest selection without re-subscribing.
  useEffect(() => {
    selectedRef.current = current
  }, [current])

  // Viewing a past version closes the edit panel first (through the guard).
  const viewVersion = useCallback((v?: Version) => guarded(() => { setEditing(false); setViewing(v && { cv: current, version: v }) }), [guarded, current])
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

  const doExport = async (f: ExportFormat) => {
    setExported(undefined)
    setNeedsBrowser(false)
    // Flush a pending draft first so the export matches the screen; keep the panel open.
    if (saveRef.current && !(await saveRef.current())) {
      const msg = 'Your latest edits could not be saved. Export the last saved version instead?'
      if (!(MOCK ? window.confirm(msg) : await ask(msg, { title: 'Export', kind: 'warning' }))) return
    }
    try {
      setExported(await exportCv(current, f))
    } catch (e) {
      if ((e as ApiError)?.kind === 'Browser') setNeedsBrowser(true)
      else fail(e)
    }
  }
  const doPrint = () => void guarded(() => printCv(current).catch(fail))
  const changeLibrary = () =>
    void guarded(async () => {
      try {
        const p = await open({ directory: true })
        if (typeof p !== 'string') return
        setConfig(await setLibrary(p))
        setEditing(false); setViewing(undefined); setSelected('main'); setView('cvs'); setError(undefined)
        void reloadCvs(); void reloadThemes(); setReloadKey((k) => k + 1)
      } catch (e) {
        fail(e)
      }
    })
  const themeInfo = themes.find((t) => t.id === themeSel) ?? themes[0]
  const themesView = view === 'themes' && !!themeInfo

  return (
    <div className="app">
      <Sidebar cvs={cvs} selected={current} onSelect={(n) => (n === current ? undefined : void guarded(() => { setViewing(undefined); setExported(undefined); setNeedsBrowser(false); setSelected(n) }))} onChanged={reloadCvs} onLeaveSelected={leaveSelected} onError={fail} reloadKey={reloadKey} viewing={viewing?.commit} onView={(v) => void viewVersion(v)} view={view} onViewMode={setView} themes={themes} theme={themeInfo?.id} onTheme={setThemeSel} onSettings={() => setSettingsOpen(true)} />
      <main className="main">
        <Toolbar
          name={current}
          theme={cv?.theme ?? null}
          themes={themes}
          editing={editing}
          viewing={!!viewing}
          onEdit={() => void guarded(() => setEditing((v) => !v))}
          onExport={(f) => void doExport(f)}
          onPrint={doPrint}
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
        {config?.fallbackFrom && !fallbackDismissed && (
          <div className="banner" role="alert">
            <span>Couldn't open {config.fallbackFrom.path}: {config.fallbackFrom.message}. Using {config.library} instead.</span>
            <button className="link" onClick={changeLibrary}>Choose folder…</button>
            <button className="link" onClick={() => setFallbackDismissed(true)}>Dismiss</button>
          </div>
        )}
        {exported && (
          <div className="banner ok" role="status">
            <span>Exported to {exported}</span>
            <button className="link" onClick={() => reveal(exported).catch(fail)}>{revealLabel}</button>
            <button className="link" onClick={() => setExported(undefined)}>Dismiss</button>
          </div>
        )}
        {needsBrowser && (
          <div className="banner" role="alert">
            <span>No Chrome, Edge or Chromium found — use Print… instead</span>
            <button className="link" onClick={() => { setNeedsBrowser(false); doPrint() }}>Print…</button>
            <button className="link" onClick={() => setNeedsBrowser(false)}>Dismiss</button>
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
        {themesView && (
          <ThemeDetail
            key={themeInfo.id}
            theme={themeInfo} reloadKey={reloadKey} cvName={current}
            useDisabled={!!viewing} onUse={() => setCvTheme(current, themeInfo.id).then(reloadCvs, fail)}
            onDuplicated={(t) => { void reloadThemes(); setThemeSel(t.id) }}
            onError={fail}
          />
        )}
        <div className="workspace">
          <Preview cv={current} commit={viewing?.commit} theme={themesView ? themeInfo.id : undefined} reloadKey={reloadKey} />
          {editing && <EditPanel key={current} name={current} leaveRef={leaveRef} saveRef={saveRef} onClose={() => void guarded(() => setEditing(false))} />}
        </div>
      </main>
      {settingsOpen && <Settings config={config} binary={binary} onChange={changeLibrary} onClose={() => setSettingsOpen(false)} />}
    </div>
  )
}
