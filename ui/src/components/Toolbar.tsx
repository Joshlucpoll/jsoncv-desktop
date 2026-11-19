import { useEffect, useState } from 'react'
import { DEFAULT_THEME, type ExportFormat, type ThemeInfo, type ThemeReport, previewHtml, themeReport } from '../api.ts'
import { MOD } from '../lib/platform.ts'
import { withHead } from '../lib/preview.ts'
import { Icon } from './Icon.tsx'
import { useMenu } from '../lib/menu.ts'

type Props = {
  name: string; theme: string | null; themes: ThemeInfo[]; onTheme: (id: string) => void; onManageThemes: () => void
  editing: boolean; exporting: boolean; onEdit: () => void; onExport: (f: ExportFormat) => void; onPrint: () => void
  /** The edit panel shows the JSON editor. */
  json: boolean; onJson: () => void
  continuous: boolean; onPageMode: (continuous: boolean) => void
}

function ExportMenu({ busy, onExport, onPrint }: { busy: boolean; onExport: (f: ExportFormat) => void; onPrint: () => void }) {
  const { open, setOpen, box, onKeyDown } = useMenu()
  const pick = (go: () => void) => () => { setOpen(false); go() }
  return (
    <div className="popup" ref={box} onKeyDown={onKeyDown}>
      <button className="trigger primary" aria-haspopup="menu" aria-expanded={open} disabled={busy} onClick={() => setOpen((o) => !o)}>
        {busy ? <><span className="spinner" aria-hidden="true" /> Exporting…</> : <><Icon name="download" /> Export <Icon name="chevronDown" size={12} /></>}
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

/** Per-CV options: how to edit it and how its pages are laid out. */
function OptionsMenu({ json, onJson, continuous, onPageMode }: Pick<Props, 'json' | 'onJson' | 'continuous' | 'onPageMode'>) {
  const { open, setOpen, box, onKeyDown } = useMenu()
  const pick = (go: () => void) => () => { setOpen(false); go() }
  return (
    <div className="popup" ref={box} onKeyDown={onKeyDown}>
      <button className="trigger icon-only" aria-haspopup="menu" aria-expanded={open} aria-label="Options" title="Options" onClick={() => setOpen((o) => !o)}>
        <Icon name="sliders" />
      </button>
      {open && (
        <div className="menu options-menu" role="menu" aria-label="Options">
          <button role="menuitem" onClick={pick(onJson)}><Icon name={json ? 'basics' : 'braces'} />{json ? 'Edit with the form' : 'Edit JSON directly'}</button>
          <div className="menu-sep" role="separator" />
          <div className="menu-heading" role="presentation">Page setup</div>
          <button role="menuitemradio" aria-checked={!continuous} onClick={pick(() => continuous && onPageMode(false))}>
            <Icon name="pages" />
            <span className="opt-text"><span>Multiple pages</span><span className="opt-note">Paper-sized pages with page breaks</span></span>
            {!continuous && <span className="check-mark"><Icon name="check" /></span>}
          </button>
          <button role="menuitemradio" aria-checked={continuous} onClick={pick(() => !continuous && onPageMode(true))}>
            <Icon name="scroll" />
            <span className="opt-text"><span>Continuous page</span><span className="opt-note">One long page, for screens and web links</span></span>
            {continuous && <span className="check-mark"><Icon name="check" /></span>}
          </button>
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
      <button className="trigger theme-trigger" aria-haspopup="menu" aria-expanded={open} title="Theme" onClick={() => setOpen((o) => !o)}>
        <Icon name="palette" /> {current?.meta.name ?? value} <Icon name="chevronDown" size={12} />
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

export function Toolbar({ name, theme, themes, onTheme, onManageThemes, editing, exporting, onEdit, onExport, onPrint, json, onJson, continuous, onPageMode }: Props) {
  return (
    <header className="toolbar" data-tauri-drag-region>
      <h1 data-tauri-drag-region>{name}</h1>
      <span className="spacer" data-tauri-drag-region />
      {/* Least to most important, left to right: the primary action sits at the edge. */}
      <OptionsMenu json={json} onJson={onJson} continuous={continuous} onPageMode={onPageMode} />
      <ThemePicker cv={name} value={theme ?? DEFAULT_THEME} themes={themes} onPick={onTheme} onManage={onManageThemes} />
      <span className="tb-sep" aria-hidden="true" />
      <button className={`with-icon${editing ? ' on' : ''}`} aria-pressed={editing} title={`Edit (${MOD}E)`} onClick={onEdit}><Icon name="pencil" />Edit</button>
      <ExportMenu busy={exporting} onExport={onExport} onPrint={onPrint} />
    </header>
  )
}
