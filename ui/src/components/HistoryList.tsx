import { useEffect, useMemo, useState } from 'react'
import { type Version, history } from '../api.ts'
import { Icon } from './Icon.tsx'
import { absoluteTime, groupVersions, relativeTime, useNow } from '../lib/history.ts'

const PAGE = 50

type Props = { name: string; reloadKey: number; viewing?: string; onView: (v?: Version) => void; onError: (e: unknown) => void }

export function HistoryList({ name, reloadKey, viewing, onView, onError }: Props) {
  const [versions, setVersions] = useState<Version[]>([])
  const [open, setOpen] = useState<Set<string>>(new Set())
  const [limit, setLimit] = useState(PAGE)
  const now = useNow()
  const groups = useMemo(() => groupVersions(versions), [versions])

  useEffect(() => {
    let stale = false
    history(name, limit).then((v) => !stale && setVersions(v), (e) => !stale && onError(e))
    return () => void (stale = true)
  }, [name, reloadKey, onError, limit])

  const pick = (v: Version) => onView(v.commit === versions[0]?.commit ? undefined : v) // newest = current
  const row = (v: Version, label: string, nested = false, on = (viewing ?? versions[0]?.commit) === v.commit) => (
    <button key={v.commit} className={`item history-item${nested ? ' nested-version' : ''}${on ? ' on' : ''}`} title={`${label}\n${absoluteTime(v.time_ms)}`} onClick={() => pick(v)}>
      <span className="label">{label}</span>
      <time className="when">{relativeTime(v.time_ms, now)}</time>
    </button>
  )

  return (
    <section className="history" aria-label="History">
      <h2>History</h2>
      {groups.length === 0 && <div className="empty">No history yet</div>}
      {groups.map((g) => {
        const key = g.newest.commit
        const many = g.versions.length > 1
        const expanded = open.has(key)
        return (
          <div key={key}>
            <div className="history-group">
              {many ? (
                <button
                  className="chevron"
                  aria-expanded={expanded}
                  aria-label={`${expanded ? 'Collapse' : 'Expand'} ${g.label}`}
                  onClick={() => setOpen((s) => { const n = new Set(s); if (!n.delete(key)) n.add(key); return n })}
                >
                  <Icon name={expanded ? 'chevronDown' : 'chevronRight'} size={12} />
                </button>
              ) : <span className="chevron" />}
              {row(g.newest, g.label, false, !(many && expanded) && g.versions.some((x) => x.commit === (viewing ?? versions[0]?.commit)))}
            </div>
            {many && expanded && g.versions.map((v) => row(v, v.message, true))}
          </div>
        )
      })}
      {versions.length === limit && <button className="item add" onClick={() => setLimit((l) => l + PAGE)}>Show older versions</button>}
    </section>
  )
}
