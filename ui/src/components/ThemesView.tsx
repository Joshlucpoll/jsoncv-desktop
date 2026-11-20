import { useEffect, useState } from 'react'
import { confirm } from '@tauri-apps/plugin-dialog'
import { DEFAULT_THEME, MOCK, type ThemeInfo, type ThemeReport, deleteTheme, duplicateTheme, errorMessage, reveal, themeDirPath, themeReport } from '../api.ts'
import { revealLabel } from '../lib/platform.ts'
import { themeIdFromName } from '../lib/themes.ts'
import { Icon } from './Icon.tsx'

type RowProps = {
  theme: ThemeInfo; reloadKey: number; usedBy: string[]
  onDuplicate: () => void; onDeleted: () => void; onError: (e: unknown) => void
}

/** One theme in Settings: what it hides, plus duplicate, open and delete. */
export function ThemeRow({ theme, reloadKey, usedBy, onDuplicate, onDeleted, onError }: RowProps) {
  const [report, setReport] = useState<ThemeReport>()

  useEffect(() => {
    let live = true
    themeReport(theme.id).then((r) => live && setReport(r), (e) => live && setReport({ render_error: { message: errorMessage(e), line: null }, missing: [] }))
    return () => { live = false }
  }, [theme.id, reloadKey])

  const remove = async () => {
    const name = theme.meta.name
    const msg = `Move ${name} to the Trash?` + (usedBy.length
      ? ` ${usedBy.length} CV${usedBy.length === 1 ? ' uses' : 's use'} it and will switch to Classic: ${usedBy.join(', ')}.`
      : '')
    // The dialog plugin replaces window.confirm with an async version, so use its confirm().
    if (!(MOCK ? window.confirm(msg) : await confirm(msg, { title: 'Delete theme', kind: 'warning' }))) return
    try { await deleteTheme(theme.id); onDeleted() } catch (e) { onError(e) }
  }

  const note = report?.render_error
    ? `Render error${report.render_error.line != null ? ` (line ${report.render_error.line})` : ''}: ${report.render_error.message}`
    : report?.missing.length ? `Hides ${report.missing.join(', ')}` : report ? 'Shows every field' : ''
  return (
    <div className="theme-row">
      <div className="theme-head">
        <strong>{theme.meta.name}</strong>
        {theme.builtin && <span className="badge soft">built-in</span>}
        <span className="spacer" />
        <button onClick={onDuplicate}>Duplicate…</button>
        {!theme.builtin && <button onClick={() => themeDirPath(theme.id).then(reveal).catch(onError)}>{revealLabel}</button>}
        {!theme.builtin && <button className="icon-btn danger" aria-label="Delete theme" title="Delete theme" onClick={() => void remove()}><Icon name="trash" /></button>}
      </div>
      {note && <p className={`theme-note${report?.render_error || report?.missing.length ? ' warn' : ''}`}>{note}</p>}
    </div>
  )
}

type NewProps = { themes: ThemeInfo[]; from: string; onCreated: (t: ThemeInfo) => void; onCancel: () => void; onError: (e: unknown) => void }

/** Name a new theme and pick the theme it starts as a copy of. */
export function NewTheme({ themes, from, onCreated, onCancel, onError }: NewProps) {
  const [name, setName] = useState('')
  const [src, setSrc] = useState(themes.some((t) => t.id === from) ? from : DEFAULT_THEME)
  const [err, setErr] = useState<string>()
  const id = themeIdFromName(name)
  const taken = themes.some((t) => t.id === id)
  const problem = !name.trim() ? undefined : !id ? 'Use at least one letter or digit.' : taken ? 'A theme with this folder already exists.' : undefined

  const submit = async () => {
    if (!id || taken) return
    try { onCreated(await duplicateTheme(src, id, name.trim())) } catch (e) { setErr(errorMessage(e)); onError(e) }
  }

  return (
    <form className="new-theme" onSubmit={(e) => { e.preventDefault(); void submit() }}
      onKeyDown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); onCancel() } }}>
      <label>Name
        <input autoFocus placeholder="My theme" value={name} onChange={(e) => { setName(e.target.value); setErr(undefined) }} />
      </label>
      <label>Start from
        <select value={src} onChange={(e) => setSrc(e.target.value)}>
          {themes.map((t) => <option key={t.id} value={t.id}>{t.meta.name}</option>)}
        </select>
      </label>
      <div className="new-theme-foot">
        <span className={problem || err ? 'field-error' : 'muted'}>{problem ?? err ?? (id ? `Folder: themes/${id}` : ' ')}</span>
        <span className="spacer" />
        <button type="button" onClick={onCancel}>Cancel</button>
        <button type="submit" className="primary" disabled={!id || taken}>Create</button>
      </div>
    </form>
  )
}
