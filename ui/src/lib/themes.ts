/** A theme folder id from a display name: "My Theme!" → "my-theme". Empty when nothing usable is left. */
export const themeIdFromName = (name: string): string =>
  name.normalize('NFKD').replace(/\p{M}/gu, '').toLowerCase()
    .replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '')
    .slice(0, 64).replace(/-+$/, '')

/** The open action to use: the saved one if that editor is still installed, otherwise Reveal. */
export const openAction = (stored: string | null, editors: { id: string }[]): string =>
  stored && editors.some((e) => e.id === stored) ? stored : 'reveal'

/** The prompt "Copy prompt" puts on the clipboard for the user's agent. */
export const themePrompt = (cv: string | null): string =>
  `Using the jsoncv MCP server, read jsoncv://theme-guide, then create a theme called <name> that looks like: <describe it>. Fix every render_error, then set it as the theme for ${cv ? `my CV "${cv}"` : 'my CV'}.`
