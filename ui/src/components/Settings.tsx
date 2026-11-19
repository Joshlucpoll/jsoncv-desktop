import { useEffect, useRef, useState } from 'react'
import { type AgentId, type LibraryConfig, type ThemeInfo, connectAgent, errorMessage, reveal } from '../api.ts'
import { MOD, revealLabel } from '../lib/platform.ts'
import { Icon, type IconName } from './Icon.tsx'
import { ThemeRow } from './ThemesView.tsx'

const copy = (text: string) => navigator.clipboard?.writeText(text)

export type SettingsSection = 'library' | 'themes' | 'agents'
const SECTIONS: { id: SettingsSection; label: string; icon: IconName }[] = [
  { id: 'library', label: 'CV folder', icon: 'folder' },
  { id: 'themes', label: 'Themes', icon: 'palette' },
  { id: 'agents', label: 'AI agents', icon: 'plug' },
]

const AGENTS: { id: AgentId; label: string; how: string }[] = [
  { id: 'claude-code', label: 'Claude Code', how: 'Runs claude mcp add' },
  { id: 'claude-desktop', label: 'Claude Desktop', how: 'Edits its config file' },
  { id: 'codex', label: 'Codex', how: 'Runs codex mcp add' },
  { id: 'cursor', label: 'Cursor', how: 'Edits ~/.cursor/mcp.json' },
  { id: 'vscode', label: 'VS Code', how: 'Edits its user mcp.json' },
  { id: 'gemini', label: 'Gemini CLI', how: 'Edits ~/.gemini/settings.json' },
  { id: 'windsurf', label: 'Windsurf', how: 'Edits its mcp_config.json' },
]

/** For any other agent: it works out where its own MCP settings live. */
const setupPrompt = (binary: string) => `Please add an MCP server to your configuration so you can read and edit my CVs.

- Name: jsoncv
- Transport: stdio (a local command)
- Command: ${binary}
- Arguments: ["mcp"]

It comes from the jsoncv-desktop app. Add it at user (global) scope if you can, so it works in every project. Find where your MCP servers are configured, add this entry without changing any other server, then tell me whether I need to restart you.`

function Copy({ text, label = 'Copy' }: { text: string; label?: string }) {
  const [done, setDone] = useState(false)
  useEffect(() => {
    if (!done) return
    const t = setTimeout(() => setDone(false), 1500)
    return () => clearTimeout(t)
  }, [done])
  return <button onClick={() => copy(text)?.then(() => setDone(true), () => {})}><Icon name={done ? 'check' : 'copy'} size={12} /> {done ? 'Copied' : label}</button>
}

function Agents({ binary }: { binary: string }) {
  const [state, setState] = useState<Partial<Record<AgentId, { busy?: boolean; ok?: string; err?: string }>>>({})
  const connect = async (id: AgentId) => {
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
      <div className="agent-grid">
        {AGENTS.map((a) => {
          const st = state[a.id]
          return (
            <div key={a.id} className={`agent${st?.ok ? ' ok' : st?.err ? ' err' : ''}`}>
              <div className="agent-head">
                <strong>{a.label}</strong>
                <span className="spacer" />
                <button className="primary" disabled={st?.busy} onClick={() => void connect(a.id)}>
                  {st?.busy ? <span className="spinner" aria-hidden="true" /> : st?.ok ? <Icon name="check" size={12} /> : null}
                  {st?.ok ? 'Added' : 'Add'}
                </button>
              </div>
              <span className="agent-note" role={st?.err ? 'alert' : undefined}>{st?.ok ?? st?.err ?? a.how}</span>
            </div>
          )
        })}
      </div>
      <div className="setting other-agent">
        <div className="setting-label">
          <strong>Another agent</strong>
          <span className="muted">Copy a prompt, paste it into any agent, and it sets itself up.</span>
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

type Props = {
  config?: LibraryConfig; binary: string; themes: ThemeInfo[]; reloadKey: number; section?: SettingsSection
  onChange: () => void; onThemesChanged: () => void; onError: (e: unknown) => void; onClose: () => void
}

export function Settings({ config, binary, themes, reloadKey, section: initial = 'library', onChange, onThemesChanged, onError, onClose }: Props) {
  const [section, setSection] = useState<SettingsSection>(initial)
  const done = useRef<HTMLButtonElement>(null)
  useEffect(() => { done.current?.focus() }, [])
  const current = SECTIONS.find((s) => s.id === section)!
  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()} onKeyDown={(e) => e.key === 'Escape' && onClose()}>
      <div className="dialog settings" role="dialog" aria-modal="true" aria-label="Settings">
        <nav className="settings-nav" aria-label="Settings sections">
          <div className="settings-title">Settings</div>
          {SECTIONS.map((s) => (
            <button key={s.id} className={`settings-tab${s.id === section ? ' on' : ''}`} aria-current={s.id === section ? 'page' : undefined} onClick={() => setSection(s.id)}>
              <Icon name={s.icon} /> {s.label}
            </button>
          ))}
        </nav>
        <div className="settings-body">
          <header><h2>{current.label}</h2><span className="spacer" /><button ref={done} onClick={onClose}>Done</button></header>
          {section === 'library' && (
            <div className="setting">
              <div className="setting-label">
                <strong>Location</strong>
                <span className="muted">Where your CVs, themes and assets live. Any folder works, including iCloud Drive or Dropbox. Undo history stays on this Mac.</span>
              </div>
              <div className="lib-row">
                <code title={config?.library}>{config?.library ?? 'Unknown (could not load settings)'}</code>
                {config?.library && <button onClick={() => reveal(config.library).catch(onError)}>{revealLabel}</button>}
                <button onClick={onChange}>Change…</button>
              </div>
            </div>
          )}
          {section === 'themes' && (
            <>
              <p className="muted lead">Pick a theme per CV from the toolbar. Duplicate one to make it your own; its files open in any editor.</p>
              <div className="theme-rows">
                {themes.map((t) => <ThemeRow key={t.id} theme={t} reloadKey={reloadKey} onDuplicated={onThemesChanged} onError={onError} />)}
              </div>
            </>
          )}
          {section === 'agents' && <Agents binary={binary} />}
        </div>
      </div>
    </div>
  )
}
