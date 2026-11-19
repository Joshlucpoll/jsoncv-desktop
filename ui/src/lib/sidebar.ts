import type { CvInfo } from '../api.ts'

const byName = (a: CvInfo, b: CvInfo) => a.name.localeCompare(b.name)

/** Sidebar sections: pinned CVs (pin order), then folders (empty ones too), then top-level CVs. A pinned CV also stays in its place. */
export function groupCvs(cvs: CvInfo[], folders: string[]) {
  const sorted = [...cvs].sort(byName)
  return {
    pinned: cvs.filter((c) => c.pin !== null).sort((a, b) => a.pin! - b.pin!),
    folders: [...folders].sort().map((name) => ({ name, cvs: sorted.filter((c) => c.folder === name) })),
    loose: sorted.filter((c) => c.folder === null),
  }
}

/** The CV to open when none is selected: the most recently modified one. */
export const pickDefault = (cvs: CvInfo[]): string | undefined =>
  cvs.reduce<CvInfo | undefined>((best, c) => (!best || c.modified_ms > best.modified_ms ? c : best), undefined)?.name
