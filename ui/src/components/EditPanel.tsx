import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import Form from '@rjsf/core'
import validator from '@rjsf/validator-ajv8'
import type { IconButtonProps, ObjectFieldTemplateProps, RJSFSchema, UiSchema, WidgetProps } from '@rjsf/utils'
import CodeMirror from '@uiw/react-codemirror'
import { json } from '@codemirror/lang-json'
import { open } from '@tauri-apps/plugin-dialog'
import {
  type ApiError, type SchemaError, addAssetFromPath, errorMessage, getSchema, history, onLibraryChanged, readCv, saveDraft,
} from '../api.ts'
import { ConflictBanner } from './ConflictBanner.tsx'
import { DiffView } from './DiffView.tsx'

const SAVE_DELAY_MS = 1000

function same(a: unknown, b: unknown): boolean {
  if (a === b) return true
  if (typeof a !== 'object' || typeof b !== 'object' || !a || !b || Array.isArray(a) !== Array.isArray(b)) return false
  const ka = Object.keys(a)
  const kb = Object.keys(b)
  return ka.length === kb.length && ka.every((k) => same((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k]))
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

export function EditPanel({ name, onClose }: { name: string; onClose(): void }) {
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

  const dirty = () => !same(draftRef.current, baseRef.current)
  const isValid = (d: unknown) =>
    !!schemaRef.current && !validator.rawValidation(schemaRef.current, d).errors?.length

  const applyDisk = useCallback((cv: unknown, hash: string, commit: string | undefined) => {
    baseRef.current = draftRef.current = cv
    lastBase.current = hash
    setBase(cv)
    setDraft(cv)
    setJsonText(JSON.stringify(cv, null, 2))
    setJsonError(undefined)
    setLoadCommit(commit)
  }, [])

  const load = useCallback(async () => {
    const [doc, h] = await Promise.all([readCv(name), history(name, 1).catch(() => [])])
    applyDisk(doc.cv, doc.base, h[0]?.commit)
  }, [name, applyDisk])

  const flush = useCallback(async () => {
    window.clearTimeout(timer.current)
    timer.current = undefined
    if (saving.current || conflictRef.current || !dirty() || !isValid(draftRef.current)) return setPending(false)
    saving.current = true
    const sent = draftRef.current
    try {
      const res = await saveDraft(name, baseRef.current, sent)
      baseRef.current = sent
      lastBase.current = res.base
      setBase(sent)
      setLoadCommit(res.commit)
      setSaveError(undefined)
      setServerErrors(undefined)
    } catch (e) {
      const err = e as ApiError
      if (err.kind === 'PatchFailed') {
        conflictRef.current = true
        setConflict(true)
      } else if (err.kind === 'Invalid') {
        setServerErrors(err.errors ?? [{ path: '', message: err.message }])
      } else {
        setSaveError(errorMessage(e)) // draft stays; the next change retries
      }
    } finally {
      saving.current = false
      if (dirty() && !conflictRef.current && !timer.current && draftRef.current !== sent) scheduleRef.current()
      else setPending(false)
    }
  }, [name])

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
      if (!paths.includes(`${name}.cv.json`) || dirty() || timer.current || saving.current || conflictRef.current) return
      readCv(name).then(async (doc) => {
        if (doc.base === lastBase.current || dirty() || timer.current || saving.current) return
        applyDisk(doc.cv, doc.base, (await history(name, 1).catch(() => []))[0]?.commit)
      }, () => {})
    })
    return () => void un.then((f) => f())
  }, [name, applyDisk])

  // Closing or switching CV must not drop a pending edit.
  useEffect(() => () => {
    window.clearTimeout(timer.current)
    if (!conflictRef.current && !saving.current && dirty() && isValid(draftRef.current)) {
      saveDraft(name, baseRef.current, draftRef.current).catch(() => {})
    }
  }, [name])

  const discard = async () => {
    window.clearTimeout(timer.current)
    conflictRef.current = false
    setConflict(false)
    setServerErrors(undefined)
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
      setLoadCommit(res.commit)
      conflictRef.current = false
      setConflict(false)
      setPending(false)
    } catch (e) {
      const err = e as ApiError
      if (err.kind === 'Invalid') setServerErrors(err.errors ?? [{ path: '', message: err.message }])
      else if (err.kind !== 'PatchFailed') setSaveError(errorMessage(e))
    } finally {
      setBusy(false)
    }
  }

  const onJson = (text: string) => {
    setJsonText(text)
    try {
      const parsed = JSON.parse(text)
      setJsonError(undefined)
      onDraft(parsed)
    } catch (e) {
      setJsonError(errorMessage(e))
    }
  }

  const switchTab = (t: 'form' | 'json') => {
    if (t === 'json') {
      setJsonText(JSON.stringify(draftRef.current, null, 2))
      setJsonError(undefined)
    }
    setTab(t)
  }

  const unsaved = pending || (base !== undefined && !same(draft, base))
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
            {jsonError && <div className="field-error">Invalid JSON: {jsonError}</div>}
          </>
        )}
      </div>
      {diffOpen && loadCommit && <DiffView name={name} commit={loadCommit} onClose={() => setDiffOpen(false)} />}
    </aside>
  )
}
