import { useEffect, useState } from 'react'
import { type ThemeInfo, type ThemeReport, duplicateTheme, errorMessage, reveal, themeReport } from '../api.ts'
import { THEME_ID, revealLabel } from '../lib/platform.ts'

export function ThemeList({ themes, selected, onSelect }: { themes: ThemeInfo[]; selected?: string; onSelect: (id: string) => void }) {
  return (
    <div className="theme-list" role="listbox" aria-label="Themes">
      {themes.map((t) => (
        <button key={t.id} role="option" aria-selected={t.id === selected} className={`item${t.id === selected ? ' on' : ''}`} onClick={() => onSelect(t.id)}>
          <span className="label">{t.meta.name}</span>
          {t.builtin && <span className="badge soft">built-in</span>}
        </button>
      ))}
    </div>
  )
}

type Props = {
  theme: ThemeInfo; library: string; reloadKey: number; cvName: string
  onUse: () => void; onDuplicated: (t: ThemeInfo) => void; onError: (e: unknown) => void
}

/** Report panel and actions for the selected theme; the preview itself is rendered by the parent. */
export function ThemeDetail({ theme, library, reloadKey, cvName, onUse, onDuplicated, onError }: Props) {
  const [report, setReport] = useState<ThemeReport>()
  const [dup, setDup] = useState<string>() // undefined = closed
  const [dupErr, setDupErr] = useState<string>()

  useEffect(() => {
    let live = true
    themeReport(theme.id).then((r) => live && setReport(r), (e) => live && setReport({ render_error: { message: errorMessage(e), line: null }, missing: [] }))
    return () => { live = false }
  }, [theme.id, reloadKey])

  const submit = async () => {
    const id = (dup ?? '').trim()
    if (!THEME_ID.test(id)) return setDupErr('Use lowercase letters, digits and hyphens (max 64).')
    try {
      onDuplicated(await duplicateTheme(theme.id, id))
      setDup(undefined)
      setDupErr(undefined)
    } catch (e) {
      setDupErr(errorMessage(e))
      onError(e)
    }
  }

  return (
    <div className="theme-panel">
      <div className="theme-head">
        <strong>{theme.meta.name}</strong>
        {theme.builtin && <span className="badge soft">built-in</span>}
        <span className="spacer" />
        <button onClick={onUse} title={`Set as the theme of ${cvName}`}>Use for this CV</button>
        <button onClick={() => { setDup(''); setDupErr(undefined) }}>Duplicate…</button>
        {!theme.builtin && <button onClick={() => reveal(`${library}/themes/${theme.id}`).catch(onError)}>{revealLabel}</button>}
      </div>
      {dup !== undefined && (
        <form className="dup-form" onSubmit={(e) => { e.preventDefault(); void submit() }}>
          <input autoFocus aria-label="New theme id" placeholder="new-theme-id" value={dup} onChange={(e) => { setDup(e.target.value); setDupErr(undefined) }} onKeyDown={(e) => e.key === 'Escape' && setDup(undefined)} />
          <button type="submit">Duplicate</button>
          <button type="button" onClick={() => setDup(undefined)}>Cancel</button>
          {dupErr && <div className="field-error">{dupErr}</div>}
        </form>
      )}
      {report?.render_error ? (
        <p className="report error" role="alert">
          Render error{report.render_error.line != null && ` (line ${report.render_error.line})`}: {report.render_error.message}
        </p>
      ) : report && (
        <p className={`report${report.missing.length ? ' warn' : ''}`}>
          {report.missing.length ? `⚠ Not shown: ${report.missing.join(', ')}` : 'All schema fields are shown'}
        </p>
      )}
    </div>
  )
}
