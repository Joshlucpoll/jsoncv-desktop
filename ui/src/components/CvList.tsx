import { useEffect, useRef, useState } from 'react'
import { confirm } from '@tauri-apps/plugin-dialog'
import { type CvInfo, MOCK, createTailored, deleteCv, errorMessage, renameCv } from '../api.ts'

type Props = {
  cvs: CvInfo[]; selected: string; onSelect: (n: string) => void; onChanged: () => Promise<unknown>; onError: (e: unknown) => void
  /** Runs the edit panel's leave guard before the selected CV is renamed or deleted; false cancels. */
  onLeaveSelected: () => Promise<boolean>
}

const SLUG = /^[a-z0-9][a-z0-9-]{0,63}$/
const slugOf = (name: string) => name.replace(/^tailored\//, '')

function SlugInput({ initial, onSubmit, onCancel }: { initial: string; onSubmit: (s: string) => Promise<void>; onCancel: () => void }) {
  const [v, setV] = useState(initial)
  const [err, setErr] = useState<string>()
  const submit = async () => {
    if (!SLUG.test(v)) return setErr('Use a-z, 0-9 and "-"; start with a letter or digit (max 64).')
    try {
      await onSubmit(v)
    } catch (e) {
      setErr(errorMessage(e))
    }
  }
  return (
    <div className="slug-form nested">
      <input
        autoFocus
        value={v}
        placeholder="slug, e.g. stripe"
        onChange={(e) => { setV(e.target.value); setErr(undefined) }}
        onFocus={(e) => e.target.select()}
        onKeyDown={(e) => { if (e.key === 'Enter') submit(); if (e.key === 'Escape') onCancel() }}
        onBlur={onCancel}
      />
      {err && <div className="field-error">{err}</div>}
    </div>
  )
}

export function CvList({ cvs, selected, onSelect, onChanged, onError, onLeaveSelected }: Props) {
  const [menu, setMenu] = useState<{ x: number; y: number; name: string }>()
  const [creating, setCreating] = useState(false)
  const [renaming, setRenaming] = useState<string>()

  const menuEl = useRef<HTMLDivElement>(null)
  const returnTo = useRef<HTMLElement | null>(null)
  const closeMenu = (refocus: boolean) => {
    setMenu(undefined)
    if (refocus) returnTo.current?.focus()
  }
  const openMenu = (el: HTMLElement, x: number, y: number, name: string) => {
    returnTo.current = el
    setMenu({ x, y, name })
  }

  useEffect(() => {
    if (!menu) return
    menuEl.current?.querySelector('button')?.focus()
    const close = () => setMenu(undefined)
    window.addEventListener('click', close)
    window.addEventListener('blur', close)
    return () => {
      window.removeEventListener('click', close)
      window.removeEventListener('blur', close)
    }
  }, [menu])

  const onMenuKey = (e: React.KeyboardEvent) => {
    const items = [...menuEl.current!.querySelectorAll('button')]
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

  const main = cvs.filter((c) => !c.tailored_from && !c.name.startsWith('tailored/'))
  const tailored = cvs.filter((c) => !main.includes(c))

  const row = (c: CvInfo, nested: boolean) =>
    renaming === c.name ? (
      <SlugInput
        key={c.name}
        initial={slugOf(c.name)}
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
      <button
        key={c.name}
        className={`item${nested ? ' nested' : ''}${c.name === selected ? ' on' : ''}`}
        onClick={() => onSelect(c.name)}
        onContextMenu={(e) => {
          if (!nested) return
          e.preventDefault()
          openMenu(e.currentTarget, e.clientX, e.clientY, c.name)
        }}
        onKeyDown={(e) => {
          if (!nested || !(e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey))) return
          e.preventDefault()
          const r = e.currentTarget.getBoundingClientRect()
          openMenu(e.currentTarget, r.left + 24, r.bottom, c.name)
        }}
      >
        <span className="label">{nested ? slugOf(c.name) : 'Main CV'}</span>
        {c.error && <span className="badge" title={c.error} role="img" aria-label={`Error: ${c.error}`}>⚠</span>}
      </button>
    )

  return (
    <section>
      <h2>CVs</h2>
      {main.map((c) => row(c, false))}
      {tailored.map((c) => row(c, true))}
      {creating ? (
        <SlugInput
          initial=""
          onCancel={() => setCreating(false)}
          onSubmit={async (s) => {
            const r = await createTailored(s, 'main')
            setCreating(false)
            await select(r.name)
          }}
        />
      ) : (
        <button className="item add" onClick={() => setCreating(true)}>+ New tailored CV…</button>
      )}
      {menu && (
        <div className="menu" ref={menuEl} onKeyDown={onMenuKey} style={{ left: menu.x, top: menu.y }} role="menu">
          <button role="menuitem" onClick={() => setRenaming(menu.name)}>Rename…</button>
          <button
            role="menuitem"
            className="danger"
            onClick={async () => {
              const name = menu.name
              const msg = `Delete "${slugOf(name)}"? It stays in the library's Git history.`
              // The dialog plugin replaces window.confirm with an async version, so use its confirm().
              if (!(MOCK ? window.confirm(msg) : await confirm(msg, { title: 'Delete CV', kind: 'warning' }))) return
              if (selected === name && !(await onLeaveSelected())) return
              try {
                await deleteCv(name)
                if (selected === name) onSelect('main')
                await onChanged()
              } catch (e) {
                onError(e)
              }
            }}
          >
            Delete…
          </button>
        </div>
      )}
    </section>
  )
}
