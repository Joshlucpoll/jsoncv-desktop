import { DEFAULT_THEME, type ThemeInfo } from '../api.ts'

type Props = { name: string; theme: string | null; themes: ThemeInfo[]; onTheme: (id: string) => void; editing: boolean; onEdit: () => void }

const title = (name: string) => (name === 'main' ? 'Main CV' : name.replace(/^tailored\//, ''))

export function Toolbar({ name, theme, themes, onTheme, editing, onEdit }: Props) {
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
      <button className={editing ? 'on' : undefined} aria-pressed={editing} onClick={onEdit}>Edit <kbd>{/mac/i.test(navigator.platform) ? '⌘E' : 'Ctrl+E'}</kbd></button>
      {/* Export is enabled in Task 15. */}
      <button disabled title="Coming soon">Export ▾</button>
    </header>
  )
}
