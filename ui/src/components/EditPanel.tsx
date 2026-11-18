import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import Form from '@rjsf/core'
import validator from '@rjsf/validator-ajv8'
import type { IconButtonProps, ObjectFieldTemplateProps, RJSFSchema, UiSchema, WidgetProps } from '@rjsf/utils'
import CodeMirror from '@uiw/react-codemirror'
import { json } from '@codemirror/lang-json'
import { ask, open } from '@tauri-apps/plugin-dialog'
import {
  MOCK, type ApiError, type SchemaError, addAssetFromPath, errorMessage, getSchema, history, onLibraryChanged, readCv, saveDraft,
} from '../api.ts'
import { ConflictBanner } from './ConflictBanner.tsx'
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

function ObjectFieldTemplate({ fieldPathId, title, properties }: ObjectFieldTemplateProps) {
  const body = properties.filter((p) => !p.hidden).map((p) => <div key={p.name} className="prop">{p.content}</div>)
  const depth = fieldPathId.path.length
  if (depth === 0) return <>{body}</>
  if (depth === 1) {
    return (
      <details className="section" open={fieldPathId.path[0] === 'basics'}>
        <summary>{title}</summary>
        <div className="section-body">{body}</div>
      </details>
    )
  }
  return (
    <fieldset className="nested">
      <legend>{title}</legend>
      {body}
    </fieldset>
  )
}

function ImageWidget({ id, value, onChange, disabled, readonly }: WidgetProps) {
  const [err, setErr] = useState<string>()
  const choose = async () => {
    try {
      const path = await open({ filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg'] }] })
      if (typeof path === 'string') onChange(await addAssetFromPath(path))
      setErr(undefined)
    } catch (e) {
      setErr(errorMessage(e))
    }
  }
  return (
    <div className="image-widget">
      <div className="row">
        <input id={id} type="text" value={value ?? ''} disabled={disabled} readOnly={readonly} onChange={(e) => onChange(e.target.value || undefined)} />
        <button type="button" disabled={disabled || readonly} onClick={choose}>Choose file…</button>
      </div>
      {err && <div className="field-error">{err}</div>}
    </div>
  )
}

const uiSchema: UiSchema = {
  'ui:title': '',
  'ui:description': '',
  $schema: { 'ui:widget': 'hidden' },
  basics: { image: { 'ui:widget': 'imageFile' }, summary: { 'ui:widget': 'textarea' } },
}
// The default theme's icons are a glyph font we do not ship; use text.
const textButton = (label: string) =>
  function TextButton({ id, title, disabled, onClick }: IconButtonProps) {
    return <button type="button" id={id} title={title} aria-label={title} disabled={disabled} onClick={onClick}>{label}</button>
  }
const templates = {
  ObjectFieldTemplate,
  ButtonTemplates: {
    AddButton: textButton('+ Add'),
    RemoveButton: textButton('✕'),
    MoveUpButton: textButton('↑'),
    MoveDownButton: textButton('↓'),
  },
}
const widgets = { imageFile: ImageWidget }
const dark = matchMedia('(prefers-color-scheme: dark)').matches

type Props = {
  name: string
  onClose(): void
  onError(message: string): void
  /** The panel registers a check App runs before closing it or switching CV. */
  leaveRef: { current: (() => Promise<boolean>) | null }
  /** Saves any pending draft without leaving; resolves true when nothing unsaved remains. */
  saveRef: { current: (() => Promise<boolean>) | null }
}

export function EditPanel({ name, onClose, onError, leaveRef, saveRef }: Props) {
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
          if (draftRef.current === sent && !jsonErrRef.current && !same(doc.cv, sent)) applyDisk(doc.cv, doc.base, h[0]?.commit)
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
  }, [flush, leaveRef, saveRef, onError])

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

  const unsaved = pending || !!jsonError || (base !== undefined && !same(draft, base))
  const form = useMemo(
    () =>
      schema && draft !== undefined ? (
        <Form
          schema={schema}
          uiSchema={uiSchema}
          formData={draft}
          validator={validator}
          templates={templates}
          widgets={widgets}
          liveValidate
          noHtml5Validate
          showErrorList={false}
          tagName="div"
          onChange={(e) => onDraft(e.formData)}
        />
      ) : null,
    [schema, draft, onDraft],
  )

  return (
    <aside className="edit">
      <header>
        <strong>Edit{unsaved && <i className="dot" title="Unsaved changes" />}</strong>
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
