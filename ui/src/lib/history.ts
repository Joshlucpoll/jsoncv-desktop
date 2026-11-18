import type { Version } from '../api.ts'

export type VersionGroup = { author: string; label: string; newest: Version; versions: Version[] }

/** `versions` is newest first. A group chains same-author versions each within `windowMs` of the previous one. */
export function groupVersions(versions: Version[], windowMs = 300_000): VersionGroup[] {
  const groups: VersionGroup[] = []
  for (const v of versions) {
    const g = groups[groups.length - 1]
    if (g && g.author === v.author && g.versions[g.versions.length - 1].time_ms - v.time_ms <= windowMs) g.versions.push(v)
    else groups.push({ author: v.author, label: '', newest: v, versions: [v] })
  }
  for (const g of groups) g.label = g.versions.length === 1 ? `${g.author}: ${g.newest.message}` : `${g.author}: ${g.versions.length} edits`
  return groups
}

export function relativeTime(ms: number, now = Date.now(), locale?: string): string {
  const d = now - ms
  if (d < 60_000) return 'now'
  if (d < 3_600_000) return `${Math.floor(d / 60_000)}m`
  if (d < 86_400_000) return `${Math.floor(d / 3_600_000)}h`
  if (d < 7 * 86_400_000) return new Intl.DateTimeFormat(locale, { weekday: 'short' }).format(ms)
  return new Intl.DateTimeFormat(locale, { day: 'numeric', month: 'short' }).format(ms)
}
