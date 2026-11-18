import { useEffect, useRef } from 'react'
import { type LibraryConfig } from '../api.ts'

const copy = (text: string) => navigator.clipboard?.writeText(text).catch(() => {})

function Snippet({ label, text }: { label: string; text: string }) {
  return (
    <div className="snippet">
      <div className="snippet-head"><span>{label}</span><button onClick={() => void copy(text)}>Copy</button></div>
      <pre>{text}</pre>
    </div>
  )
}

type Props = { config?: LibraryConfig; binary: string; onChange: () => void; onClose: () => void }

export function Settings({ config, binary, onChange, onClose }: Props) {
  const ref = useRef<HTMLButtonElement>(null)
  useEffect(() => ref.current?.focus(), [])
  const desktop = JSON.stringify({ mcpServers: { jsoncv: { command: binary, args: ['mcp'] } } }, null, 2)
  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()} onKeyDown={(e) => e.key === 'Escape' && onClose()}>
      <div className="dialog settings" role="dialog" aria-modal="true" aria-label="Settings">
        <header><strong>Settings</strong><span className="spacer" /><button ref={ref} onClick={onClose}>Done</button></header>
        <h3>Library</h3>
        <div className="lib-row"><code>{config?.library ?? 'Unknown (could not load settings)'}</code><button onClick={onChange}>Change…</button></div>
        {config?.inSyncFolder && (
          <p className="banner-inline" role="alert">This folder is synced by iCloud or Dropbox. Syncing can corrupt the CV history. Pick a folder outside synced storage.</p>
        )}
        <h3>Connect an AI agent (MCP)</h3>
        <Snippet label="Claude Code" text={`claude mcp add jsoncv -- "${binary}" mcp`} />
        <Snippet label="Claude Desktop" text={desktop} />
      </div>
    </div>
  )
}
