/** A theme folder id from a display name: "My Theme!" → "my-theme". Empty when nothing usable is left. */
export const themeIdFromName = (name: string): string =>
  name.normalize('NFKD').replace(/\p{M}/gu, '').toLowerCase()
    .replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '')
    .slice(0, 64).replace(/-+$/, '')
