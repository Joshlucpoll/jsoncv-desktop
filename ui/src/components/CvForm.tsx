import { createContext, type ReactElement, useContext, useMemo, useRef, useState } from 'react'
import Form from '@rjsf/core'
import validator from '@rjsf/validator-ajv8'
import { getUiOptions } from '@rjsf/utils'
import type { ArrayFieldItemButtonsTemplateProps, ArrayFieldItemTemplateProps, ArrayFieldTemplateProps, IconButtonProps, ObjectFieldTemplateProps, RJSFSchema, UiSchema, WidgetProps } from '@rjsf/utils'
import { open } from '@tauri-apps/plugin-dialog'
import { addAssetFromPath, errorMessage } from '../api.ts'
import { buildUiSchema, errorsUnder, itemSummary } from '../lib/form.ts'
import { Icon } from './Icon.tsx'

/** Validation error paths, plus whether newly mounted cards start open (true once the form has rendered). */
const FormCtx = createContext<{ errors: string[]; fresh: { current: boolean } }>({ errors: [], fresh: { current: false } })

const errLabel = (n: number) => `${n} error${n === 1 ? '' : 's'}`

function Section({ id, title, count, errors, defaultOpen, children }: { id: string; title: string; count?: number; errors: number; defaultOpen: boolean; children: React.ReactNode }) {
  return (
    <details className="section" id={id} open={defaultOpen}>
      <summary>
        <Icon name="chevronRight" />
        <span className="section-title">{title}</span>
        {count !== undefined && <span className="count">{count}</span>}
        {errors > 0 && <span className="err-count">{errLabel(errors)}</span>}
      </summary>
      <div className="section-body">{children}</div>
    </details>
  )
}

function ObjectFieldTemplate({ fieldPathId, title, properties }: ObjectFieldTemplateProps) {
  const { errors } = useContext(FormCtx)
  const body = properties.filter((p) => !p.hidden).map((p) => <div key={p.name} className="prop">{p.content}</div>)
  const path = fieldPathId.path
  if (path.length === 0) return <>{body}</>
  if (path.length === 1) return <Section id={fieldPathId.$id} title={title} errors={errorsUnder(errors, path)} defaultOpen={path[0] === 'basics'}>{body}</Section>
  // A list item's card already has a heading.
  if (typeof path[path.length - 1] === 'number') return <>{body}</>
  return <fieldset className="nested"><legend>{title}</legend>{body}</fieldset>
}

function ArrayFieldTemplate({ fieldPathId, title: rawTitle, items, canAdd, onAddClick, disabled, readonly, formData, schema, uiSchema }: ArrayFieldTemplateProps) {
  const { errors } = useContext(FormCtx)
  const title = getUiOptions(uiSchema).title || rawTitle
  const path = fieldPathId.path
  const count = Array.isArray(formData) ? formData.length : 0
  const objects = (schema.items as RJSFSchema | undefined)?.type === 'object'
  const body = (
    <>
      <div className={objects ? 'cards' : 'rows'}>{items}</div>
      {canAdd && (
        <button type="button" className="add" onClick={onAddClick} disabled={disabled || readonly}>
          <Icon name="plus" /> Add{objects ? '' : ` ${title.toLowerCase().replace(/s$/, '')}`}
        </button>
      )}
    </>
  )
  if (path.length === 1) return <Section id={fieldPathId.$id} title={title} count={count} errors={errorsUnder(errors, path)} defaultOpen={false}>{body}</Section>
  return <fieldset className="nested"><legend>{title}</legend>{body}</fieldset>
}

function ItemButtons({ hasMoveUp, hasMoveDown, hasRemove, totalItems, disabled, readonly, onMoveUpItem, onMoveDownItem, onRemoveItem }: ArrayFieldItemButtonsTemplateProps) {
  const off = disabled || readonly
  return (
    <div className="item-tools">
      {totalItems > 1 && (
        <>
          <button type="button" className="icon-btn" title="Move up" aria-label="Move up" disabled={off || !hasMoveUp} onClick={onMoveUpItem}><Icon name="up" /></button>
          <button type="button" className="icon-btn" title="Move down" aria-label="Move down" disabled={off || !hasMoveDown} onClick={onMoveDownItem}><Icon name="down" /></button>
        </>
      )}
      {hasRemove && <button type="button" className="icon-btn danger" title="Remove" aria-label="Remove" disabled={off} onClick={onRemoveItem}><Icon name="x" /></button>}
    </div>
  )
}

function ArrayFieldItemTemplate({ children, buttonsProps, schema, index }: ArrayFieldItemTemplateProps) {
  const { errors, fresh } = useContext(FormCtx)
  const [open, setOpen] = useState(fresh.current) // items added after the first render open at once
  const tools = <ItemButtons {...buttonsProps} />
  if (schema.type !== 'object') return <div className="row-item"><div className="grow">{children}</div>{tools}</div>
  const data = (children as ReactElement<{ formData?: unknown }>).props.formData
  const { title, sub } = itemSummary(data)
  const errs = errorsUnder(errors, buttonsProps.fieldPathId.path)
  return (
    <div className={`card${open ? ' open' : ''}`}>
      <div className="card-head">
        <button type="button" className="card-toggle" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
          <Icon name={open ? 'chevronDown' : 'chevronRight'} />
          <span className={`card-title${title ? '' : ' empty'}`}>{title || `New item ${index + 1}`}</span>
          {sub && <span className="card-sub">{sub}</span>}
          {errs > 0 && <span className="err-count" title={errLabel(errs)}>{errs}</span>}
        </button>
        {tools}
      </div>
      {open && <div className="card-body">{children}</div>}
    </div>
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
const widgets = { imageFile: ImageWidget, endDate: EndDateWidget }

type Props = { schema: RJSFSchema; draft: unknown; errors: string[]; fresh: { current: boolean }; onChange: (d: unknown) => void }

export function CvForm({ schema, draft, errors, fresh, onChange }: Props) {
  const uiSchema = useMemo<UiSchema>(() => {
    const ui = buildUiSchema(schema as Parameters<typeof buildUiSchema>[0])
    return {
      ...ui,
      'ui:title': '',
      'ui:description': '',
      $schema: { 'ui:widget': 'hidden' },
      meta: { 'ui:widget': 'hidden' }, // tool settings (theme, tailoredFrom); the JSON tab still shows them
      basics: { ...ui.basics, image: { ...ui.basics?.image, 'ui:widget': 'imageFile' } },
      // The role reads first on a CV, so it comes before the employer.
      work: { ...ui.work, items: { ...ui.work?.items, 'ui:order': ['position', 'name', '*'] } },
      volunteer: { ...ui.volunteer, items: { ...ui.volunteer?.items, 'ui:order': ['position', 'organization', '*'] } },
    }
  }, [schema])
  const ctx = useMemo(() => ({ errors, fresh }), [errors, fresh])
  return (
    <FormCtx.Provider value={ctx}>
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
        onChange={(e) => onChange(e.formData)}
      />
    </FormCtx.Provider>
  )
}
