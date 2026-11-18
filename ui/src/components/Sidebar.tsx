import type { CvInfo, ThemeInfo, Version } from '../api.ts'
import { CvList } from './CvList.tsx'
import { HistoryList } from './HistoryList.tsx'
import { ThemeList } from './ThemesView.tsx'

export type View = 'cvs' | 'themes'

type Props = {
  cvs: CvInfo[]; selected: string; onSelect: (n: string) => void; onChanged: () => Promise<unknown>; onLeaveSelected: () => Promise<boolean>; onError: (e: unknown) => void
  reloadKey: number; viewing?: string; onView: (v?: Version) => void
  view: View; onViewMode: (v: View) => void
  themes: ThemeInfo[]; theme?: string; onTheme: (id: string) => void; onSettings: () => void
}

export function Sidebar({ reloadKey, viewing, onView, view, onViewMode, themes, theme, onTheme, onSettings, ...p }: Props) {
  return (
    <aside className="sidebar">
      <div className="seg" role="tablist" aria-label="Sidebar view">
        {(['cvs', 'themes'] as const).map((v) => (
          <button key={v} role="tab" aria-selected={view === v} onClick={() => onViewMode(v)}>{v === 'cvs' ? 'CVs' : 'Themes'}</button>
        ))}
      </div>
      {view === 'cvs' ? (
        <>
          <CvList {...p} />
          <HistoryList key={p.selected} name={p.selected} reloadKey={reloadKey} viewing={viewing} onView={onView} onError={p.onError} />
        </>
      ) : (
        <ThemeList themes={themes} selected={theme} onSelect={onTheme} />
      )}
      <div className="sidebar-foot">
        <button className="gear" aria-label="Settings" title="Settings" onClick={onSettings}>⚙ Settings</button>
      </div>
    </aside>
  )
}
