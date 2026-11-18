import { useEffect, useRef, useState } from 'react'
import { previewUrl } from '../api.ts'

const PAPER_W = 794
const PAPER_H = 1123 // A4 at 96 dpi; fallback when the iframe document is not readable

export function Preview({ cv, commit, reloadKey }: { cv: string; commit?: string; reloadKey: number }) {
  const box = useRef<HTMLDivElement>(null)
  const [zoom, setZoom] = useState(1)
  const [height, setHeight] = useState(PAPER_H)

  useEffect(() => {
    const el = box.current!
    const ro = new ResizeObserver(() => setZoom(Math.min(1.5, Math.max(0.3, (el.clientWidth - 48) / PAPER_W))))
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  return (
    <div className="preview" ref={box}>
      <div className="paper" style={{ width: PAPER_W, height, zoom }}>
        <iframe
          key={`${cv}|${commit ?? ''}|${reloadKey}`}
          title="CV preview"
          sandbox="allow-same-origin"
          src={previewUrl(cv, undefined, commit)}
          onLoad={(e) => {
            // Cross-origin in the real app (throws); readable in the dev mock.
            try {
              const h = e.currentTarget.contentDocument?.documentElement.scrollHeight
              setHeight(Math.max(PAPER_H, h ?? PAPER_H))
            } catch {
              setHeight(PAPER_H)
            }
          }}
        />
      </div>
    </div>
  )
}
