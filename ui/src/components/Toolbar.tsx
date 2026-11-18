import { useEffect, useRef, useState } from 'react'
import { DEFAULT_THEME, type ExportFormat, type ThemeInfo } from '../api.ts'

type Props = { name: string; theme: string | null; themes: ThemeInfo[]; onTheme: (id: string) => void; editing: boolean; viewing: boolean; onEdit: () => void; onExport: (f: ExportFormat) => void; onPrint: () => void }

const title = (name: string) => (name === 'main' ? 'Main CV' : name.replace(/^tailored\//, ''))

function ExportMenu({ disabled, onExport, onPrint }: { disabled: boolean; onExport: (f: ExportFormat) => void; onPrint: () => void }) {
  const [open, setOpen] = useState(false)
  const box = useRef<HTMLDivElement>(null)
  const items = () => Array.from(box.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]') ?? [])
  useEffect(() => {
    if (!open) return
    items()[0]?.focus()
    const away = (e: MouseEvent) => !box.current?.contains(e.target as Node) && setOpen(false)
    window.addEventListener('mousedown', away)
    return () => window.removeEventListener('mousedown', away)
  }, [open])
  const key = (e: React.KeyboardEvent) => {
    const els = items()
    const i = els.indexOf(document.activeElement as HTMLButtonElement)
    if (e.key === 'Escape') { setOpen(false); (box.current?.querySelector('.trigger') as HTMLElement)?.focus() }
    else if (e.key === 'ArrowDown') { e.preventDefault(); els[(i + 1) % els.length]?.focus() }
    else if (e.key === 'ArrowUp') { e.preventDefault(); els[(i - 1 + els.length) % els.length]?.focus() }
  }
  const pick = (go: () => void) => () => { setOpen(false); go() }
  return (
    <div className="export-menu" ref={box} onKeyDown={key}>
      <button className="trigger" aria-haspopup="menu" aria-expanded={open} disabled={disabled} title={disabled ? 'Go back to the current version to export' : undefined} onClick={() => setOpen((o) => !o)}>Export ▾</button>
      {open && (
        <div className="menu" role="menu">
          <button role="menuitem" onClick={pick(() => onExport('pdf'))}>PDF</button>
          <button role="menuitem" onClick={pick(() => onExport('html'))}>HTML</button>
          <button role="menuitem" onClick={pick(() => onExport('json'))}>JSON</button>
          <button role="menuitem" onClick={pick(onPrint)}>Print…</button>
        </div>
      )}
    </div>
  )
}

export function Toolbar({ name, theme, themes, onTheme, editing, viewing, onEdit, onExport, onPrint }: Props) {
  const value = theme ?? DEFAULT_THEME
  const builtin = themes.filter((t) => t.builtin)
  const custom = themes.filter((t) => !t.builtin)
  return (
    <header className="toolbar">
      <h1>{title(name)}</h1>
      <span className="spacer" />
      <label className="picker">
        <span>Theme</span>
        <select value={value} onChange={(e) => onTheme(e.target.value)}>
          {!themes.some((t) => t.id === value) && <option value={value}>{value}</option>}
          <optgroup label="Built-in">
            {builtin.map((t) => <option key={t.id} value={t.id}>{t.meta.name}</option>)}
          </optgroup>
          {custom.length > 0 && (
            <optgroup label="Library">
              {custom.map((t) => <option key={t.id} value={t.id}>{t.meta.name}</option>)}
            </optgroup>
          )}
        </select>
      </label>
      <button className={editing ? 'on' : undefined} aria-pressed={editing} disabled={viewing} title={viewing ? 'Go back to the current version to edit' : undefined} onClick={onEdit}>Edit <kbd>{/mac/i.test(navigator.platform) ? '⌘E' : 'Ctrl+E'}</kbd></button>
      <ExportMenu disabled={viewing} onExport={onExport} onPrint={onPrint} />
    </header>
  )
}
