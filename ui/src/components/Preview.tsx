import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { errorMessage, previewHtml, readCv } from '../api.ts'
import { type Leaf, type Prepared, elementFor, leaves, pathAt, prepare } from '../lib/locate.ts'
import { pageCount, placePage, withHead } from '../lib/preview.ts'
import { roman } from '../lib/roman.ts'
import { Icon } from './Icon.tsx'

export type PageSize = 'A4' | 'Letter'
const PAGE: Record<PageSize, { w: number; h: number }> = { A4: { w: 794, h: 1123 }, Letter: { w: 816, h: 1056 } } // CSS px at 96 dpi
const ZOOMS = [0.5, 0.67, 0.75, 0.9, 1, 1.1, 1.25, 1.5, 2]
const [MIN_ZOOM, MAX_ZOOM] = [0.3, 3]
const PAD = 24 // .preview padding

const HL_STYLE = '[data-jsoncv-hl]{outline:2px solid rgba(10,100,255,.85);outline-offset:2px;border-radius:2px;background-color:rgba(10,100,255,.07)}'
/** Pointer travel below this is a click (pick a field), above it a drag (pan). */
const CLICK_PX = 4

type Props = {
  cv: string; commit?: string; theme?: string; reloadKey: number; size: PageSize
  /** One long page: no page breaks. */
  continuous?: boolean
  /** Called with the CV path (JSON Pointer) under a click, or undefined for a click on nothing. */
  onPick?: (path: string | undefined) => void
  /** Marks what renders `path`; `scroll` pans the page to it when it is out of view. */
  highlight?: { path: string; scroll: boolean; n: number }
}

export function Preview({ cv, commit, theme, reloadKey, size, continuous, onPick, highlight }: Props) {
  const page = PAGE[size]
  const box = useRef<HTMLDivElement>(null)
  const [html, setHtml] = useState<string>()
  const [view, setView] = useState({ w: 0, h: 0 })
  const [off, setOff] = useState<{ x: number; y: number } | null>(null) // page position a pinch chose, in view px
  const [zoom, setZoom] = useState<number | 'fit'>('fit')
  const [height, setHeight] = useState(page.h)
  const iframe = useRef<HTMLIFrameElement>(null)
  // Text index of the loaded page and the CV it shows, for mapping clicks and highlights to fields.
  const located = useRef<{ p: Prepared; ls: Leaf[] }>(undefined)
  const cvData = useRef<unknown>(undefined)
  const [loads, setLoads] = useState(0)

  const pickable = !!onPick && !commit
  useEffect(() => {
    let live = true
    cvData.current = undefined
    if (pickable) readCv(cv).then((d) => { if (live) { cvData.current = d.cv; located.current = undefined; setLoads((n) => n + 1) } }, () => {})
    previewHtml(cv, theme, commit).then(
      (h) => live && setHtml(withHead(h)),
      (e) => live && setHtml(withHead(`<body style="font:14px system-ui;padding:2em;color:#a00">${errorMessage(e).replace(/</g, '&lt;')}</body>`)),
    )
    return () => { live = false }
  }, [cv, theme, commit, reloadKey, pickable])

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
  // The click target inside the page: the iframe is scaled, so pointer offsets divide by its scale.
  const pickRef = useRef((_x: number, _y: number) => {})
  useEffect(() => {
    pickRef.current = (x, y) => {
      const f = iframe.current
      const doc = f?.contentDocument
      if (!onPick || !f || !doc) return
      const r = f.getBoundingClientRect()
      const k = r.width / page.w
      const hit = doc.elementFromPoint((x - r.left) / k, (y - r.top) / k)
      if (!located.current && cvData.current !== undefined) located.current = { p: prepare([doc.body, ...doc.body.querySelectorAll('*')]), ls: leaves(cvData.current) }
      const l = located.current
      onPick(hit && l ? pathAt(l.p, l.ls, hit) : undefined)
    }
  })
  useEffect(() => { keysRef.current = { step, zoomTo } })
  const frame = useRef<HTMLDivElement>(null)
  // What is on screen now, plus values set but not yet rendered, so fast events add up.
  const shown = useRef({ scale, view, sheetH: page.h, pageW: page.w })
  const pending = useRef<{ scale: number; x: number; y: number } | null>(null)
  useLayoutEffect(() => { pending.current = null }, [scale, off])
  useEffect(() => {
    // The preview is a canvas: the page moves by transform, not by native scroll.
    // Pinch zooms around the pointer; a plain wheel or two-finger scroll pans, and so does dragging with the mouse.
    // placePage keeps a page that fits centred.
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
    let grab: { x: number; y: number; moved: boolean; at: { scale: number; x: number; y: number } } | null = null
    const down = (e: PointerEvent) => {
      if (e.button !== 0) return
      e.preventDefault()
      grab = { x: e.clientX, y: e.clientY, moved: false, at: now() }
      el.setPointerCapture(e.pointerId)
      el.classList.add('grabbing')
    }
    const move = (e: PointerEvent) => {
      if (!grab) return
      const { at } = grab
      if (Math.hypot(e.clientX - grab.x, e.clientY - grab.y) > CLICK_PX) grab.moved = true
      if (!grab.moved) return
      set({ scale: at.scale, x: at.x + e.clientX - grab.x, y: at.y + e.clientY - grab.y })
    }
    const up = (e: PointerEvent) => {
      const click = grab && !grab.moved && e.type === 'pointerup'
      grab = null
      el.classList.remove('grabbing')
      if (click) pickRef.current(e.clientX, e.clientY)
    }
    const listeners = [['wheel', wheel], ['gesturestart', start], ['gesturechange', change], ['pointerdown', down], ['pointermove', move], ['pointerup', up], ['pointercancel', up]] as const
    for (const [t, f] of listeners) el.addEventListener(t, f as EventListener, t === 'wheel' ? { passive: false } : undefined)
    return () => { for (const [t, f] of listeners) el.removeEventListener(t, f as EventListener) }
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

  const pages = continuous ? 1 : pageCount(height, page.h)
  const sheetH = continuous ? Math.max(height, 200) : pages * page.h
  const [w, h] = [page.w * scale, sheetH * scale]
  const pos = { x: placePage(off?.x ?? 0, view.w, w), y: placePage(off?.y ?? 0, view.h, h) } // default: top-left, or centred if it fits
  useLayoutEffect(() => { shown.current = { scale, view, sheetH, pageW: page.w } })

  // Mark the highlighted field inside the page, and pan to it when the edit panel asked for it.
  const lastScroll = useRef(0)
  useEffect(() => {
    const doc = iframe.current?.contentDocument
    if (!doc?.body) return
    doc.querySelectorAll('[data-jsoncv-hl]').forEach((e) => e.removeAttribute('data-jsoncv-hl'))
    if (!highlight || cvData.current === undefined) return
    if (!doc.getElementById('jsoncv-hl')) {
      const st = doc.createElement('style')
      st.id = 'jsoncv-hl'
      st.textContent = HL_STYLE
      doc.head.append(st)
    }
    located.current ??= { p: prepare([doc.body, ...doc.body.querySelectorAll('*')]), ls: leaves(cvData.current) }
    const el = elementFor(located.current.p, located.current.ls, highlight.path) as HTMLElement | undefined
    if (!el || el === doc.body) return
    el.setAttribute('data-jsoncv-hl', '')
    if (!highlight.scroll || lastScroll.current === highlight.n) return
    lastScroll.current = highlight.n
    const r = el.getBoundingClientRect()
    const [top, bottom] = [r.top * scale + pos.y, r.bottom * scale + pos.y]
    if (top >= 0 && bottom <= view.h) return
    setOff({ x: pos.x, y: Math.round(view.h / 3 - r.top * scale) }) // oxlint-disable-line react/set-state-in-effect -- pans to a field the editor named
  }, [highlight, loads]) // eslint-disable-line react-hooks/exhaustive-deps -- reads the current pan; reruns only for a new highlight or page
  return (
    <div className="preview-wrap">
      <div className="preview" ref={box}>
        {/* Scale with a transform, not CSS zoom: zoom resizes the iframe viewport, so the theme reflows and the page size drifts. */}
        <div className="paper-frame" ref={frame} style={{ width: w, height: h, transform: `translate(${pos.x}px, ${pos.y}px)` }}>
        <div className="paper" style={{ width: page.w, height: sheetH, transform: `scale(${scale})` }}>
          <iframe
            ref={iframe}
            title="CV preview"
            sandbox="allow-same-origin"
            srcDoc={html}
            onLoad={(e) => {
              // srcdoc is same-origin, so the content height is readable (the <html> box, not the viewport).
              const h = e.currentTarget.contentDocument?.documentElement.getBoundingClientRect().height
              setHeight(h && h > 0 ? h : page.h)
              located.current = undefined
              setLoads((n) => n + 1)
            }}
          />
          {!continuous && Array.from({ length: pages - 1 }, (_, i) => (
            <div key={i} className="page-break" style={{ top: (i + 1) * page.h }} title="Approximate page break; the print margins decide the exact one">
              <span>Pagina {roman(i + 2)}</span>
            </div>
          ))}
        </div>
        </div>
      </div>
      <div className="preview-bar" role="toolbar" aria-label="Preview">
        <span className="pages" title={continuous ? 'One long page (Options › Page setup)' : `${pages} ${size} page${pages === 1 ? '' : 's'}`}>{continuous ? 'Continuous' : `${roman(pages)} ${pages === 1 ? 'pagina' : 'paginae'}`}</span>
        <span className="sep" />
        <button className="icon-btn" aria-label="Zoom out" title="Zoom out (⌘−)" onClick={() => step(-1)}><Icon name="minus" size={12} /></button>
        <button className="zoom-value" title="Actual size (⌘0)" onClick={() => zoomTo(1)}>{Math.round(scale * 100)}%</button>
        <button className="icon-btn" aria-label="Zoom in" title="Zoom in (⌘+)" onClick={() => step(1)}><Icon name="plus" size={12} /></button>
        <button className={`fit${zoom === 'fit' ? ' on' : ''}`} aria-pressed={zoom === 'fit'} title="Fit width (⌘9)" onClick={() => zoomTo('fit')}>Fit</button>
      </div>
    </div>
  )
}
