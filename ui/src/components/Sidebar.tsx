import type { CvInfo } from '../api.ts'
import { CvList } from './CvList.tsx'
import { Icon } from './Icon.tsx'
import { Meander } from './Mosaic.tsx'
import { MOD } from '../lib/platform.ts'

type Props = {
  cvs: CvInfo[]; folders: string[]; loaded: boolean; selected: string; onSelect: (n: string) => void; onChanged: () => Promise<unknown>; onLeaveSelected: () => Promise<boolean>; onError: (e: unknown) => void
  onSettings: () => void; newSignal: number
}

export function Sidebar({ onSettings, ...p }: Props) {
  return (
    <aside className="sidebar">
      {/* On macOS the window controls sit in this header (overlay title bar); it drags the window. */}
      <header className="sidebar-head" data-tauri-drag-region>
        <span className="wordmark">jsoncv<span>-desktop</span></span>
      </header>
      <div className="frieze"><Meander periods={7} /></div>
      <CvList {...p} />
      <div className="sidebar-foot">
        <button className="gear" title={`Settings (${MOD},)`} onClick={onSettings}><Icon name="gear" /> Settings</button>
      </div>
    </aside>
  )
}
