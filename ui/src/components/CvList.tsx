import { useEffect, useState } from 'react'
import { type CvInfo, createTailored, deleteCv, renameCv } from '../api.ts'

type Props = { cvs: CvInfo[]; selected: string; onSelect: (n: string) => void; onChanged: () => void; onError: (e: unknown) => void }

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
      setErr(typeof e === 'object' && e && 'message' in e ? String(e.message) : String(e))
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

export function CvList({ cvs, selected, onSelect, onChanged, onError }: Props) {
  const [menu, setMenu] = useState<{ x: number; y: number; name: string }>()
  const [creating, setCreating] = useState(false)
  const [renaming, setRenaming] = useState<string>()

  useEffect(() => {
    if (!menu) return
    const close = () => setMenu(undefined)
    window.addEventListener('click', close)
    window.addEventListener('blur', close)
    window.addEventListener('keydown', close)
    return () => {
      window.removeEventListener('click', close)
      window.removeEventListener('blur', close)
      window.removeEventListener('keydown', close)
    }
  }, [menu])

  const main = cvs.filter((c) => !c.tailored_from && !c.name.startsWith('tailored/'))
  const tailored = cvs.filter((c) => !main.includes(c))

  const row = (c: CvInfo, nested: boolean) =>
    renaming === c.name ? (
      <SlugInput
        key={c.name}
        initial={slugOf(c.name)}
        onCancel={() => setRenaming(undefined)}
        onSubmit={async (s) => {
          const r = await renameCv(c.name, s)
          setRenaming(undefined)
          if (selected === c.name) onSelect(r.name)
          onChanged()
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
          setMenu({ x: e.clientX, y: e.clientY, name: c.name })
        }}
      >
        <span className="label">{nested ? slugOf(c.name) : 'Main CV'}</span>
        {c.error && <span className="badge" title={c.error}>⚠</span>}
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
            onSelect(r.name)
            onChanged()
          }}
        />
      ) : (
        <button className="item add" onClick={() => setCreating(true)}>+ New tailored CV…</button>
      )}
      {menu && (
        <div className="menu" style={{ left: menu.x, top: menu.y }} role="menu">
          <button role="menuitem" onClick={() => setRenaming(menu.name)}>Rename…</button>
          <button
            role="menuitem"
            className="danger"
            onClick={() => {
              const name = menu.name
              if (!window.confirm(`Delete "${slugOf(name)}"? It stays in the library's Git history.`)) return
              deleteCv(name).then(() => {
                if (selected === name) onSelect('main')
                onChanged()
              }, onError)
            }}
          >
            Delete…
          </button>
        </div>
      )}
    </section>
  )
}
