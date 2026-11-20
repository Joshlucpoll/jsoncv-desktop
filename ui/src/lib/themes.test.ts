import { describe, expect, it } from 'vitest'
import { openAction, themeIdFromName } from './themes.ts'
import { THEME_ID } from './platform.ts'

describe('themeIdFromName', () => {
  it('slugs names', () => {
    expect(themeIdFromName('Navy')).toBe('navy')
    expect(themeIdFromName('  My Cool Theme! ')).toBe('my-cool-theme')
    expect(themeIdFromName('Ünïcode Café')).toBe('unicode-cafe')
    expect(themeIdFromName('2 Column')).toBe('2-column')
  })
  it('returns empty when nothing is usable', () => {
    expect(themeIdFromName('!!!')).toBe('')
    expect(themeIdFromName('日本語')).toBe('')
  })
  it('caps at 64 with no trailing hyphen', () => {
    const id = themeIdFromName('a'.repeat(63) + ' b')
    expect(id.length).toBeLessThanOrEqual(64)
    expect(id.endsWith('-')).toBe(false)
    expect(THEME_ID.test(id)).toBe(true)
  })
})

describe('openAction', () => {
  const eds = [{ id: 'vscode', label: 'VS Code' }]
  it('defaults to reveal', () => expect(openAction(null, eds)).toBe('reveal'))
  it('keeps an installed editor', () => expect(openAction('vscode', eds)).toBe('vscode'))
  it('falls back when the editor is gone', () => expect(openAction('zed', eds)).toBe('reveal'))
})
