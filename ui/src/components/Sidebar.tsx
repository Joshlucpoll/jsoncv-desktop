import type { CvInfo, Version } from '../api.ts'
import { CvList } from './CvList.tsx'
import { HistoryList } from './HistoryList.tsx'

type Props = {
  cvs: CvInfo[]; selected: string; onSelect: (n: string) => void; onChanged: () => Promise<unknown>; onError: (e: unknown) => void
  reloadKey: number; viewing?: string; onView: (v?: Version) => void
}

export function Sidebar({ reloadKey, viewing, onView, ...p }: Props) {
  return (
    <aside className="sidebar">
      <CvList {...p} />
      <HistoryList name={p.selected} reloadKey={reloadKey} viewing={viewing} onView={onView} onError={p.onError} />
    </aside>
  )
}
