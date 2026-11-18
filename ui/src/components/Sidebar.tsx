import type { CvInfo } from '../api.ts'
import { CvList } from './CvList.tsx'

type Props = { cvs: CvInfo[]; selected: string; onSelect: (n: string) => void; onChanged: () => void; onError: (e: unknown) => void }

export function Sidebar(p: Props) {
  return (
    <aside className="sidebar">
      <CvList {...p} />
      {/* History list: Task 14. */}
      <div className="history-slot" />
    </aside>
  )
}
