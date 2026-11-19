import { createContext, type ReactElement, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import Form from '@rjsf/core'
import validator from '@rjsf/validator-ajv8'
import { getUiOptions } from '@rjsf/utils'
import type { ArrayFieldItemButtonsTemplateProps, ArrayFieldItemTemplateProps, ArrayFieldTemplateProps, FieldProps, IconButtonProps, ObjectFieldTemplateProps, RJSFSchema, UiSchema, WidgetProps } from '@rjsf/utils'
import { open } from '@tauri-apps/plugin-dialog'
import { addAssetFromPath, errorMessage } from '../api.ts'
import { SINGULAR, buildUiSchema, errorsUnder, humanize, isWide, itemSummary, pointer, splitTags } from '../lib/form.ts'
import { roman } from '../lib/roman.ts'
import { Icon, type IconName } from './Icon.tsx'

/** A field to show: set when the user clicks the preview; `n` makes a repeat click count again. */
export type Reveal = { path: string; n: number }

type Ctx = {
  errors: string[]
  /** False while the form's first render runs, so existing cards start closed and new ones open. */
  fresh: { current: boolean }
  section: string
  setSection: (s: string) => void
  reveal?: Reveal
}
const FormCtx = createContext<Ctx>({ errors: [], fresh: { current: false }, section: 'basics', setSection: () => {} })

const errLabel = (n: number) => `${n} error${n === 1 ? '' : 's'}`
const SECTION_ICONS = new Set(['basics', 'work', 'volunteer', 'education', 'awards', 'certificates', 'publications', 'skills', 'languages', 'interests', 'references', 'projects', 'sideProjects'])
const sectionIcon = (k: string): IconName => (SECTION_ICONS.has(k) ? (k as IconName) : 'more')

/** Grows a textarea to fit its text; hidden ones (another section) are fitted when shown. */
const fit = (el: HTMLTextAreaElement) => {
  if (!el.offsetParent) return
  el.style.height = 'auto'
  el.style.height = `${el.scrollHeight + 2}px`
}

type Tab = { key: string; label: string; count?: number; errors: number }

/** A rail of section icons on the left; hovering (or keyboard focus) widens it rightwards to show the names. */
function SectionTabs({ tabs, active, onPick }: { tabs: Tab[]; active: string; onPick: (k: string) => void }) {
  const bar = useRef<HTMLDivElement>(null)
  // After a pick the rail folds back, even with the pointer still on it, so it no longer covers the form.
  const [rest, setRest] = useState(false)
  const onKeyDown = (e: React.KeyboardEvent) => {
    const i = tabs.findIndex((t) => t.key === active)
    const j = e.key === 'ArrowDown' ? i + 1 : e.key === 'ArrowUp' ? i - 1 : e.key === 'Home' ? 0 : e.key === 'End' ? tabs.length - 1 : -2
    if (j === -2) return
    e.preventDefault()
    const next = tabs[(j + tabs.length) % tabs.length]
    onPick(next.key)
    bar.current?.querySelector<HTMLElement>(`[data-key="${next.key}"]`)?.focus()
  }
  return (
    <nav className="section-nav">
      <div className={`section-tabs${rest ? ' rest' : ''}`} role="tablist" aria-label="CV sections" aria-orientation="vertical" ref={bar} onKeyDown={onKeyDown} onMouseLeave={() => setRest(false)}>
        {tabs.map((t) => (
          <button
            key={t.key} type="button" role="tab" data-key={t.key} aria-selected={t.key === active} tabIndex={t.key === active ? 0 : -1}
            className="section-tab" title={t.label} onClick={(e) => { onPick(t.key); if (e.detail) setRest(true) }}
          >
            <Icon name={sectionIcon(t.key)} size={15} />
            <span className="tab-label">{t.label}</span>
            {t.errors > 0 ? <span className="tab-err" title={errLabel(t.errors)} /> : !!t.count && <span className="tab-count">{t.count}</span>}
          </button>
        ))}
      </div>
    </nav>
  )
}

function Root({ properties, formData }: ObjectFieldTemplateProps) {
  const { errors, section, setSection } = useContext(FormCtx)
  const visible = properties.filter((p) => !p.hidden)
  const data = (formData ?? {}) as Record<string, unknown>
  const tabs = visible.map((p) => ({
    key: p.name, label: humanize(p.name), errors: errorsUnder(errors, [p.name]),
    count: Array.isArray(data[p.name]) ? (data[p.name] as unknown[]).length : undefined,
  }))
  const active = visible.some((p) => p.name === section) ? section : visible[0]?.name
  const panes = useRef<HTMLDivElement>(null)
  useLayoutEffect(() => {
    panes.current?.querySelectorAll<HTMLTextAreaElement>(`[data-path="/${active}"] textarea`).forEach(fit)
  }, [active])
  return (
    <div className="form-layout">
      <SectionTabs tabs={tabs} active={active} onPick={setSection} />
      <div className="panes" ref={panes}>
        {visible.map((p) => {
          const t = tabs.find((x) => x.key === p.name)!
          return (
            // Every section stays mounted, so open cards and scroll state survive switching tabs.
            <section key={p.name} className="pane" data-path={`/${p.name}`} hidden={p.name !== active} role="tabpanel" aria-label={t.label}>
              <header className="pane-head">
                <span className="ordinal" aria-hidden="true">{roman(tabs.indexOf(t) + 1)}</span>
                <h2>{t.label}</h2>
                {t.count !== undefined && <span className="count">{t.count}</span>}
                {t.errors > 0 && <span className="err-count">{errLabel(t.errors)}</span>}
              </header>
              {p.content}
            </section>
          )
        })}
      </div>
    </div>
  )
}

function ObjectFieldTemplate(props: ObjectFieldTemplateProps) {
  const { fieldPathId, title, properties, schema } = props
  const path = fieldPathId.path
  if (path.length === 0) return <Root {...props} />
  const kinds = (schema.properties ?? {}) as Record<string, RJSFSchema>
  const fields = (
    <div className="fields">
      {properties.filter((p) => !p.hidden).map((p) => (
        <div key={p.name} className={`prop${isWide(p.name, kinds[p.name] as Parameters<typeof isWide>[1]) ? ' wide' : ''}`} data-path={pointer([...path, p.name])}>
          {p.content}
        </div>
      ))}
    </div>
  )
  // A section (basics) has the pane heading; a list item has its card heading.
  if (path.length === 1 || typeof path[path.length - 1] === 'number') return fields
  return <fieldset className="group"><legend>{title}</legend>{fields}</fieldset>
}

function ArrayFieldTemplate({ fieldPathId, title: rawTitle, items, canAdd, onAddClick, disabled, readonly, formData, schema, uiSchema }: ArrayFieldTemplateProps) {
  const title = getUiOptions(uiSchema).title || rawTitle
  const path = fieldPathId.path
  const key = String(path[path.length - 1])
  const count = Array.isArray(formData) ? formData.length : 0
  const objects = (schema.items as RJSFSchema | undefined)?.type === 'object'
  const add = canAdd && (
    <button type="button" className="add" onClick={onAddClick} disabled={disabled || readonly}>
      <Icon name="plus" size={12} /> Add {SINGULAR[key] ?? 'item'}
    </button>
  )
  if (path.length === 1) {
    return (
      <>
        {count === 0 && <p className="empty tabula"><span className="latin" lang="la">Nihil adhuc</span>Nothing here yet. Sections without entries are left out of the CV.</p>}
        <div className="cards">{items}</div>
        {add}
      </>
    )
  }
  return (
    <div className="group list">
      <div className="group-label">{title}</div>
      <div className={objects ? 'cards' : 'rows'}>{items}</div>
      {add}
    </div>
  )
}

function ItemButtons({ hasMoveUp, hasMoveDown, hasRemove, totalItems, disabled, readonly, onMoveUpItem, onMoveDownItem, onRemoveItem }: ArrayFieldItemButtonsTemplateProps) {
  const off = disabled || readonly
  return (
    <div className="item-tools">
      {totalItems > 1 && (
        <>
          <button type="button" className="icon-btn" title="Move up" aria-label="Move up" disabled={off || !hasMoveUp} onClick={onMoveUpItem}><Icon name="up" size={12} /></button>
          <button type="button" className="icon-btn" title="Move down" aria-label="Move down" disabled={off || !hasMoveDown} onClick={onMoveDownItem}><Icon name="down" size={12} /></button>
        </>
      )}
      {hasRemove && <button type="button" className="icon-btn danger" title="Remove" aria-label="Remove" disabled={off} onClick={onRemoveItem}><Icon name="x" size={12} /></button>}
    </div>
  )
}

function ArrayFieldItemTemplate({ children, buttonsProps, schema, index }: ArrayFieldItemTemplateProps) {
  const { errors, fresh, reveal } = useContext(FormCtx)
  const [open, setOpen] = useState(fresh.current) // items added after the first render open at once
  const at = pointer(buttonsProps.fieldPathId.path)
  // A preview click on something inside this item opens it.
  useEffect(() => {
    if (reveal && (reveal.path === at || reveal.path.startsWith(`${at}/`))) setOpen(true) // oxlint-disable-line react/set-state-in-effect -- opening follows an outside request
  }, [reveal, at])
  const tools = <ItemButtons {...buttonsProps} />
  if (schema.type !== 'object') return <div className="row-item" data-path={at}><span className="bullet" aria-hidden="true" /><div className="grow">{children}</div>{tools}</div>
  const data = (children as ReactElement<{ formData?: unknown }>).props.formData
  const { title, sub, when } = itemSummary(data)
  const errs = errorsUnder(errors, buttonsProps.fieldPathId.path)
  return (
    <div className={`card${open ? ' open' : ''}`} data-path={at}>
      <div className="card-head">
        <button type="button" className="card-toggle" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
          <Icon name={open ? 'chevronDown' : 'chevronRight'} size={12} />
          <span className="card-text">
            <span className={`card-title${title ? '' : ' empty'}`}>{title || `New ${SINGULAR[String(buttonsProps.fieldPathId.path[buttonsProps.fieldPathId.path.length - 2])] ?? 'item'} ${index + 1}`}</span>
            {sub && <span className="card-sub">{sub}</span>}
          </span>
          {when && <span className="card-when">{when}</span>}
          {errs > 0 && <span className="err-count" title={errLabel(errs)}>{errs}</span>}
        </button>
        {tools}
      </div>
      {open && <div className="card-body">{children}</div>}
    </div>
  )
}

/** A list of short words (keywords, courses) as removable chips; Enter or a comma adds one. */
function TagsField({ formData, onChange, fieldPathId, disabled, readonly, uiSchema, name }: FieldProps) {
  const tags: string[] = Array.isArray(formData) ? formData.filter((t) => typeof t === 'string') : []
  const [text, setText] = useState('')
  const input = useRef<HTMLInputElement>(null)
  const off = disabled || readonly
  const set = (next: string[]) => onChange(next, fieldPathId.path)
  const commit = () => {
    const add = splitTags(text)
    if (add.length) set([...tags, ...add])
    setText('')
  }
  const label = (uiSchema?.['ui:title'] as string | undefined) ?? humanize(name)
  return (
    <div className="tags-field">
      <label className="control-label" htmlFor={fieldPathId.$id}>{label}</label>
      <div className="tags-box" onMouseDown={(e) => { if (e.target === e.currentTarget) { e.preventDefault(); input.current?.focus() } }}>
        {tags.map((t, i) => (
          <span className="tag" key={`${i}-${t}`}>
            {t}
            {!off && <button type="button" aria-label={`Remove ${t}`} onClick={() => set(tags.filter((_, j) => j !== i))}><Icon name="x" size={10} /></button>}
          </span>
        ))}
        <input
          ref={input} id={fieldPathId.$id} value={text} disabled={off} placeholder={tags.length ? '' : 'Type, then press Enter'}
          onChange={(e) => (/[,\n]/.test(e.target.value) ? (set([...tags, ...splitTags(e.target.value)]), setText('')) : setText(e.target.value))}
          onKeyDown={(e) => {
            if (e.key === 'Enter') { e.preventDefault(); commit() }
            else if (e.key === 'Backspace' && !text && tags.length) set(tags.slice(0, -1))
          }}
          onBlur={commit}
        />
      </div>
    </div>
  )
}

function TextareaWidget({ id, value, onChange, onBlur, onFocus, disabled, readonly, placeholder, options }: WidgetProps) {
  const ref = useRef<HTMLTextAreaElement>(null)
  useLayoutEffect(() => { if (ref.current) fit(ref.current) }, [value])
  return (
    <textarea
      ref={ref} id={id} value={value ?? ''} rows={(options.rows as number | undefined) ?? 3} disabled={disabled} readOnly={readonly} placeholder={placeholder}
      onChange={(e) => onChange(e.target.value === '' ? options.emptyValue : e.target.value)}
      onBlur={(e) => onBlur(id, e.target.value)} onFocus={(e) => { fit(e.target); onFocus(id, e.target.value) }}
    />
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
        <input id={id} type="text" value={value ?? ''} placeholder="assets/photo.jpg or https://" disabled={disabled} readOnly={readonly} onChange={(e) => onChange(e.target.value || undefined)} />
        <button type="button" disabled={disabled || readonly} onClick={choose}>Choose…</button>
      </div>
      {err && <div className="field-error">{err}</div>}
    </div>
  )
}

/** An end date, or "Present" (no value: the role is ongoing). */
function EndDateWidget({ id, value, onChange, onBlur, onFocus, disabled, readonly, placeholder }: WidgetProps) {
  const input = useRef<HTMLInputElement>(null)
  const present = !value
  return (
    <div className="date-row">
      <input
        ref={input} id={id} type="text" value={value ?? ''} disabled={disabled} readOnly={readonly}
        placeholder={present ? 'Present' : placeholder}
        onChange={(e) => onChange(e.target.value || undefined)}
        onBlur={(e) => onBlur(id, e.target.value)} onFocus={(e) => onFocus(id, e.target.value)}
      />
      <label className="check">
        <input type="checkbox" checked={present} disabled={disabled || readonly} onChange={(e) => (e.target.checked ? onChange(undefined) : input.current?.focus())} />
        Present
      </label>
    </div>
  )
}

const textButton = (label: string) =>
  function TextButton({ id, title, disabled, onClick }: IconButtonProps) {
    return <button type="button" id={id} title={title} aria-label={title} disabled={disabled} onClick={onClick}>{label}</button>
  }
const templates = {
  ObjectFieldTemplate,
  ArrayFieldTemplate,
  ArrayFieldItemTemplate,
  ButtonTemplates: { AddButton: textButton('+ Add'), RemoveButton: textButton('✕'), MoveUpButton: textButton('↑'), MoveDownButton: textButton('↓') },
}
const widgets = { imageFile: ImageWidget, endDate: EndDateWidget, TextareaWidget }
const fields = { tags: TagsField }

type Props = {
  schema: RJSFSchema; draft: unknown; errors: string[]; fresh: { current: boolean }; onChange: (d: unknown) => void
  section: string; onSection: (s: string) => void; reveal?: Reveal
}

export function CvForm({ schema, draft, errors, fresh, onChange, section, onSection, reveal }: Props) {
  const uiSchema = useMemo<UiSchema>(() => {
    const ui = buildUiSchema(schema as Parameters<typeof buildUiSchema>[0])
    return {
      ...ui,
      'ui:title': '',
      'ui:description': '',
      'ui:submitButtonOptions': { norender: true }, // edits save as you type
      $schema: { 'ui:widget': 'hidden' },
      meta: { 'ui:widget': 'hidden' }, // tool settings (theme, page setup); the JSON editor still shows them
      basics: { ...ui.basics, image: { ...ui.basics?.image, 'ui:widget': 'imageFile' } },
    }
  }, [schema])
  const ctx = useMemo(() => ({ errors, fresh, section, setSection: onSection, reveal }), [errors, fresh, section, onSection, reveal])
  return (
    <FormCtx.Provider value={ctx}>
      <Form
        schema={schema}
        uiSchema={uiSchema}
        formData={draft}
        validator={validator}
        templates={templates}
        widgets={widgets}
        fields={fields}
        liveValidate
        noHtml5Validate
        showErrorList={false}
        tagName="div"
        onChange={(e) => onChange(e.formData)}
      />
    </FormCtx.Provider>
  )
}
