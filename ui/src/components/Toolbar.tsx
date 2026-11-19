import { useEffect, useRef, useState } from 'react'
import { DEFAULT_THEME, type ExportFormat, type ThemeInfo, type ThemeReport, previewHtml, themeReport } from '../api.ts'
import { cvTitle } from '../lib/names.ts'
import { MOD } from '../lib/platform.ts'
import { withHead } from '../lib/preview.ts'
import { Icon } from './Icon.tsx'

type Props = {
  name: string; theme: string | null; themes: ThemeInfo[]; onTheme: (id: string) => void; onManageThemes: () => void
  editing: boolean; viewing: boolean; exporting: boolean; onEdit: () => void; onExport: (f: ExportFormat) => void; onPrint: () => void
}

/** Open state, outside-click close and arrow-key focus for a popup menu. */
function useMenu() {
  const [open, setOpen] = useState(false)
  const box = useRef<HTMLDivElement>(null)
  const items = () => Array.from(box.current?.querySelectorAll<HTMLElement>('[role^="menuitem"]') ?? [])
  useEffect(() => {
    if (!open) return
    const els = items()
    ;(els.find((e) => e.getAttribute('aria-checked') === 'true') ?? els[0])?.focus()
    const away = (e: MouseEvent) => !box.current?.contains(e.target as Node) && setOpen(false)
    window.addEventListener('mousedown', away)
    return () => window.removeEventListener('mousedown', away)
  }, [open])
  const onKeyDown = (e: React.KeyboardEvent) => {
    const els = items()
    const i = els.indexOf(document.activeElement as HTMLElement)
    if (e.key === 'Escape') { setOpen(false); box.current?.querySelector<HTMLElement>('.trigger')?.focus() }
    else if (e.key === 'ArrowDown') { e.preventDefault(); els[(i + 1) % els.length]?.focus() }
    else if (e.key === 'ArrowUp') { e.preventDefault(); els[(i - 1 + els.length) % els.length]?.focus() }
  }
  return { open, setOpen, box, onKeyDown }
}

function ExportMenu({ disabled, busy, onExport, onPrint }: { disabled: boolean; busy: boolean; onExport: (f: ExportFormat) => void; onPrint: () => void }) {
  const { open, setOpen, box, onKeyDown } = useMenu()
  const pick = (go: () => void) => () => { setOpen(false); go() }
  return (
    <div className="popup" ref={box} onKeyDown={onKeyDown}>
      <button className="trigger" aria-haspopup="menu" aria-expanded={open} disabled={disabled || busy} title={disabled ? 'Go back to the current version to export' : undefined} onClick={() => setOpen((o) => !o)}>
        {busy ? <><span className="spinner" aria-hidden="true" /> Exporting…</> : <>Export <Icon name="chevronDown" size={12} /></>}
      </button>
      {open && (
        <div className="menu" role="menu">
          <button role="menuitem" onClick={pick(() => onExport('pdf'))}>PDF<kbd>{MOD}⇧E</kbd></button>
          <button role="menuitem" onClick={pick(() => onExport('html'))}>HTML</button>
          <button role="menuitem" onClick={pick(() => onExport('json'))}>JSON</button>
          <div className="menu-sep" role="separator" />
          <button role="menuitem" onClick={pick(onPrint)}>Print…<kbd>{MOD}P</kbd></button>
        </div>
      )}
    </div>
  )
}

function ThemePicker({ cv, value, themes, onPick, onManage }: { cv: string; value: string; themes: ThemeInfo[]; onPick: (id: string) => void; onManage: () => void }) {
  const { open, setOpen, box, onKeyDown } = useMenu()
  const [reports, setReports] = useState<Record<string, ThemeReport>>({})
  const [thumbs, setThumbs] = useState<Record<string, string>>({})
  useEffect(() => {
    if (!open) return
    let live = true
    for (const t of themes) {
      themeReport(t.id).then((r) => live && setReports((x) => ({ ...x, [t.id]: r })), () => {})
      previewHtml(cv, t.id).then((h) => live && setThumbs((x) => ({ ...x, [`${cv}|${t.id}`]: withHead(h) })), () => {})
    }
    return () => { live = false }
  }, [open, themes, cv])
  const current = themes.find((t) => t.id === value)
  const pick = (id: string) => { setOpen(false); if (id !== value) onPick(id) }
  return (
    <div className="popup" ref={box} onKeyDown={onKeyDown}>
      <button className="trigger theme-trigger" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        <span className="muted">Theme</span> {current?.meta.name ?? value} <Icon name="chevronDown" size={12} />
      </button>
      {open && (
        <div className="menu theme-menu" role="menu" aria-label="Theme">
          {themes.map((t) => {
            const r = reports[t.id]
            const note = r?.render_error ? 'Render error' : r?.missing.length ? `Hides ${r.missing.join(', ')}` : r ? 'Shows every field' : ''
            return (
              <button key={t.id} role="menuitemradio" aria-checked={t.id === value} className="theme-option" onClick={() => pick(t.id)}>
                <span className="thumb" aria-hidden="true">
                  {thumbs[`${cv}|${t.id}`] && <iframe tabIndex={-1} title="" sandbox="allow-same-origin" srcDoc={thumbs[`${cv}|${t.id}`]} />}
                </span>
                <span className="theme-meta">
                  <span className="theme-name">{t.meta.name}{t.builtin && <span className="badge soft">built-in</span>}</span>
                  <span className={`theme-note${r?.missing.length || r?.render_error ? ' warn' : ''}`}>{note}</span>
                </span>
                {t.id === value && <span className="check-mark"><Icon name="check" /></span>}
              </button>
            )
          })}
          <div className="menu-sep" role="separator" />
          <button role="menuitem" onClick={() => { setOpen(false); onManage() }}>Manage themes…</button>
        </div>
      )}
    </div>
  )
}

export function Toolbar({ name, theme, themes, onTheme, onManageThemes, editing, viewing, exporting, onEdit, onExport, onPrint }: Props) {
  return (
    <header className="toolbar">
      <h1>{cvTitle(name)}</h1>
      <span className="spacer" />
      <ThemePicker cv={name} value={theme ?? DEFAULT_THEME} themes={themes} onPick={onTheme} onManage={onManageThemes} />
      <button className={editing ? 'on' : undefined} aria-pressed={editing} disabled={viewing} title={viewing ? 'Go back to the current version to edit' : undefined} onClick={onEdit}>Edit <kbd>{MOD}E</kbd></button>
      <ExportMenu disabled={viewing} busy={exporting} onExport={onExport} onPrint={onPrint} />
    </header>
  )
}
