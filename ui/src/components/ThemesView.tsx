import { useEffect, useState } from 'react'
import { type ThemeInfo, type ThemeReport, duplicateTheme, errorMessage, reveal, themeDirPath, themeReport } from '../api.ts'
import { THEME_ID, revealLabel } from '../lib/platform.ts'

type Props = { theme: ThemeInfo; reloadKey: number; onDuplicated: (t: ThemeInfo) => void; onError: (e: unknown) => void }

/** One theme in Settings: what it hides, plus duplicate and reveal. */
export function ThemeRow({ theme, reloadKey, onDuplicated, onError }: Props) {
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

  const note = report?.render_error
    ? `Render error${report.render_error.line != null ? ` (line ${report.render_error.line})` : ''}: ${report.render_error.message}`
    : report?.missing.length ? `Hides ${report.missing.join(', ')}` : report ? 'Shows every field' : ''
  return (
    <div className="theme-row">
      <div className="theme-head">
        <strong>{theme.meta.name}</strong>
        {theme.builtin && <span className="badge soft">built-in</span>}
        <span className="spacer" />
        <button onClick={() => { setDup(''); setDupErr(undefined) }}>Duplicate…</button>
        {!theme.builtin && <button onClick={() => themeDirPath(theme.id).then(reveal).catch(onError)}>{revealLabel}</button>}
      </div>
      {note && <p className={`theme-note${report?.render_error || report?.missing.length ? ' warn' : ''}`}>{note}</p>}
      {dup !== undefined && (
        <form className="dup-form" onSubmit={(e) => { e.preventDefault(); void submit() }}>
          <input
            autoFocus aria-label="New theme id" placeholder="new-theme-id" value={dup}
            onChange={(e) => { setDup(e.target.value); setDupErr(undefined) }}
            onKeyDown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); setDup(undefined) } }}
          />
          <button type="submit">Duplicate</button>
          <button type="button" onClick={() => setDup(undefined)}>Cancel</button>
          {dupErr && <div className="field-error">{dupErr}</div>}
        </form>
      )}
    </div>
  )
}
