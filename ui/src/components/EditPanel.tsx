import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import validator from '@rjsf/validator-ajv8'
import type { RJSFSchema } from '@rjsf/utils'
import CodeMirror from '@uiw/react-codemirror'
import { json } from '@codemirror/lang-json'
import { ask } from '@tauri-apps/plugin-dialog'
import {
  MOCK, type ApiError, type SchemaError, errorMessage, getSchema, history, onLibraryChanged, readCv, saveDraft,
} from '../api.ts'
import { shrank } from '../lib/form.ts'
import { ConflictBanner } from './ConflictBanner.tsx'
import { CvForm } from './CvForm.tsx'
import { DiffView } from './DiffView.tsx'

const SAVE_DELAY_MS = 1000

// Object keys holding undefined are absent in JSON, so they do not count (array slots do).
const keys = (o: object) => Object.keys(o).filter((k) => Array.isArray(o) || (o as Record<string, unknown>)[k] !== undefined)

// The dialog plugin replaces window.confirm with an async version; the mock browser has no plugin.
const confirmLeave = (msg: string) => (MOCK ? Promise.resolve(window.confirm(msg)) : ask(msg, { title: 'Unsaved changes', kind: 'warning' }))

function same(a: unknown, b: unknown): boolean {
  if (a === b) return true
  if (typeof a !== 'object' || typeof b !== 'object' || !a || !b || Array.isArray(a) !== Array.isArray(b)) return false
  const ka = keys(a)
  return ka.length === keys(b).length && ka.every((k) => same((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k]))
}

const WIDTH_KEY = 'jsoncv.editWidth'
const readWidth = () => {
  try {
    return Number(localStorage.getItem(WIDTH_KEY)) || 380
  } catch {
    return 380
  }
}
const clampWidth = (w: number) => Math.round(Math.min(Math.max(w, 320), window.innerWidth * 0.6))

const darkQuery = matchMedia('(prefers-color-scheme: dark)')
const useDark = () => useSyncExternalStore((cb) => { darkQuery.addEventListener('change', cb); return () => darkQuery.removeEventListener('change', cb) }, () => darkQuery.matches)

type Props = {
  name: string
  onClose(): void
  /** The panel registers a check App runs before closing it or switching CV. */
  leaveRef: { current: (() => Promise<boolean>) | null }
  /** Saves any pending draft without leaving; resolves true when nothing unsaved remains. */
  saveRef: { current: (() => Promise<boolean>) | null }
}

export function EditPanel({ name, onClose, leaveRef, saveRef }: Props) {
  const [schema, setSchema] = useState<RJSFSchema>()
  const [base, setBase] = useState<unknown>()
  const [draft, setDraft] = useState<unknown>()
  const [tab, setTab] = useState<'form' | 'json'>('form')
  const [jsonText, setJsonText] = useState('')
  const [jsonError, setJsonError] = useState<string>()
  const [conflict, setConflict] = useState(false)
  const [busy, setBusy] = useState(false)
  const [saveError, setSaveError] = useState<string>()
  const [serverErrors, setServerErrors] = useState<SchemaError[]>()
  const [loadCommit, setLoadCommit] = useState<string>()
  const [diffOpen, setDiffOpen] = useState(false)
  const [pending, setPending] = useState(false)
  const [undo, setUndo] = useState<{ prev: unknown }>() // offered after a list item is removed
  const [width, setWidth] = useState(() => clampWidth(readWidth()))
  const dark = useDark()
  const fresh = useRef(false) // false while the form's first render runs, so existing cards start closed

  const baseRef = useRef<unknown>(undefined)
  const draftRef = useRef<unknown>(undefined)
  const schemaRef = useRef<RJSFSchema>(undefined)
  const conflictRef = useRef(false)
  const timer = useRef<number>(undefined)
  const saving = useRef(false)
  const lastBase = useRef('') // content hash of what we last read or saved
  const scheduleRef = useRef<() => void>(() => {})
  const jsonErrRef = useRef<string>(undefined) // set while the JSON tab holds text that does not parse
  const diskChanged = useRef(false) // the file changed while we held an unsaved draft
  const inflight = useRef<Promise<void>>(undefined)
  const lastError = useRef<string>(undefined)

  const unsavedDraft = () => !same(draftRef.current, baseRef.current)
  // Unparsed JSON text is unsaved work too.
  const dirty = () => unsavedDraft() || !!jsonErrRef.current
  const busy_ = () => dirty() || !!timer.current || saving.current || conflictRef.current
  const isValid = (d: unknown) =>
    !!schemaRef.current && !validator.rawValidation(schemaRef.current, d).errors?.length

  const applyDisk = useCallback((cv: unknown, hash: string, commit: string | undefined) => {
    baseRef.current = draftRef.current = cv
    lastBase.current = hash
    setBase(cv)
    setDraft(cv)
    setJsonText(JSON.stringify(cv, null, 2))
    jsonErrRef.current = undefined
    setJsonError(undefined)
    setLoadCommit(commit)
  }, [])

  const load = useCallback(async () => {
    const [doc, h] = await Promise.all([readCv(name), history(name, 1).catch(() => [])])
    applyDisk(doc.cv, doc.base, h[0]?.commit)
  }, [name, applyDisk])

  const flush = useCallback((): Promise<void> => {
    window.clearTimeout(timer.current)
    timer.current = undefined
    if (saving.current) return inflight.current ?? Promise.resolve()
    if (conflictRef.current || !unsavedDraft() || !isValid(draftRef.current)) {
      setPending(false)
      return Promise.resolve()
    }
    saving.current = true
    const sent = draftRef.current
    const run = async () => {
      try {
        const res = await saveDraft(name, baseRef.current, sent)
        baseRef.current = sent
        lastBase.current = res.base
        setBase(sent)
        if (res.commit) setLoadCommit(res.commit)
        lastError.current = undefined
        setSaveError(undefined)
        setServerErrors(undefined)
        // The save merged other edits into the file; show the merged result and rebase on it.
        if (diskChanged.current && draftRef.current === sent && !jsonErrRef.current) {
          diskChanged.current = false
          const [doc, h] = await Promise.all([readCv(name), history(name, 1).catch(() => [])])
          // Saved.base hashes the merged bytes, so compare content, not hashes.
          // A keystroke during the read keeps the refresh owed for the next save.
          if (draftRef.current !== sent || jsonErrRef.current) diskChanged.current = true
          else if (!same(doc.cv, sent)) applyDisk(doc.cv, doc.base, h[0]?.commit)
        }
      } catch (e) {
        const err = e as ApiError
        if (err.kind === 'PatchFailed') {
          conflictRef.current = true
          setConflict(true)
          lastError.current = 'This CV changed while you were editing'
        } else if (err.kind === 'Invalid') {
          setServerErrors(err.errors ?? [{ path: '', message: err.message }])
          lastError.current = err.message
        } else {
          lastError.current = errorMessage(e)
          setSaveError(lastError.current) // draft stays; the next change retries
        }
      } finally {
        saving.current = false
        if (unsavedDraft() && !conflictRef.current && !timer.current && draftRef.current !== sent) scheduleRef.current()
        else setPending(false)
      }
    }
    inflight.current = run()
    return inflight.current
  }, [name, applyDisk])

  const schedule = useCallback(() => {
    window.clearTimeout(timer.current)
    timer.current = undefined
    if (conflictRef.current || !dirty() || !isValid(draftRef.current)) return setPending(false)
    setPending(true)
    timer.current = window.setTimeout(flush, SAVE_DELAY_MS)
  }, [flush])
  useEffect(() => {
    scheduleRef.current = schedule
  }, [schedule])

  const onDraft = useCallback((next: unknown) => {
    draftRef.current = next
    setDraft(next)
    setSaveError(undefined)
    setServerErrors(undefined)
    schedule()
  }, [schedule])

  useEffect(() => {
    getSchema().then((s) => {
      const { $schema: _, ...rest } = s // draft-07 URI without '#' is unknown to ajv
      schemaRef.current = rest as RJSFSchema
      setSchema(rest as RJSFSchema)
    }, (e) => setSaveError(errorMessage(e)))
    // oxlint-disable-next-line react/set-state-in-effect -- load() sets state only after the async read
    load().catch((e) => setSaveError(errorMessage(e)))
  }, [load])

  useEffect(() => {
    const un = onLibraryChanged((paths) => {
      if (!paths.includes(`${name}.cv.json`)) return
      if (busy_()) {
        diskChanged.current = true
        return
      }
      Promise.all([readCv(name), history(name, 1).catch(() => [])]).then(([doc, h]) => {
        // Re-check after the awaits: the user may have typed meanwhile.
        if (doc.base === lastBase.current) return
        if (busy_()) diskChanged.current = true
        else applyDisk(doc.cv, doc.base, h[0]?.commit)
      }, () => {})
    })
    return () => void un.then((f) => f())
  }, [name, applyDisk])

  useEffect(() => () => window.clearTimeout(timer.current), [])

  // App calls this before closing the panel or switching CV. false cancels the leave.
  useEffect(() => {
    const save = async () => {
      for (let i = 0; i < 3 && unsavedDraft() && !conflictRef.current && !jsonErrRef.current && isValid(draftRef.current); i++) await flush()
      return !dirty()
    }
    saveRef.current = save
    leaveRef.current = async () => {
      if (await save()) return true
      if (conflictRef.current || jsonErrRef.current || !isValid(draftRef.current)) {
        return confirmLeave("You have unsaved changes that can't be saved yet. Discard them?")
      }
      return confirmLeave(`Couldn't save: ${lastError.current ?? 'unknown error'}. Discard your changes?`)
    }
    return () => { leaveRef.current = null; saveRef.current = null }
  }, [flush, leaveRef, saveRef])

  const discard = async () => {
    window.clearTimeout(timer.current)
    timer.current = undefined
    conflictRef.current = false
    setConflict(false)
    setServerErrors(undefined)
    setSaveError(undefined)
    diskChanged.current = false
    await load().catch((e) => setSaveError(errorMessage(e)))
    setPending(false)
  }

  const overwrite = async () => {
    setBusy(true)
    try {
      const fresh = await readCv(name)
      const sent = draftRef.current
      const res = await saveDraft(name, fresh.cv, sent)
      baseRef.current = sent
      lastBase.current = res.base
      setBase(sent)
      if (res.commit) setLoadCommit(res.commit)
      conflictRef.current = false
      setConflict(false)
      setSaveError(undefined)
      setPending(false)
      if (draftRef.current !== sent) scheduleRef.current()
    } catch (e) {
      const err = e as ApiError
      if (err.kind === 'Invalid') setServerErrors(err.errors ?? [{ path: '', message: err.message }])
      else if (err.kind === 'PatchFailed') setSaveError('Changed again — try Overwrite once more')
      else setSaveError(errorMessage(e))
    } finally {
      setBusy(false)
    }
  }

  const onJson = (text: string) => {
    setJsonText(text)
    try {
      const parsed = JSON.parse(text)
      jsonErrRef.current = undefined
      setJsonError(undefined)
      onDraft(parsed)
    } catch (e) {
      jsonErrRef.current = errorMessage(e)
      setJsonError(`Invalid JSON: ${errorMessage(e)}`)
    }
  }

  const switchTab = (t: 'form' | 'json') => {
    if (t === 'form' && jsonErrRef.current) {
      setJsonError(`Fix the JSON before switching to the form. ${jsonErrRef.current}`)
      return
    }
    if (t === 'json') {
      setJsonText(JSON.stringify(draftRef.current, null, 2))
      setJsonError(undefined)
    }
    setTab(t)
  }

  const onFormChange = useCallback((next: unknown) => {
    if (shrank(draftRef.current, next)) setUndo({ prev: draftRef.current })
    onDraft(next)
  }, [onDraft])
  useEffect(() => {
    if (!undo) return
    const t = setTimeout(() => setUndo(undefined), 8000)
    return () => clearTimeout(t)
  }, [undo])
  useEffect(() => {
    fresh.current = tab === 'form' && !!schema && draft !== undefined
  }, [tab, schema, draft])

  const errors = useMemo(
    () => (schema && draft !== undefined ? (validator.rawValidation(schema, draft).errors ?? []).map((e) => (e as { instancePath?: string }).instancePath ?? '') : []),
    [schema, draft],
  )
  const showFirstError = () => {
    const el = document.getElementById(`root_${errors[0]?.split('/')[1] ?? ''}`)
    if (el instanceof HTMLDetailsElement) el.open = true
    el?.scrollIntoView({ block: 'start', behavior: 'smooth' })
  }

  const unsaved = pending || !!jsonError || (base !== undefined && !same(draft, base))
  const nErr = errors.length
  const status = base === undefined ? undefined
    : jsonError ? { kind: 'error', text: 'Not saved: invalid JSON' }
    : conflict ? { kind: 'error', text: 'Not saved: conflict' }
    : nErr ? { kind: 'error', text: `Not saved: ${nErr} error${nErr === 1 ? '' : 's'}`, onClick: tab === 'form' ? showFirstError : undefined }
    : saveError || serverErrors ? { kind: 'error', text: "Couldn't save" }
    : unsaved ? { kind: 'busy', text: 'Saving…' }
    : { kind: 'ok', text: 'Saved' }

  const form = useMemo(
    () => (schema && draft !== undefined ? <CvForm schema={schema} draft={draft} errors={errors} fresh={fresh} onChange={onFormChange} /> : null),
    [schema, draft, errors, onFormChange],
  )

  const resize = (w: number) => {
    const next = clampWidth(w)
    setWidth(next)
    try { localStorage.setItem(WIDTH_KEY, String(next)) } catch { /* storage unavailable: width is not remembered */ }
  }
  const onHandleDown = (e: React.PointerEvent) => {
    e.preventDefault()
    const el = e.currentTarget as HTMLElement
    el.setPointerCapture(e.pointerId)
    const move = (ev: PointerEvent) => resize(window.innerWidth - ev.clientX)
    const up = () => { el.removeEventListener('pointermove', move); el.removeEventListener('pointerup', up) }
    el.addEventListener('pointermove', move)
    el.addEventListener('pointerup', up)
  }

  return (
    <aside className="edit" style={{ width }}>
      <div
        className="resize-handle" role="separator" aria-orientation="vertical" aria-label="Resize edit panel" tabIndex={0}
        aria-valuenow={width} aria-valuemin={320}
        onPointerDown={onHandleDown}
        onKeyDown={(e) => { if (e.key === 'ArrowLeft') resize(width + 16); else if (e.key === 'ArrowRight') resize(width - 16) }}
      />
      <header>
        <strong>Edit</strong>
        {status && (status.onClick
          ? <button className={`status ${status.kind}`} onClick={status.onClick} title="Show the first error">{status.text}</button>
          : <span className={`status ${status.kind}`} role="status">{status.text}</span>)}
        <div className="tabs" role="tablist">
          <button role="tab" aria-selected={tab === 'form'} onClick={() => switchTab('form')}>Form</button>
          <button role="tab" aria-selected={tab === 'json'} onClick={() => switchTab('json')}>JSON</button>
        </div>
        <button className="link" onClick={onClose} aria-label="Close edit panel">Close</button>
      </header>
      {conflict && (
        <ConflictBanner canShow={!!loadCommit} busy={busy} onShow={() => setDiffOpen(true)} onDiscard={discard} onOverwrite={overwrite} />
      )}
      {saveError && <div className="banner" role="alert"><span>{saveError}</span></div>}
      {serverErrors && (
        <div className="banner" role="alert">
          <span>{serverErrors.map((e) => `${e.path || '/'}: ${e.message}`).join('\n')}</span>
        </div>
      )}
      {undo && (
        <div className="banner ok" role="status">
          <span>Item removed</span>
          <button className="link" onClick={() => { onDraft(undo.prev); setUndo(undefined) }}>Undo</button>
        </div>
      )}
      <div className="edit-body">
        {tab === 'form' ? form : (
          <>
            <CodeMirror value={jsonText} extensions={[json()]} theme={dark ? 'dark' : 'light'} onChange={onJson} basicSetup={{ foldGutter: false }} />
            {jsonError && <div className="field-error">{jsonError}</div>}
          </>
        )}
      </div>
      {diffOpen && loadCommit && <DiffView name={name} commit={loadCommit} onClose={() => setDiffOpen(false)} />}
    </aside>
  )
}
