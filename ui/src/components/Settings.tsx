import { useEffect, useRef } from 'react'
import { type LibraryConfig, type ThemeInfo, reveal } from '../api.ts'
import { MOD, revealLabel } from '../lib/platform.ts'
import { ThemeRow } from './ThemesView.tsx'

const copy = (text: string) => navigator.clipboard?.writeText(text).catch(() => {})

function Snippet({ label, text }: { label: string; text: string }) {
  return (
    <div className="snippet">
      <div className="snippet-head"><span>{label}</span><button onClick={() => void copy(text)}>Copy</button></div>
      <pre>{text}</pre>
    </div>
  )
}

function PathRow({ label, help, path, onChange, onError }: { label: string; help: string; path?: string; onChange: () => void; onError: (e: unknown) => void }) {
  return (
    <div className="setting">
      <div className="setting-label"><strong>{label}</strong><span className="muted">{help}</span></div>
      <div className="lib-row">
        <code title={path}>{path ?? 'Unknown (could not load settings)'}</code>
        {path && <button onClick={() => reveal(path).catch(onError)}>{revealLabel}</button>}
        <button onClick={onChange}>Change…</button>
      </div>
    </div>
  )
}

type Props = {
  config?: LibraryConfig; binary: string; themes: ThemeInfo[]; reloadKey: number; focusThemes?: boolean
  onChange: () => void; onChangeHistory: () => void; onThemesChanged: () => void; onError: (e: unknown) => void; onClose: () => void
}

export function Settings({ config, binary, themes, reloadKey, focusThemes, onChange, onChangeHistory, onThemesChanged, onError, onClose }: Props) {
  const ref = useRef<HTMLButtonElement>(null)
  const themesRef = useRef<HTMLHeadingElement>(null)
  useEffect(() => {
    ref.current?.focus()
    if (focusThemes) themesRef.current?.scrollIntoView({ block: 'start' })
  }, [focusThemes])
  const desktop = JSON.stringify({ mcpServers: { jsoncv: { command: binary, args: ['mcp'] } } }, null, 2)
  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()} onKeyDown={(e) => e.key === 'Escape' && onClose()}>
      <div className="dialog settings" role="dialog" aria-modal="true" aria-label="Settings">
        <header><strong>Settings</strong><span className="spacer" /><button ref={ref} onClick={onClose}>Done</button></header>

        <h3>Storage</h3>
        <PathRow
          label="CV folder" path={config?.library} onChange={onChange} onError={onError}
          help="Where your CVs, themes and assets live. Any folder works, including iCloud Drive or Dropbox."
        />
        <PathRow
          label="Undo history" path={config?.history} onChange={onChangeHistory} onError={onError}
          help={`Past versions, so ${MOD}Z and ${MOD}⇧Z can undo and redo changes by you or an agent.`}
        />
        {config?.historyInSyncFolder && (
          <p className="banner-inline" role="alert">This file is in iCloud or Dropbox. Syncing can corrupt the history if two Macs write to it.</p>
        )}

        <h3 ref={themesRef}>Themes</h3>
        <div className="theme-rows">
          {themes.map((t) => <ThemeRow key={t.id} theme={t} reloadKey={reloadKey} onDuplicated={onThemesChanged} onError={onError} />)}
        </div>

        <h3>Connect an AI agent (MCP)</h3>
        <Snippet label="Claude Code" text={`claude mcp add jsoncv -- "${binary}" mcp`} />
        <Snippet label="Claude Desktop" text={desktop} />
      </div>
    </div>
  )
}
