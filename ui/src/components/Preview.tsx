import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { errorMessage, previewHtml } from '../api.ts'
import { pageCount, placePage, withHead } from '../lib/preview.ts'
import { Icon } from './Icon.tsx'

export type PageSize = 'A4' | 'Letter'
const PAGE: Record<PageSize, { w: number; h: number }> = { A4: { w: 794, h: 1123 }, Letter: { w: 816, h: 1056 } } // CSS px at 96 dpi
const ZOOMS = [0.5, 0.67, 0.75, 0.9, 1, 1.1, 1.25, 1.5, 2]
const [MIN_ZOOM, MAX_ZOOM] = [0.3, 3]
const PAD = 24 // .preview padding

type Props = { cv: string; commit?: string; theme?: string; reloadKey: number; size: PageSize }

export function Preview({ cv, commit, theme, reloadKey, size }: Props) {
  const page = PAGE[size]
  const box = useRef<HTMLDivElement>(null)
  const [html, setHtml] = useState<string>()
  const [view, setView] = useState({ w: 0, h: 0 })
  const [off, setOff] = useState<{ x: number; y: number } | null>(null) // page position a pinch chose, in view px
  const [zoom, setZoom] = useState<number | 'fit'>('fit')
  const [height, setHeight] = useState(page.h)

  useEffect(() => {
    let live = true
    previewHtml(cv, theme, commit).then(
      (h) => live && setHtml(withHead(h)),
      (e) => live && setHtml(withHead(`<body style="font:14px system-ui;padding:2em;color:#a00">${errorMessage(e).replace(/</g, '&lt;')}</body>`)),
    )
    return () => { live = false }
  }, [cv, theme, commit, reloadKey])

  useEffect(() => {
    const el = box.current!
    // The area the page can sit in: the scroller minus its padding (24px sides and top, 64px bottom for the bar).
    const ro = new ResizeObserver(() => setView({ w: el.clientWidth - 48, h: el.clientHeight - 88 }))
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  const fit = view.w ? Math.min(1.5, Math.max(0.3, view.w / page.w)) : 1
  const scale = zoom === 'fit' ? fit : zoom
  // Keys and buttons drop any pinch position: the page goes back to centred, top-aligned.
  const zoomTo = (z: number | 'fit') => { setOff(null); setZoom(z) }
  const step = (dir: 1 | -1) =>
    zoomTo(dir > 0 ? (ZOOMS.find((z) => z > scale + 0.001) ?? scale) : ([...ZOOMS].reverse().find((z) => z < scale - 0.001) ?? scale))
  const keysRef = useRef({ step, zoomTo })
  useEffect(() => { keysRef.current = { step, zoomTo } })
  const frame = useRef<HTMLDivElement>(null)
  // What is on screen now, plus values set but not yet rendered, so fast events add up.
  const shown = useRef({ scale, view, sheetH: page.h, pageW: page.w })
  const pending = useRef<{ scale: number; x: number; y: number } | null>(null)
  useLayoutEffect(() => { pending.current = null }, [scale, off])
  useEffect(() => {
    // The preview is a canvas: the page moves by transform, not by native scroll.
    // Pinch zooms around the pointer; a plain wheel or two-finger scroll pans. placePage keeps a page that fits centred.
    const el = box.current!
    const now = () => {
      if (pending.current) return pending.current
      const f = frame.current!.getBoundingClientRect()
      const b = el.getBoundingClientRect()
      return { scale: shown.current.scale, x: f.left - b.left - PAD, y: f.top - b.top - PAD }
    }
    const set = ({ scale: k, x, y }: { scale: number; x: number; y: number }) => {
      const { view, sheetH, pageW } = shown.current
      pending.current = { scale: k, x: placePage(x, view.w, pageW * k), y: placePage(y, view.h, sheetH * k) }
      setZoom(k)
      setOff({ x: pending.current.x, y: pending.current.y })
    }
    const zoomAt = (s: number, e: { clientX: number; clientY: number }) => {
      const c = now()
      const b = el.getBoundingClientRect()
      const z = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, s))
      // Keep the page point under the pointer under it: new position = pointer - point * new scale.
      const [cx, cy] = [e.clientX - b.left - PAD, e.clientY - b.top - PAD]
      set({ scale: z, x: cx - ((cx - c.x) / c.scale) * z, y: cy - ((cy - c.y) / c.scale) * z })
    }
    let base = 1
    const wheel = (e: WheelEvent) => {
      e.preventDefault()
      const c = now()
      if (e.ctrlKey) return zoomAt(c.scale * Math.exp(-e.deltaY * 0.01), e)
      set({ scale: c.scale, x: c.x - e.deltaX, y: c.y - e.deltaY })
    }
    const start = (e: Event) => { e.preventDefault(); base = now().scale }
    const change = (e: Event) => { e.preventDefault(); zoomAt(base * (e as MouseEvent & { scale: number }).scale, e as MouseEvent) }
    el.addEventListener('wheel', wheel, { passive: false })
    el.addEventListener('gesturestart', start)
    el.addEventListener('gesturechange', change)
    return () => { el.removeEventListener('wheel', wheel); el.removeEventListener('gesturestart', start); el.removeEventListener('gesturechange', change) }
  }, [])
  useEffect(() => {
    // ⌘+ / ⌘- zoom, ⌘0 actual size, ⌘9 fit width (Preview.app's keys).
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.altKey) return
      if (e.key === '=' || e.key === '+') keysRef.current.step(1)
      else if (e.key === '-') keysRef.current.step(-1)
      else if (e.key === '0') keysRef.current.zoomTo(1)
      else if (e.key === '9') keysRef.current.zoomTo('fit')
      else return
      e.preventDefault()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  const pages = pageCount(height, page.h)
  const sheetH = pages * page.h
  const [w, h] = [page.w * scale, sheetH * scale]
  const pos = { x: placePage(off?.x ?? 0, view.w, w), y: placePage(off?.y ?? 0, view.h, h) } // default: top-left, or centred if it fits
  useLayoutEffect(() => { shown.current = { scale, view, sheetH, pageW: page.w } })
  return (
    <div className="preview-wrap">
      <div className="preview" ref={box}>
        {/* Scale with a transform, not CSS zoom: zoom resizes the iframe viewport, so the theme reflows and the page size drifts. */}
        <div className="paper-frame" ref={frame} style={{ width: w, height: h, transform: `translate(${pos.x}px, ${pos.y}px)` }}>
        <div className="paper" style={{ width: page.w, height: sheetH, transform: `scale(${scale})` }}>
          <iframe
            title="CV preview"
            sandbox="allow-same-origin"
            srcDoc={html}
            onLoad={(e) => {
              // srcdoc is same-origin, so the content height is readable (the <html> box, not the viewport).
              const h = e.currentTarget.contentDocument?.documentElement.getBoundingClientRect().height
              setHeight(h && h > 0 ? h : page.h)
            }}
          />
          {Array.from({ length: pages - 1 }, (_, i) => (
            <div key={i} className="page-break" style={{ top: (i + 1) * page.h }} title="Approximate page break; the print margins decide the exact one">
              <span>Page {i + 2}</span>
            </div>
          ))}
        </div>
        </div>
      </div>
      <div className="preview-bar" role="toolbar" aria-label="Preview">
        <span className="pages" title={`${size} pages`}>{pages} page{pages === 1 ? '' : 's'}</span>
        <span className="sep" />
        <button className="icon-btn" aria-label="Zoom out" title="Zoom out (⌘−)" onClick={() => step(-1)}><Icon name="minus" size={12} /></button>
        <button className="zoom-value" title="Actual size (⌘0)" onClick={() => zoomTo(1)}>{Math.round(scale * 100)}%</button>
        <button className="icon-btn" aria-label="Zoom in" title="Zoom in (⌘+)" onClick={() => step(1)}><Icon name="plus" size={12} /></button>
        <button className={`fit${zoom === 'fit' ? ' on' : ''}`} aria-pressed={zoom === 'fit'} title="Fit width (⌘9)" onClick={() => zoomTo('fit')}>Fit</button>
      </div>
    </div>
  )
}
