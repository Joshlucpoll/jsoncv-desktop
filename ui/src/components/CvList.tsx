import { useEffect, useRef, useState } from 'react'
import { confirm } from '@tauri-apps/plugin-dialog'
import { type CvInfo, MOCK, createCv, createFolder, deleteCv, deleteFolder, duplicateCv, errorMessage, moveCv, renameCv, renameFolder, setPinned } from '../api.ts'
import { groupCvs } from '../lib/sidebar.ts'
import { MOD } from '../lib/platform.ts'
import { Icon } from './Icon.tsx'

type Props = {
  cvs: CvInfo[]; folders: string[]; selected: string; onSelect: (n: string) => void; onChanged: () => Promise<unknown>; onError: (e: unknown) => void
  /** False until the first list arrives, so the empty state does not flash at startup. */
  loaded: boolean
  /** Runs the edit panel's leave guard before the selected CV is renamed or deleted; false cancels. */
  onLeaveSelected: () => Promise<boolean>
  /** Increments to duplicate the open CV (the ⌘N shortcut). */
  newSignal: number
}

const SLUG = /^[a-z0-9][a-z0-9-]{0,63}$/
const COLLAPSED_KEY = 'jsoncv.collapsedFolders'

function readCollapsed(): Set<string> {
  try {
    return new Set(JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? '[]') as string[])
  } catch {
    return new Set()
  }
}

function SlugInput({ initial, label, placeholder = 'name, e.g. stripe', nested, onSubmit, onCancel }: { initial: string; label: string; placeholder?: string; nested?: boolean; onSubmit: (s: string) => Promise<void>; onCancel: () => void }) {
  const [v, setV] = useState(initial)
  const [err, setErr] = useState<string>()
  const busy = useRef(false)
  const submit = async () => {
    if (busy.current) return
    if (!SLUG.test(v)) return setErr('Use a-z, 0-9 and "-"; start with a letter or digit (max 64).')
    busy.current = true
    try {
      await onSubmit(v)
    } catch (e) {
      setErr(errorMessage(e))
    } finally {
      busy.current = false
    }
  }
  return (
    <div className={`slug-form${nested ? ' nested' : ''}`}>
      <input
        autoFocus
        value={v}
        aria-label={label}
        placeholder={placeholder}
        onChange={(e) => { setV(e.target.value); setErr(undefined) }}
        onFocus={(e) => e.target.select()}
        onKeyDown={(e) => { if (e.key === 'Enter') void submit(); if (e.key === 'Escape') onCancel() }}
        // Like a Finder rename: clicking away keeps a changed name, and cancels an unchanged or empty one.
        onBlur={() => (v.trim() === '' || v === initial ? onCancel() : void submit())}
      />
      {err && <div className="field-error">{err}</div>}
    </div>
  )
}

type Menu = { x: number; y: number } & ({ kind: 'cv'; cv: CvInfo } | { kind: 'folder'; name: string; empty: boolean } | { kind: 'new' })
type Creating = { kind: 'duplicate'; from: string } | { kind: 'blank' } | { kind: 'folder' }

export function CvList({ cvs, folders, loaded, selected, onSelect, onChanged, onError, onLeaveSelected, newSignal }: Props) {
  const [menu, setMenu] = useState<Menu>()
  const [creating, setCreating] = useState<Creating>()
  const [renaming, setRenaming] = useState<{ kind: 'cv' | 'folder'; name: string }>()
  const [collapsed, setCollapsed] = useState(readCollapsed)
  const [seenSignal, setSeenSignal] = useState(newSignal)
  if (newSignal !== seenSignal) {
    setSeenSignal(newSignal)
    if (selected) setCreating({ kind: 'duplicate', from: selected })
  }

  const toggleFolder = (name: string) => {
    const next = new Set(collapsed)
    if (!next.delete(name)) next.add(name)
    setCollapsed(next)
    try { localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...next])) } catch { /* per-viewer convenience only */ }
  }

  const menuEl = useRef<HTMLDivElement>(null)
  const returnTo = useRef<HTMLElement | null>(null)
  const closeMenu = (refocus: boolean) => {
    setMenu(undefined)
    if (refocus) returnTo.current?.focus()
  }
  const openMenu = (el: HTMLElement, m: Menu) => {
    returnTo.current = el
    setMenu(m)
  }
  const at = (el: HTMLElement) => { const r = el.getBoundingClientRect(); return { x: r.left, y: r.bottom + 2 } }

  useEffect(() => {
    if (!menu) return
    menuEl.current?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus()
    const close = () => setMenu(undefined)
    window.addEventListener('click', close)
    window.addEventListener('blur', close)
    return () => {
      window.removeEventListener('click', close)
      window.removeEventListener('blur', close)
    }
  }, [menu])

  const onMenuKey = (e: React.KeyboardEvent) => {
    const items = [...menuEl.current!.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')]
    const i = items.indexOf(document.activeElement as HTMLButtonElement)
    if (e.key === 'Escape') closeMenu(true)
    else if (e.key === 'Tab') { e.preventDefault(); closeMenu(true) }
    else if (e.key === 'ArrowDown') { e.preventDefault(); items[(i + 1) % items.length].focus() }
    else if (e.key === 'ArrowUp') { e.preventDefault(); items[(i - 1 + items.length) % items.length].focus() }
  }

  // Await the list reload first so the new name exists when it is selected.
  const select = async (name: string) => {
    await onChanged()
    onSelect(name)
  }
  const run = (p: Promise<unknown>) => void p.then(onChanged).catch(onError)

  const g = groupCvs(cvs, folders)

  const row = (c: CvInfo, nested: boolean, key: string) =>
    renaming?.kind === 'cv' && renaming.name === c.name && key === c.name ? (
      <SlugInput
        key={key}
        label="New name"
        nested={nested}
        initial={c.name}
        onCancel={() => setRenaming(undefined)}
        onSubmit={async (s) => {
          if (selected === c.name && !(await onLeaveSelected())) return setRenaming(undefined)
          const r = await renameCv(c.name, s)
          setRenaming(undefined)
          if (selected === c.name) await select(r.name)
          else await onChanged()
        }}
      />
    ) : (
      <div key={key} className={`cv-row${c.name === selected ? ' on' : ''}`}>
        <button
          className={`item${nested ? ' nested' : ''}${c.name === selected ? ' on' : ''}`}
          onClick={() => onSelect(c.name)}
          onContextMenu={(e) => {
            e.preventDefault()
            openMenu(e.currentTarget, { kind: 'cv', cv: c, x: e.clientX, y: e.clientY })
          }}
          onKeyDown={(e) => {
            if (!(e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey))) return
            e.preventDefault()
            const r = e.currentTarget.getBoundingClientRect()
            openMenu(e.currentTarget, { kind: 'cv', cv: c, x: r.left + 24, y: r.bottom })
          }}
        >
          <span className="label">{c.name}</span>
          {c.error && <span className="badge" title={c.error} role="img" aria-label={`Error: ${c.error}`}><Icon name="warn" size={12} /></span>}
        </button>
        <button
          className="row-menu" aria-label={`Actions for ${c.name}`} title="Actions" aria-haspopup="menu"
          onClick={(e) => {
            e.stopPropagation()
            openMenu(e.currentTarget, { kind: 'cv', cv: c, ...at(e.currentTarget) })
          }}
        >
          <Icon name="more" />
        </button>
      </div>
    )

  const folderRow = (name: string, empty: boolean) => {
    const open = !collapsed.has(name)
    if (renaming?.kind === 'folder' && renaming.name === name)
      return (
        <SlugInput
          key={`folder:${name}`}
          label="New folder name"
          placeholder="folder name"
          initial={name}
          onCancel={() => setRenaming(undefined)}
          onSubmit={async (s) => {
            await renameFolder(name, s)
            setRenaming(undefined)
            await onChanged()
          }}
        />
      )
    return (
      <div key={`folder:${name}`} className="cv-row">
        <button className="item folder" aria-expanded={open} onClick={() => toggleFolder(name)}
          onContextMenu={(e) => { e.preventDefault(); openMenu(e.currentTarget, { kind: 'folder', name, empty, x: e.clientX, y: e.clientY }) }}>
          <span className={`chevron${open ? ' open' : ''}`}><Icon name="chevron" size={12} /></span>
          <Icon name="folder" size={13} />
          <span className="label">{name}</span>
        </button>
        <button
          className="row-menu" aria-label={`Actions for folder ${name}`} title="Actions" aria-haspopup="menu"
          onClick={(e) => { e.stopPropagation(); openMenu(e.currentTarget, { kind: 'folder', name, empty, ...at(e.currentTarget) }) }}
        >
          <Icon name="more" />
        </button>
      </div>
    )
  }

  const creatingInput = creating && (
    <SlugInput
      key={creating.kind === 'duplicate' ? `dup:${creating.from}` : creating.kind}
      label={creating.kind === 'duplicate' ? `Name of the copy of ${creating.from}` : creating.kind === 'blank' ? 'Name of the new CV' : 'Name of the new folder'}
      placeholder={creating.kind === 'folder' ? 'folder name' : undefined}
      initial={creating.kind === 'duplicate' ? `${creating.from}-copy` : ''}
      onCancel={() => setCreating(undefined)}
      onSubmit={async (s) => {
        if (creating.kind === 'folder') {
          await createFolder(s)
          setCreating(undefined)
          await onChanged()
          return
        }
        const r = creating.kind === 'duplicate' ? await duplicateCv(s, creating.from) : await createCv(s, null)
        setCreating(undefined)
        await select(r.name)
      }}
    />
  )

  const cvMenu = (c: CvInfo) => (
    <>
      <button role="menuitem" onClick={() => setRenaming({ kind: 'cv', name: c.name })}>Rename…</button>
      <button role="menuitem" onClick={() => setCreating({ kind: 'duplicate', from: c.name })}>Duplicate…</button>
      <button role="menuitem" onClick={() => run(setPinned(c.name, c.pin === null))}>{c.pin === null ? 'Pin' : 'Unpin'}</button>
      {(c.folder !== null || folders.length > 0) && <div className="menu-label" role="presentation">Move to</div>}
      {c.folder !== null && <button role="menuitem" className="indent" onClick={() => run(moveCv(c.name, null))}>No folder</button>}
      {folders.filter((f) => f !== c.folder).map((f) => (
        <button key={f} role="menuitem" className="indent" onClick={() => run(moveCv(c.name, f))}>{f}</button>
      ))}
      <button
        role="menuitem"
        className="danger"
        disabled={cvs.length === 1}
        title={cvs.length === 1 ? "You can't delete the last CV" : undefined}
        onClick={async () => {
          const msg = `Delete "${c.name}"? Its history is kept.`
          // The dialog plugin replaces window.confirm with an async version, so use its confirm().
          if (!(MOCK ? window.confirm(msg) : await confirm(msg, { title: 'Delete CV', kind: 'warning' }))) return
          if (selected === c.name && !(await onLeaveSelected())) return
          try {
            await deleteCv(c.name) // the app then opens the most recently edited CV
            await onChanged()
          } catch (e) {
            onError(e)
          }
        }}
      >
        Delete…
      </button>
    </>
  )

  return (
    <section aria-label="CVs">
      {g.pinned.length > 0 && <h2>Pinned</h2>}
      {g.pinned.map((c) => row(c, false, `pin:${c.name}`))}
      {g.pinned.length > 0 && (g.folders.length > 0 || g.loose.length > 0) && <h2>All CVs</h2>}
      {g.folders.map((f) => [folderRow(f.name, f.cvs.length === 0), ...(collapsed.has(f.name) ? [] : f.cvs.map((c) => row(c, true, c.name)))])}
      {g.loose.map((c) => row(c, false, c.name))}
      {loaded && cvs.length === 0 && <p className="empty-note">No CVs yet</p>}
      {creating ? creatingInput : (
        <button className="item add" title={`New CV or folder (duplicate the open CV with ${MOD}N)`} aria-haspopup="menu"
          onClick={(e) => { e.stopPropagation(); openMenu(e.currentTarget, { kind: 'new', ...at(e.currentTarget) }) }}>
          <Icon name="plus" size={12} /> New CV…
        </button>
      )}
      {menu && (
        <div className="menu" ref={menuEl} onKeyDown={onMenuKey} style={{ left: menu.x, top: menu.y }} role="menu">
          {menu.kind === 'cv' && cvMenu(menu.cv)}
          {menu.kind === 'folder' && (
            <>
              <button role="menuitem" onClick={() => setRenaming({ kind: 'folder', name: menu.name })}>Rename…</button>
              {menu.empty && <button role="menuitem" className="danger" onClick={() => run(deleteFolder(menu.name))}>Delete</button>}
            </>
          )}
          {menu.kind === 'new' && (
            <>
              <button role="menuitem" onClick={() => setCreating({ kind: 'blank' })}>Blank CV…</button>
              <button role="menuitem" onClick={() => setCreating({ kind: 'folder' })}>New folder…</button>
            </>
          )}
        </div>
      )}
    </section>
  )
}
