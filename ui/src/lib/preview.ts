// Same policy the preview scheme sends; the sandbox (no allow-scripts) also blocks theme scripts.
// <base target="_blank"> stops a clicked link from loading a web page inside the preview (popups are blocked).
const HEAD = `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data: https: http:; font-src data:"><base target="_blank">`
export const withHead = (html: string) => (/<head(\s[^>]*)?>/i.test(html) ? html.replace(/<head(\s[^>]*)?>/i, (m) => m + HEAD) : HEAD + html)

/** Pages a document of `height` px fills; a few px of rounding do not start a new page. */
export const pageCount = (height: number, pageH: number) => Math.max(1, Math.ceil((height - 4) / pageH))

/**
 * Page position on one axis (page edge minus view edge) for a wanted position `pos`.
 * A page that fits is centred; a larger one follows `pos` but never shows the background past its edges.
 */
export const placePage = (pos: number, view: number, size: number) =>
  size <= view ? (view - size) / 2 : Math.min(0, Math.max(view - size, pos))
