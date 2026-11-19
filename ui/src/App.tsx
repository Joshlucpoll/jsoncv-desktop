import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { ask, open } from '@tauri-apps/plugin-dialog'
import { DEFAULT_THEME, MOCK, type ApiError, type CvInfo, type ExportFormat, type LibraryConfig, type MenuCommand, type ThemeInfo, type Version, binaryPath, errorMessage, exitApp, exportCv, getConfig, history, listCvs, listThemes, onCloseRequested, onLibraryChanged, onMenuCommand, onQuitRequested, printCv, restore, reveal, setCvTheme, setLibrary, setPageMode } from './api.ts'
import { MOD, revealLabel } from './lib/platform.ts'
import { Settings, type SettingsSection } from './components/Settings.tsx'
import { Sidebar } from './components/Sidebar.tsx'
import { Toolbar } from './components/Toolbar.tsx'
import { EditPanel, type EditTab } from './components/EditPanel.tsx'
import { Preview } from './components/Preview.tsx'
import { DiffView } from './components/DiffView.tsx'
import { Toasts } from './components/Toasts.tsx'
import { useToasts } from './lib/toasts.ts'
import { type Cursor, step } from './lib/undo.ts'

/** Waits for the core's sweep (2 s after an outside edit) to commit it, so that commit can be announced too. */
const SWEEP_WAIT_MS = 2500

export default function App() {
  const [cvs, setCvs] = useState<CvInfo[]>([])
  const [themes, setThemes] = useState<ThemeInfo[]>([])
  const [selected, setSelected] = useState('main')
  const [reloadKey, setReloadKey] = useState(0)
  const [editing, setEditing] = useState(false)
  const leaveRef = useRef<(() => Promise<boolean>) | null>(null)
  const saveRef = useRef<(() => Promise<boolean>) | null>(null)
  // Run `go` only if the edit panel (when open) agrees to leave: it saves or asks before dropping a draft.
  const guarded = useCallback(async (go: () => void | Promise<void>) => {
    if (!leaveRef.current || (await leaveRef.current())) await go()
  }, [])
  const [config, setConfig] = useState<LibraryConfig>()
  const [settingsOpen, setSettingsOpen] = useState<SettingsSection>()
  const [editTab, setEditTab] = useState<EditTab>('form')
  const [tabRequest, setTabRequest] = useState<{ tab: EditTab; n: number }>()
  // The field being worked on: picked in the preview, or focused in the form. The preview marks it.
  const [focus, setFocus] = useState<{ path: string; from: 'preview' | 'form'; n: number }>()
  const focusN = useRef(0)
  const [binary, setBinary] = useState('')
  const [fallbackDismissed, setFallbackDismissed] = useState(false)
  const [exporting, setExporting] = useState(false)
  const [diffFor, setDiffFor] = useState<{ name: string; commit: string; title: string }>()
  const [newSignal, setNewSignal] = useState(0)
  const toasts = useToasts()
  const { push, dismiss } = toasts

  const fail = useCallback((e: unknown) => void push({ key: 'error', kind: 'error', text: errorMessage(e) }), [push])
  const reloadCvs = useCallback(() => listCvs().then(setCvs, fail), [fail])
  const reloadThemes = useCallback(() => listThemes().then(setThemes, fail), [fail])

  const selectedRef = useRef('main')
  // Newest commit seen per CV; a newer one by someone other than "You" gets a toast.
  const seen = useRef(new Map<string, string>())

  const announce = useCallback(async (cv: string) => {
    const v = await history(cv, 2).catch(() => [] as Version[])
    const top = v[0]
    if (!top) return
    const last = seen.current.get(cv)
    seen.current.set(cv, top.commit)
    if (last === undefined || last === top.commit || top.author === 'You' || cv !== selectedRef.current) return
    const title = `${top.author}: ${top.message}`
    const prev = v[1]
    push({
      key: 'change',
      text: title,
      timeout: 15_000,
      actions: [
        { label: 'Show changes', run: () => setDiffFor({ name: cv, commit: top.commit, title }) },
        ...(prev ? [{
          label: 'Undo',
          run: () => void guarded(async () => {
            // Undo means "back to before this change"; anything newer would be lost, so stop.
            if ((await history(cv, 1))[0]?.commit !== top.commit) throw new Error(`The CV changed again, so Undo was cancelled. Use ${MOD}Z to step back.`)
            await restore(cv, prev.commit)
            setReloadKey((k) => k + 1)
          }).catch(fail),
        }] : []),
      ],
    })
  }, [push, guarded, fail])

  useEffect(() => {
    reloadCvs()
    reloadThemes()
    getConfig().then(setConfig, fail)
    binaryPath().then(setBinary, () => {})
    const un = onLibraryChanged((paths) => {
      reloadCvs()
      const themesChanged = paths.some((p) => p.startsWith('themes/'))
      if (themesChanged) reloadThemes()
      const cv = selectedRef.current
      if (paths.includes(`${cv}.cv.json`)) {
        void announce(cv)
        setTimeout(() => void announce(cv), SWEEP_WAIT_MS)
      }
      if (themesChanged || paths.includes(`${cv}.cv.json`)) setReloadKey((k) => k + 1)
    })
    return () => void un.then((f) => f())
  }, [reloadCvs, reloadThemes, fail, announce])

  // Closing the window or quitting runs the same guard as switching CV (no-op when nothing is open or unsaved).
  useEffect(() => {
    const unClose = onCloseRequested(async (e) => {
      if (leaveRef.current && !(await leaveRef.current())) e.preventDefault()
    })
    const unQuit = onQuitRequested(() => void guarded(() => void exitApp()))
    return () => { void unClose.then((f) => f()); void unQuit.then((f) => f()) }
  }, [guarded])
  // Renaming or deleting the open CV: let the edit panel save (or ask) first, then close it.
  const closeEditor = useCallback(() => { setEditing(false); setEditTab('form'); setFocus(undefined) }, [])
  const leaveSelected = useCallback(async () => {
    if (leaveRef.current && !(await leaveRef.current())) return false
    // The panel agreed to leave; drop its guard now so the follow-up select does not ask again before unmount.
    leaveRef.current = saveRef.current = null
    closeEditor()
    return true
  }, [closeEditor])

  // A CV removed outside the app falls back to main.
  const current = cvs.length && !cvs.some((c) => c.name === selected) ? 'main' : selected
  const cv = cvs.find((c) => c.name === current)
  // The watcher callback must see the latest selection without re-subscribing.
  useEffect(() => {
    selectedRef.current = current
    // Remember where this CV's history stands, so only later changes are announced.
    history(current, 1).then((v) => { if (v[0] && !seen.current.has(current)) seen.current.set(current, v[0].commit) }, () => {})
  }, [current])

  // ⌘Z / ⇧⌘Z step through saved versions of the open CV. Restoring adds a version, so nothing is lost.
  const cursors = useRef(new Map<string, Cursor>())
  const undoRedo = useCallback(async (dir: 'undo' | 'redo') => {
    const name = current
    try {
      // Save a pending edit first, so undo starts from what is on screen.
      if (saveRef.current && !(await saveRef.current())) return
      // shortcut: looks back 500 versions; enough for a session of undos.
      const h = await history(name, 500)
      const s = step(h.map((v) => v.commit), cursors.current.get(name), dir)
      if (!s) return void push({ key: 'undo', text: dir === 'undo' ? 'Nothing to undo' : 'Nothing to redo', timeout: 2000 })
      const saved = await restore(name, s.target)
      cursors.current.set(name, { at: s.at, redo: s.redo, last: saved.commit || h[0].commit })
      // The undone change is the version just above the target (for undo) or the target itself (for redo).
      const i = h.findIndex((v) => v.commit === s.target)
      const what = dir === 'undo' ? h[i - 1] : h[i]
      push({ key: 'undo', text: `${dir === 'undo' ? 'Undid' : 'Redid'}: ${what?.message ?? 'change'}`, timeout: 4000 })
      setReloadKey((k) => k + 1)
    } catch (e) {
      fail(e)
    }
  }, [current, push, fail])

  const doPrint = useCallback(() => void guarded(() => printCv(current).catch(fail)), [guarded, current, fail])
  const doExport = useCallback(async (f: ExportFormat) => {
    dismiss('export')
    // Flush a pending draft first so the export matches the screen; keep the panel open.
    if (saveRef.current && !(await saveRef.current())) {
      const msg = 'Your latest edits could not be saved. Export the last saved version instead?'
      if (!(MOCK ? window.confirm(msg) : await ask(msg, { title: 'Export', kind: 'warning' }))) return
    }
    setExporting(true)
    try {
      const path = await exportCv(current, f)
      push({ key: 'export', seal: true, text: `Exported to ${path}`, timeout: 10_000, actions: [{ label: revealLabel, run: () => void reveal(path).catch(fail) }] })
    } catch (e) {
      if ((e as ApiError)?.kind === 'Browser') push({ key: 'export', kind: 'warn', text: 'PDF export needs Chrome, Edge or Chromium. None was found.', actions: [{ label: 'Print…', run: doPrint }] })
      else fail(e)
    } finally {
      setExporting(false)
    }
  }, [current, dismiss, push, fail, doPrint])

  // Shortcuts arrive as key presses and, on macOS, as menu commands; whichever comes second within
  // 300 ms is the same key press, so it is dropped.
  const lastCmd = useRef({ cmd: '', at: 0 })
  const runCmd = useCallback((cmd: MenuCommand) => {
    const now = Date.now()
    if (lastCmd.current.cmd === cmd && now - lastCmd.current.at < 300) return
    lastCmd.current = { cmd, at: now }
    if (cmd === 'edit') void guarded(() => (editing ? closeEditor() : setEditing(true)))
    else if (cmd === 'export-pdf') { if (!exporting) void doExport('pdf') }
    else if (cmd === 'print') doPrint()
    else if (cmd === 'new-cv') setNewSignal((n) => n + 1)
    else setSettingsOpen('library')
  }, [guarded, editing, closeEditor, exporting, doExport, doPrint])
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.altKey) return
      const k = e.key.toLowerCase()
      // In a text field ⌘Z stays the field's own text undo.
      const t = e.target as HTMLElement | null
      if (k === 'z' && !t?.closest('input, textarea, select, [contenteditable="true"], .cm-editor')) {
        e.preventDefault()
        void undoRedo(e.shiftKey ? 'redo' : 'undo')
        return
      }
      const cmd: MenuCommand | undefined =
        k === 'e' ? (e.shiftKey ? 'export-pdf' : 'edit') : e.shiftKey ? undefined : k === 'p' ? 'print' : k === 'n' ? 'new-cv' : k === ',' ? 'settings' : undefined
      if (!cmd) return
      e.preventDefault()
      runCmd(cmd)
    }
    window.addEventListener('keydown', onKey)
    const un = onMenuCommand(runCmd)
    return () => { window.removeEventListener('keydown', onKey); void un.then((f) => f()) }
  }, [runCmd, undoRedo])

  const changeLibrary = () =>
    void guarded(async () => {
      try {
        const p = await open({ directory: true })
        if (typeof p !== 'string') return
        setConfig(await setLibrary(p))
        closeEditor(); setSelected('main'); dismiss('error')
        seen.current.clear(); cursors.current.clear()
        void reloadCvs(); void reloadThemes(); setReloadKey((k) => k + 1)
      } catch (e) {
        fail(e)
      }
    })
  const cvTheme = cv?.theme ?? DEFAULT_THEME
  const pageSize = themes.find((t) => t.id === cvTheme)?.meta.page.size ?? 'A4'
  const continuous = !!cv?.continuous

  const requestTab = (tab: EditTab) => setTabRequest({ tab, n: Date.now() })
  const toggleJson = () => {
    const tab = editing && editTab === 'json' ? 'form' : 'json'
    if (!editing) setEditTab(tab) // opens straight into it
    else requestTab(tab)
    setEditing(true)
  }
  const onPick = (path: string | undefined) => {
    if (!path) return setFocus(undefined)
    if (editing && editTab === 'json') requestTab('form')
    setEditing(true)
    setFocus({ path, from: 'preview', n: ++focusN.current })
  }
  // The JSON editor has no fields to point at, so the preview mark goes.
  const onTab = useCallback((t: EditTab) => { setEditTab(t); if (t === 'json') setFocus(undefined) }, [])
  const onFocusPath = useCallback((path: string) => {
    setFocus((f) => (f?.path === path ? f : { path, from: 'form', n: ++focusN.current }))
  }, [])
  const reveal_ = useMemo(() => (focus?.from === 'preview' ? { path: focus.path, n: focus.n } : undefined), [focus])
  const highlight = useMemo(() => (focus ? { path: focus.path, scroll: focus.from === 'form', n: focus.n } : undefined), [focus])

  return (
    <div className="app">
      <Sidebar cvs={cvs} selected={current} onSelect={(n) => (n === current ? undefined : void guarded(() => { dismiss('export'); dismiss('change'); dismiss('undo'); setFocus(undefined); setSelected(n) }))} onChanged={reloadCvs} onLeaveSelected={leaveSelected} onError={fail} onSettings={() => setSettingsOpen('library')} newSignal={newSignal} />
      <main className="main">
        <Toolbar
          name={current}
          theme={cv?.theme ?? null}
          themes={themes}
          editing={editing}
          exporting={exporting}
          onEdit={() => void guarded(() => (editing ? closeEditor() : setEditing(true)))}
          onExport={(f) => void doExport(f)}
          onPrint={doPrint}
          onTheme={(id) => setCvTheme(current, id).then(reloadCvs, fail)}
          onManageThemes={() => setSettingsOpen('themes')}
          json={editing && editTab === 'json'}
          onJson={toggleJson}
          continuous={continuous}
          onPageMode={(c) => setPageMode(current, c).then(reloadCvs, fail)}
        />
        {config?.fallbackFrom && !fallbackDismissed && (
          <div className="banner" role="alert">
            <span>Couldn't open {config.fallbackFrom.path}: {config.fallbackFrom.message}. Using {config.library} instead.</span>
            <button className="link" onClick={changeLibrary}>Choose folder…</button>
            <button className="link" onClick={() => setFallbackDismissed(true)}>Dismiss</button>
          </div>
        )}
        <div className="workspace">
          <Preview cv={current} reloadKey={reloadKey} size={pageSize} continuous={continuous} onPick={onPick} highlight={editing ? highlight : undefined} />
          {editing && (
            <EditPanel
              key={current} name={current} leaveRef={leaveRef} saveRef={saveRef} onClose={() => void guarded(closeEditor)}
              initialTab={editTab} tabRequest={tabRequest} onTab={onTab} reveal={reveal_} onFocusPath={onFocusPath}
            />
          )}
          <Toasts list={toasts.list} dismiss={dismiss} />
        </div>
      </main>
      {settingsOpen && (
        <Settings
          config={config} binary={binary} themes={themes} reloadKey={reloadKey} section={settingsOpen}
          onChange={changeLibrary} onThemesChanged={() => void reloadThemes()}
          onError={fail} onClose={() => setSettingsOpen(undefined)}
        />
      )}
      {diffFor && <DiffView name={diffFor.name} commit={diffFor.commit} mode="change" toggle title={diffFor.title} onClose={() => setDiffFor(undefined)} />}
    </div>
  )
}
