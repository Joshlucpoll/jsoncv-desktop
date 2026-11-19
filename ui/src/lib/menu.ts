import { useEffect, useRef, useState } from 'react'

/** Open state, outside-click close and arrow-key focus for a popup menu. */
export function useMenu() {
  const [open, setOpen] = useState(false)
  const box = useRef<HTMLDivElement>(null)
  const items = () => Array.from(box.current?.querySelectorAll<HTMLElement>('[role^="menuitem"]') ?? [])
  useEffect(() => {
    if (!open) return
    const els = items()
    ;(els.find((e) => e.getAttribute('aria-checked') === 'true') ?? els[0])?.focus()
    const away = (e: MouseEvent) => !box.current?.contains(e.target as Node) && setOpen(false)
    window.addEventListener('mousedown', away)
    return () => window.removeEventListener('mousedown', away)
  }, [open])
  const onKeyDown = (e: React.KeyboardEvent) => {
    const els = items()
    const i = els.indexOf(document.activeElement as HTMLElement)
    if (e.key === 'Escape') { setOpen(false); box.current?.querySelector<HTMLElement>('.trigger')?.focus() }
    else if (e.key === 'ArrowDown') { e.preventDefault(); els[(i + 1) % els.length]?.focus() }
    else if (e.key === 'ArrowUp') { e.preventDefault(); els[(i - 1 + els.length) % els.length]?.focus() }
  }
  return { open, setOpen, box, onKeyDown }
}
