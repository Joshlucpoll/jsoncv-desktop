import { useEffect, useState } from 'react'
import type { Version } from '../api.ts'

export type VersionGroup = { author: string; label: string; newest: Version; versions: Version[] }

/** `versions` is newest first. A group chains same-author versions each within `windowMs` of the previous one. */
export function groupVersions(versions: Version[], windowMs = 300_000): VersionGroup[] {
  const groups: VersionGroup[] = []
  for (const v of versions) {
    const g = groups[groups.length - 1]
    const gap = g ? g.versions[g.versions.length - 1].time_ms - v.time_ms : -1 // negative = out of order: never merge
    if (g && g.author === v.author && gap >= 0 && gap <= windowMs) g.versions.push(v)
    else groups.push({ author: v.author, label: '', newest: v, versions: [v] })
  }
  for (const g of groups) g.label = `${g.author}: ${g.versions.length === 1 ? g.newest.message : groupSummary(g.versions)}`
  return groups
}

// "Edited work, skills" messages merge into one key list (first seen first); anything else falls back to a count.
function groupSummary(versions: Version[]): string {
  const keys: string[] = []
  for (const { message } of versions) {
    if (!message.startsWith('Edited ')) return `${versions.length} edits`
    for (const k of message.slice(7).split(', ')) if (!keys.includes(k)) keys.push(k)
  }
  return `edited ${keys.join(', ')}`
}

export function relativeTime(ms: number, now = Date.now(), locale?: string): string {
  const d = now - ms
  if (d < 60_000) return 'now'
  if (d < 3_600_000) return `${Math.floor(d / 60_000)}m`
  if (d < 86_400_000) return `${Math.floor(d / 3_600_000)}h`
  if (d < 7 * 86_400_000) return new Intl.DateTimeFormat(locale, { weekday: 'short' }).format(ms)
  return new Intl.DateTimeFormat(locale, { day: 'numeric', month: 'short' }).format(ms)
}

/** Re-renders every `ms` so relative labels stay fresh. */
export function useNow(ms = 60_000): number {
  const [now, setNow] = useState(Date.now)
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), ms)
    return () => clearInterval(t)
  }, [ms])
  return now
}
