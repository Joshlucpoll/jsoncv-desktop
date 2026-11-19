const ua = navigator.userAgent
export const revealLabel = /mac/i.test(ua) ? 'Reveal in Finder' : /windows/i.test(ua) ? 'Show in Explorer' : 'Show in folder'
export const THEME_ID = /^[a-z0-9][a-z0-9-]{0,63}$/
export const MAC = /mac/i.test(ua)
export const MOD = MAC ? '⌘' : 'Ctrl+'
