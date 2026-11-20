import { useEffect, useRef, useState } from 'react'
import { type AgentId, DEFAULT_THEME, type CvInfo, type Editor, listEditors, type LibraryConfig, type LibraryStats, type ThemeInfo, connectAgent, errorMessage, libraryStats, reveal } from '../api.ts'
import { MOD, revealLabel } from '../lib/platform.ts'
import { Meander } from './Mosaic.tsx'
import { Icon, type IconName } from './Icon.tsx'
import { Logo, type LogoName } from './Logo.tsx'
import { NewTheme, ThemeRow } from './ThemesView.tsx'
import { useMenu } from '../lib/menu.ts'
import { openAction } from '../lib/themes.ts'

const OPEN_KEY = 'theme-open-with'

const copy = (text: string) => navigator.clipboard?.writeText(text)

export type SettingsSection = 'library' | 'themes' | 'agents'
const SECTIONS: { id: SettingsSection; label: string; icon: IconName }[] = [
  { id: 'library', label: 'CV folder', icon: 'folder' },
  { id: 'themes', label: 'Themes', icon: 'palette' },
  { id: 'agents', label: 'AI agents', icon: 'plug' },
]

// One-click setup only where the agent has a stable route; every other agent uses the setup prompt.
// Claude Code and Claude Desktop keep separate MCP settings, so Claude's Add button asks which.
type Target = { id: AgentId; label: string; note: string }
const AGENTS: { label: string; how: string; logo: LogoName; targets: Target[] }[] = [
  { label: 'Claude', how: 'Claude Code or the Claude Desktop chat app', logo: 'claude', targets: [
    { id: 'claude-code', label: 'Claude Code', note: 'Terminal and the Code tab' },
    { id: 'claude-desktop', label: 'Claude Desktop', note: 'Chat and the Code tab' },
  ] },
  { label: 'Codex', how: 'Runs codex mcp add', logo: 'openai', targets: [{ id: 'codex', label: 'Codex', note: '' }] },
]
const PROMPT_AGENTS: [LogoName, string][] = [['cursor', 'Cursor'], ['vscode', 'VS Code'], ['antigravity', 'Antigravity']]

const bytes = (n: number) => {
  const units = ['B', 'KB', 'MB', 'GB']
  const i = Math.min(units.length - 1, Math.floor(Math.log(Math.max(n, 1)) / Math.log(1024)))
  return `${(n / 1024 ** i).toFixed(i && n < 10 * 1024 ** i ? 1 : 0)} ${units[i]}`
}

/** For any other agent: it works out where its own MCP settings live. */
const setupPrompt = (binary: string) => `Please add an MCP server to your configuration so you can read and edit my CVs.

- Name: jsoncv
- Transport: stdio (a local command)
- Command: ${binary}
- Arguments: ["mcp"]

It comes from the jsoncv-desktop app. Add it at user (global) scope if you can, so it works in every project. Find where your MCP servers are configured, add this entry without changing any other server, then tell me whether I need to restart you.`

export function Copy({ text, label = 'Copy' }: { text: string; label?: string }) {
  const [done, setDone] = useState(false)
  useEffect(() => {
    if (!done) return
    const t = setTimeout(() => setDone(false), 1500)
    return () => clearTimeout(t)
  }, [done])
  return <button onClick={() => copy(text)?.then(() => setDone(true), () => {})}><Icon name={done ? 'check' : 'copy'} size={12} /> {done ? 'Copied' : label}</button>
}

type AgentState = { busy?: boolean; ok?: string; err?: string }

function AddButton({ st, onClick }: { st?: AgentState; onClick: () => void }) {
  return (
    <button className="primary" disabled={st?.busy} onClick={onClick}>
      {st?.busy ? <span className="spinner" aria-hidden="true" /> : st?.ok ? <Icon name="check" size={12} /> : null}
      {st?.ok ? 'Added' : 'Add'}
    </button>
  )
}

/** Add button for a brand with several apps: a menu picks which one. */
function AddMenu({ targets, state, onPick }: { targets: Target[]; state: Partial<Record<AgentId, AgentState>>; onPick: (id: AgentId) => void }) {
  const { open, setOpen, box, onKeyDown } = useMenu()
  const busy = targets.some((t) => state[t.id]?.busy)
  return (
    // Esc closes only the menu, not the Settings dialog around it.
    <div className="popup" ref={box} onKeyDown={(e) => { if (open && e.key === 'Escape') e.stopPropagation(); onKeyDown(e) }}>
      <button className="trigger primary" aria-haspopup="menu" aria-expanded={open} disabled={busy} onClick={() => setOpen((o) => !o)}>
        {busy && <span className="spinner" aria-hidden="true" />} Add <Icon name="chevronDown" size={12} />
      </button>
      {open && (
        <div className="menu add-menu" role="menu">
          {targets.map((t) => (
            <button key={t.id} role="menuitem" onClick={() => { setOpen(false); onPick(t.id) }}>
              <span className="add-option"><span>{t.label}</span><span className="opt-note">{t.note}</span></span>
              {state[t.id]?.ok && <Icon name="check" size={12} />}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

const Note = ({ st, how }: { st?: AgentState; how: string }) => (
  <span className={`agent-note${st?.err ? ' err' : st?.ok ? ' ok' : ''}`} role={st?.err ? 'alert' : undefined}>{st?.ok ?? st?.err ?? how}</span>
)

function Agents({ binary }: { binary: string }) {
  const [state, setState] = useState<Partial<Record<AgentId, AgentState>>>({})
  const [last, setLast] = useState<AgentId>() // the target whose result a multi-app row shows
  const connect = async (id: AgentId) => {
    setLast(id)
    setState((s) => ({ ...s, [id]: { busy: true } }))
    try {
      const ok = await connectAgent(id)
      setState((s) => ({ ...s, [id]: { ok } }))
    } catch (e) {
      setState((s) => ({ ...s, [id]: { err: errorMessage(e).replace(/^not found: /, "Couldn't find ") } }))
    }
  }
  const json = JSON.stringify({ mcpServers: { jsoncv: { command: binary, args: ['mcp'] } } }, null, 2)
  return (
    <>
      <p className="muted lead">Let a coding agent read and edit your CVs over MCP. Its edits show up here at once, and {MOD}Z undoes them.</p>
      <h3>One-click setup</h3>
      <div className="agent-list">
        {AGENTS.map((a) => {
          const single = a.targets.length === 1 ? a.targets[0] : undefined
          const shown = single ?? a.targets.find((t) => t.id === last)
          const st = shown && state[shown.id]
          return (
            <div key={a.label} className="agent-row">
              <span className="agent-logo"><Logo name={a.logo} size={20} /></span>
              <div className="agent-text">
                <strong>{a.label}</strong>
                <Note st={st?.busy ? undefined : st} how={a.how} />
              </div>
              {single
                ? <AddButton st={state[single.id]} onClick={() => void connect(single.id)} />
                : <AddMenu targets={a.targets} state={state} onPick={(id) => void connect(id)} />}
            </div>
          )
        })}
      </div>
      <h3>Other agents</h3>
      <div className="agent-list other-agent">
        <div className="setting-label">
          <strong>Any agent that can edit its own settings</strong>
          <span className="muted">Paste it into your agent and it sets itself up.</span>
          <span className="works-with">
            {PROMPT_AGENTS.map(([logo, label]) => <span key={logo}><Logo name={logo} size={13} /> {label}</span>)}
            <span>and others</span>
          </span>
        </div>
        <Copy text={setupPrompt(binary)} label="Copy setup prompt" />
      </div>
      <details className="manual">
        <summary>Manual setup</summary>
        <div className="snippet">
          <div className="snippet-head"><span>Command</span><Copy text={`"${binary}" mcp`} /></div>
          <pre>{`"${binary}" mcp`}</pre>
        </div>
        <div className="snippet">
          <div className="snippet-head"><span>JSON config</span><Copy text={json} /></div>
          <pre>{json}</pre>
        </div>
      </details>
    </>
  )
}

function Library({ config, cvs, themes, onChange, onError }: { config?: LibraryConfig; cvs: CvInfo[]; themes: ThemeInfo[]; onChange: () => void; onError: (e: unknown) => void }) {
  const [stats, setStats] = useState<LibraryStats>()
  const lib = config?.library
  // Re-measure when the folder or its CVs change.
  useEffect(() => { if (lib) libraryStats().then(setStats, () => setStats(undefined)) }, [lib, cvs])
  const folder = lib?.split(/[\\/]/).filter(Boolean).pop()
  const folders = new Set(cvs.map((c) => c.folder).filter(Boolean)).size
  const custom = themes.filter((t) => !t.builtin).length
  const tiles: [string, string, string?][] = [
    ['CVs', String(cvs.length), folders ? `In ${folders} folder${folders === 1 ? '' : 's'}` : undefined],
    ['Custom themes', String(custom), `${themes.length - custom} built in`],
    ['Assets', stats ? String(stats.assets) : '–', 'Photos and files'],
    ['Size on disk', stats ? bytes(stats.bytes) : '–', 'The whole folder'],
    ['Undo history', stats ? bytes(stats.history_bytes) : '–', 'Kept on this Mac'],
  ]
  return (
    <>
      <p className="muted lead">Where your CVs, themes and assets live. Any folder works, including iCloud Drive or Dropbox.</p>
      <div className="lib-card">
        <span className="lib-icon"><Icon name="folder" size={22} /></span>
        <div className="lib-text">
          <strong>{folder ?? 'Unknown folder'}</strong>
          <span className="muted" title={lib}>{lib ?? 'Could not load settings'}</span>
        </div>
        {lib && <button onClick={() => reveal(lib).catch(onError)}>{revealLabel}</button>}
        <button onClick={onChange}>Change…</button>
      </div>
      <div className="stats">
        {tiles.map(([label, value, note]) => (
          <div key={label} className="stat">
            <span className="stat-label">{label}</span>
            <span className="stat-value">{value}</span>
            {note && <span className="stat-note">{note}</span>}
          </div>
        ))}
      </div>
    </>
  )
}

function Themes({ cvs, themes, reloadKey, onThemesChanged, onGuide, onError }: { cvs: CvInfo[]; themes: ThemeInfo[]; reloadKey: number; onThemesChanged: () => void; onGuide: () => void; onError: (e: unknown) => void }) {
  const [newFrom, setNewFrom] = useState<string>() // undefined = dialog closed
  const [editors, setEditors] = useState<Editor[]>([])
  const [openWith, setOpenWith] = useState('reveal')
  useEffect(() => {
    let live = true
    listEditors().then((eds) => {
      if (!live) return
      let stored: string | null = null
      try { stored = localStorage.getItem(OPEN_KEY) } catch { /* storage unavailable: start from Reveal */ }
      setEditors(eds)
      setOpenWith(openAction(stored, eds))
    }, onError)
    return () => { live = false }
  }, [onError])
  const pickOpen = (a: string) => {
    setOpenWith(a)
    try { localStorage.setItem(OPEN_KEY, a) } catch { /* per-machine convenience only */ }
  }
  return (
    <>
      <p className="muted lead">Themes are files: a template and a stylesheet. The easiest way to make one is to ask your agent. You can also open a theme in your editor.</p>
      <div className="theme-actions">
        <button onClick={onGuide}>How do I create a theme?</button>
        <button className="primary" onClick={() => setNewFrom(DEFAULT_THEME)}><Icon name="plus" /> New theme…</button>
      </div>
      {newFrom !== undefined && (
        <NewTheme key={newFrom} themes={themes} from={newFrom} onError={onError} onCancel={() => setNewFrom(undefined)}
          onCreated={() => { setNewFrom(undefined); onThemesChanged() }} />
      )}
      <div className="theme-rows">
        {themes.map((t) => (
          <ThemeRow key={t.id} theme={t} reloadKey={reloadKey} editors={editors} openWith={openWith} onOpenWith={pickOpen} usedBy={cvs.filter((c) => c.theme === t.id).map((c) => c.name)}
            onDuplicate={() => setNewFrom(t.id)} onDeleted={onThemesChanged} onError={onError} />
        ))}
      </div>
    </>
  )
}

type Props = {
  cvs: CvInfo[]; config?: LibraryConfig; binary: string; themes: ThemeInfo[]; reloadKey: number; section?: SettingsSection; onSection: (s: SettingsSection) => void
  onChange: () => void; onThemesChanged: () => void; onGuide: () => void; onError: (e: unknown) => void; onClose: () => void
}

export function Settings({ cvs, config, binary, themes, reloadKey, section = 'library', onSection, onChange, onThemesChanged, onGuide, onError, onClose }: Props) {
  const done = useRef<HTMLButtonElement>(null)
  useEffect(() => { done.current?.focus() }, [])
  const current = SECTIONS.find((s) => s.id === section)!
  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()} onKeyDown={(e) => e.key === 'Escape' && onClose()}>
      <div className="dialog settings" role="dialog" aria-modal="true" aria-label="Settings">
        <nav className="settings-nav" aria-label="Settings sections">
          <div className="settings-title">Settings</div>
          <div className="frieze"><Meander periods={5} /></div>
          {SECTIONS.map((s) => (
            <button key={s.id} className={`settings-tab${s.id === section ? ' on' : ''}`} aria-current={s.id === section ? 'page' : undefined} onClick={() => onSection(s.id)}>
              <Icon name={s.icon} /> {s.label}
            </button>
          ))}
        </nav>
        <div className="settings-body">
          <header><h2>{current.label}</h2><span className="spacer" /><button ref={done} className="icon-btn close" aria-label="Close" title="Close (Esc)" onClick={onClose}><Icon name="x" size={16} /></button></header>
          {section === 'library' && <Library config={config} cvs={cvs} themes={themes} onChange={onChange} onError={onError} />}
          {section === 'themes' && <Themes cvs={cvs} themes={themes} reloadKey={reloadKey} onThemesChanged={onThemesChanged} onGuide={onGuide} onError={onError} />}
          {section === 'agents' && <Agents binary={binary} />}
        </div>
      </div>
    </div>
  )
}
