import { useEffect, useState } from 'react'
import type { Toast } from '../lib/toasts.ts'
import { Icon } from './Icon.tsx'

function Item({ t, dismiss }: { t: Toast; dismiss: (id: number) => void }) {
  const [hover, setHover] = useState(false)
  useEffect(() => {
    if (!t.timeout || hover) return
    const h = setTimeout(() => dismiss(t.id), t.timeout)
    return () => clearTimeout(h)
  }, [t.id, t.timeout, hover, dismiss])
  return (
    <div className={`toast ${t.kind ?? 'ok'}`} role={t.kind === 'error' ? 'alert' : 'status'} onMouseEnter={() => setHover(true)} onMouseLeave={() => setHover(false)}>
      {t.kind && t.kind !== 'ok' && <Icon name="warn" />}
      <span className="toast-text">{t.text}</span>
      {t.actions?.map((a) => (
        <button key={a.label} className="link" onClick={() => { dismiss(t.id); a.run() }}>{a.label}</button>
      ))}
      <button className="icon-btn" aria-label="Dismiss" title="Dismiss" onClick={() => dismiss(t.id)}><Icon name="x" size={12} /></button>
    </div>
  )
}

export function Toasts({ list, dismiss }: { list: Toast[]; dismiss: (id: number) => void }) {
  return (
    <div className="toasts" aria-live="polite">
      {list.map((t) => <Item key={t.id} t={t} dismiss={dismiss} />)}
    </div>
  )
}
