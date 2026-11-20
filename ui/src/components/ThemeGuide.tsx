import { useEffect, useRef, useState } from 'react'
import { errorMessage, openLink, themeGuideHtml } from '../api.ts'
import { themePrompt } from '../lib/themes.ts'
import { Icon } from './Icon.tsx'
import { Copy } from './Settings.tsx'

type Props = { cv: string | null; onAgents: () => void; onClose: () => void; onError: (e: unknown) => void }

/** "How do I create a theme?": agents first, then your own editor, then the full guide. */
export function ThemeGuide({ cv, onAgents, onClose, onError }: Props) {
  const [html, setHtml] = useState<string>()
  const close = useRef<HTMLButtonElement>(null)
  useEffect(() => { close.current?.focus() }, [])
  useEffect(() => {
    let live = true
    themeGuideHtml().then((h) => live && setHtml(h), (e) => live && setHtml(`<p>${errorMessage(e).replace(/</g, '&lt;')}</p>`))
    return () => { live = false }
  }, [])
  return (
    <div className="scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()} onKeyDown={(e) => { if (e.key === 'Escape') { e.stopPropagation(); onClose() } }}>
      <div className="dialog theme-guide" role="dialog" aria-modal="true" aria-label="How to create a theme">
        <header>
          <h2>How do I create a theme?</h2>
          <span className="spacer" />
          <button ref={close} className="icon-btn" aria-label="Close" title="Close (Esc)" onClick={onClose}><Icon name="x" size={16} /></button>
        </header>
        <section>
          <h3>Ask your agent <span className="badge soft">recommended</span></h3>
          <p>An agent can write a theme, check which fields it hides, and preview it.</p>
          <div className="guide-actions">
            <Copy text={themePrompt(cv)} label="Copy prompt" />
            <button className="link" onClick={onAgents}>Set up an agent</button>
          </div>
        </section>
        <section>
          <h3>Edit the files yourself</h3>
          <p>Click New theme…, then open it in your editor. The preview reloads when you save.</p>
        </section>
        <section>
          <h3>Reference</h3>
          {/* Safe: the guide is bundled with the app, and the renderer escapes raw HTML in it. */}
          {/* A link must not navigate the app's own window: open web links in the browser, drop the rest. */}
          <div className="guide-ref" dangerouslySetInnerHTML={{ __html: html ?? '' }} onClick={(e) => {
            const a = (e.target as HTMLElement).closest('a')
            if (!a) return
            e.preventDefault()
            if (/^https?:/i.test(a.href)) openLink(a.href).catch(onError)
          }} />
        </section>
      </div>
    </div>
  )
}
